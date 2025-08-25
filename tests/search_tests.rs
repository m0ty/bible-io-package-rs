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
    assert!(search_results.contains(&(BibleBook::Genesis, 1, 1)));
    assert!(search_results.contains(&(BibleBook::John, 1, 1)));

    let indexed_results = index.search(query);
    assert_eq!(search_results, indexed_results);
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
    assert_eq!(search_results, vec![(BibleBook::FirstThessalonians, 5, 16)]);
    assert_eq!(search_results, indexed_results);
}
