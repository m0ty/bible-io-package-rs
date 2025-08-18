use rust_bible_struct::Bible;

mod common;
use common::test_utils;

#[test]
fn test_get_verse_by_reference_valid() {
    let file_path = match test_utils::get_kjv_json() {
        Some(p) => p,
        None => {
            println!("Skipping test_get_verse_by_reference_valid: en_kjv.json not found");
            return;
        }
    };

    let bible = Bible::new_from_json(&file_path).expect("Failed to load Bible JSON");

    let verse = bible
        .get_verse_by_reference("Gen 1:1")
        .expect("Verse not found");
    assert_eq!(
        verse.text(),
        "In the beginning God created the heaven and the earth."
    );

    let verse = bible
        .get_verse_by_reference("Jn 3:16")
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
        .starts_with("The grace of our Lord Jesus Christ be with you all"));
}

#[test]
fn test_get_verse_by_reference_invalid() {
    let file_path = match test_utils::get_kjv_json() {
        Some(p) => p,
        None => {
            println!("Skipping test_get_verse_by_reference_invalid: en_kjv.json not found");
            return;
        }
    };

    let bible = Bible::new_from_json(&file_path).expect("Failed to load Bible JSON");

    assert!(bible.get_verse_by_reference("Unknown 1:1").is_err());
    assert!(bible.get_verse_by_reference("Genesis 1").is_err());
}
