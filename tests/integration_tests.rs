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

    assert_eq!(genesis.as_str(), "gn");
    assert_eq!(psalms.as_str(), "ps");
    assert_eq!(revelation.as_str(), "re");
}

#[test]
fn test_bible_creation_with_real_data() {
    // This test requires the en_kjv.json file to exist
    let file_path = match test_utils::get_kjv_json() {
        Some(path) => path,
        None => {
            // Skip the test if the file doesn't exist
            println!("Skipping test_bible_creation_with_real_data: en_kjv.json not found");
            println!("To run this test, place en_kjv.json in tests/fixtures/");
            return;
        }
    };

    println!("Using en_kjv.json at: {}", file_path);

    let bible = Bible::new_from_json(&file_path).expect("Failed to load Bible JSON");

    // Test that we can get a verse from Genesis
    if let Ok(verse) = bible.get_verse(BibleBook::Genesis, 1, 1) {
        // We can't access private fields, but we can test the Display trait
        let verse_str = format!("{}", verse);
        assert_eq!(
            verse_str,
            "1: In the beginning God created the heaven and the earth."
        );
    }

    // Test that we can get a book
    if let Ok(book) = bible.get_book(BibleBook::Genesis) {
        assert_eq!(book.abbrev(), "gn");
        assert_eq!(book.title(), "Genesis");
    }
}

#[test]
fn test_bible_book_display_format() {
    // Test that the Display trait works correctly
    let genesis = BibleBook::Genesis;
    let psalms = BibleBook::Psalms;

    assert_eq!(format!("{}", genesis), "gn");
    assert_eq!(format!("{}", psalms), "ps");
}

#[test]
fn test_bible_book_from_str_invalid() {
    // Test that invalid strings return Err
    assert_eq!(BibleBook::from_str("invalid"), Err(ParseBibleBookError));
    assert_eq!(BibleBook::from_str(""), Err(ParseBibleBookError));
    assert_eq!(BibleBook::from_str("xyz"), Err(ParseBibleBookError));
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
