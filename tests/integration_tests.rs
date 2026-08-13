use bible_io::{bible_books_enum::ParseBibleBookError, Bible, BibleBook};
use std::str::FromStr;

mod common;
use common::test_utils;

#[test]
fn test_library_imports() {
    // Test that we can import all the main types
    use bible_io::Verse;

    // Create a simple verse to test the import
    let verse = Verse::new(BibleBook::Genesis, 1, 1, "Test verse".to_string());
    // Note: We can't access private fields in integration tests
    // This test just verifies the import works
    assert_eq!(format!("{}", verse), "1: Test verse");
}

#[test]
fn test_bible_book_enum_import() {
    // Test that the BibleBook enum is accessible
    let genesis = BibleBook::Genesis;
    let psalms = BibleBook::Psalms;
    let revelation = BibleBook::Revelation;

    assert_eq!(genesis.abbreviation(), "gn");
    assert_eq!(psalms.abbreviation(), "ps");
    assert_eq!(revelation.abbreviation(), "re");
}

#[test]
fn test_bible_creation_with_real_data() {
    let file_path = test_utils::get_kjv_json()
        .expect("repository fixture tests/fixtures/en_kjv.json must be present");

    println!("Using en_kjv.json at: {}", file_path);

    let bible = Bible::new(&file_path).expect("Failed to load Bible JSON");

    let verse = bible
        .get_verse(BibleBook::Genesis, 1, 1)
        .expect("Genesis 1:1 must be present in the repository fixture");
    let verse_str = format!("{}", verse);
    assert_eq!(
        verse_str,
        "1: In the beginning God created the heaven and the earth."
    );

    let book = bible
        .get_book(BibleBook::Genesis)
        .expect("Genesis must be present in the repository fixture");
    assert_eq!(book.abbrev(), "gn");
    assert_eq!(book.title(), "Genesis");
}

#[test]
fn test_bible_book_display_format() {
    // The shared reference package displays canonical English book names.
    let genesis = BibleBook::Genesis;
    let psalms = BibleBook::Psalms;

    assert_eq!(format!("{}", genesis), "Genesis");
    assert_eq!(format!("{}", psalms), "Psalms");
}

#[test]
fn test_bible_book_from_str_invalid() {
    for input in ["invalid", "", "xyz"] {
        let error: ParseBibleBookError = BibleBook::from_str(input).unwrap_err();
        assert_eq!(error.input(), input);
    }
}

#[test]
fn test_bible_book_debug() {
    // Test that Debug trait works
    let book = BibleBook::Genesis;
    let debug_str = format!("{:?}", book);
    assert!(debug_str.contains("Genesis"));
}

#[test]
fn test_bible_book_clone_copy() {
    // Test Clone and Copy traits
    let book1 = BibleBook::Genesis;
    let book2 = book1; // Copy
    #[allow(clippy::clone_on_copy)]
    let book3 = book1.clone(); // Clone

    assert_eq!(book1, book2);
    assert_eq!(book1, book3);
    assert_eq!(book2, book3);
}
