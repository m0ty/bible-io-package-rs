use bible_io::{Bible, BibleError};

mod common;
use common::test_utils;

#[test]
fn test_get_verse_by_reference_valid() {
    let file_path = test_utils::get_kjv_json()
        .expect("repository fixture tests/fixtures/en_kjv.json must be present");

    let bible = Bible::new(&file_path).expect("Failed to load Bible JSON");

    let verse = bible
        .get_verse_by_reference("Gen 1:1")
        .expect("Verse not found");
    assert_eq!(
        verse.text(),
        "In the beginning God created the heaven and the earth."
    );

    let verse = bible
        .get_verse_by_reference("John 3:16")
        .expect("Verse not found");
    assert!(verse.text().starts_with("For God so loved the world"));

    // The shared parser also accepts adjacent coordinates.
    let verse = bible
        .get_verse_by_reference("John3:16")
        .expect("Verse not found");
    assert!(verse.text().starts_with("For God so loved the world"));

    // Bundled localized book names work without package-local lookup tables.
    let verse = bible
        .get_verse_by_reference("Juan 3:16")
        .expect("Verse not found");
    assert!(verse.text().starts_with("For God so loved the world"));

    let verse = bible
        .get_verse_by_reference("Exo 3:14")
        .expect("Verse not found");
    assert!(verse
        .text()
        .starts_with("And God said unto Moses, I AM THAT I AM"));

    let verse = bible
        .get_verse_by_reference("1Co 13:1")
        .expect("Verse not found");
    assert!(verse
        .text()
        .starts_with("Though I speak with the tongues of men and of angels"));

    let verse = bible
        .get_verse_by_reference("Rev 22:21")
        .expect("Verse not found");
    assert!(verse
        .text()
        .starts_with("The grace of our Lord Jesus Christ {be} with you all"));
}

#[test]
fn test_get_verse_by_reference_invalid() {
    let file_path = test_utils::get_kjv_json()
        .expect("repository fixture tests/fixtures/en_kjv.json must be present");

    let bible = Bible::new(&file_path).expect("Failed to load Bible JSON");
    assert!(matches!(
        bible.get_verse_by_reference("Unknown 1:1"),
        Err(BibleError::BookNotFound { .. })
    ));
    assert!(matches!(
        bible.get_verse_by_reference("Genesis 1"),
        Err(BibleError::ReferenceParse { input, .. }) if input == "Genesis 1"
    ));
    assert!(matches!(
        bible.get_verse_by_reference("John 3:16-17"),
        Err(BibleError::ReferenceParse { input, .. }) if input == "John 3:16-17"
    ));
}
