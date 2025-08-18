use rust_bible_struct::{Bible, BibleBook};

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

    let bible = Bible::new_from_json(&file_path).expect("Failed to load Bible JSON");
    let results = bible.search("in the beginning");
    assert!(results.contains(&(BibleBook::Genesis, 1, 1)));
    assert!(results.contains(&(BibleBook::John, 1, 1)));
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

    let bible = Bible::new_from_json(&file_path).expect("Failed to load Bible JSON");
    let results = bible.search("REJOICE EVERMORE");
    assert_eq!(results, vec![(BibleBook::FirstThessalonians, 5, 16)]);
}
