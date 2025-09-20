use bible_io::{Bible, BibleBook};

mod common;
use common::test_utils;

#[test]
fn search_finds_multiple_books() {
    let file_path = match test_utils::get_kjv_json() {
        Some(path) => path,
        None => {
            println!("Skipping search_finds_multiple_books: en_kjv.json not found");
            return;
        }
    };

    let mut bible = Bible::new_from_json(&file_path).expect("Failed to load Bible JSON");
    let index = bible.build_search_index();
    let query = "in the beginning";
    let search_results = bible.search(query);
    let genesis = bible
        .get_verse(BibleBook::Genesis, 1, 1)
        .expect("Missing Genesis 1:1")
        .clone();
    let john = bible
        .get_verse(BibleBook::John, 1, 1)
        .expect("Missing John 1:1")
        .clone();

    assert!(search_results.contains(&genesis));
    assert!(search_results.contains(&john));

    let indexed_results = index.search(query);
    let verses_from_index: Vec<_> = indexed_results
        .into_iter()
        .map(|(book, chapter, verse)| {
            bible
                .get_verse(book, chapter, verse)
                .expect("Indexed verse missing from Bible")
                .clone()
        })
        .collect();
    assert_eq!(search_results, verses_from_index);
}

#[test]
fn search_is_case_insensitive() {
    let file_path = match test_utils::get_kjv_json() {
        Some(path) => path,
        None => {
            println!("Skipping search_is_case_insensitive: en_kjv.json not found");
            return;
        }
    };

    let mut bible = Bible::new_from_json(&file_path).expect("Failed to load Bible JSON");
    let index = bible.build_search_index();
    let query = "REJOICE EVERMORE";
    let search_results = bible.search(query);
    let indexed_results = index.search(query);

    let expected = bible
        .get_verse(BibleBook::FirstThessalonians, 5, 16)
        .expect("Missing 1 Thessalonians 5:16")
        .clone();

    assert_eq!(search_results, vec![expected.clone()]);
    let verses_from_index: Vec<_> = indexed_results
        .into_iter()
        .map(|(book, chapter, verse)| {
            bible
                .get_verse(book, chapter, verse)
                .expect("Indexed verse missing from Bible")
                .clone()
        })
        .collect();
    assert_eq!(search_results, verses_from_index);
}
