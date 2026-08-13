use std::{
    error::Error,
    panic::{catch_unwind, AssertUnwindSafe},
};

use bible_io::{
    Bible, BibleBook, BibleDataFormatErrorCode, BibleDataValidationOptions, BibleLoadOptions,
    BibleLoadPhase, BibleLocation, Book, Chapter, EditionReference, JsonMap, Language,
    SearchIndexMode, Verse,
};
use serde_json::json;

fn annotated_bible() -> Bible {
    Bible::from_json_value(json!({
        "schemaVersion": 1,
        "language": "English",
        "provider": {"slug": "example"},
        "metadata": {
            "id": "example-2026",
            "description": "Schema fixture",
            "translationName": "Example Translation",
            "customMetadata": {"revision": 2}
        },
        "bookOrder": ["ex", "gn"],
        "books": {
            "gn": {
                "name": "Genesis Local",
                "section": "Torah",
                "chapters": {
                    "7": {
                        "heading": "Creation",
                        "verses": {
                            "9": {"text": "In {the} beginning God created.", "paragraphStart": true}
                        }
                    }
                }
            },
            "ex": {
                "name": "Exodus Local",
                "chapters": {"2": {"4": "These are the names."}}
            }
        }
    }))
    .unwrap()
}

#[test]
fn versioned_schema_round_trips_order_sparse_numbers_and_annotations() {
    let bible = annotated_bible();
    assert_eq!(
        bible.books().iter().map(Book::book).collect::<Vec<_>>(),
        [BibleBook::Exodus, BibleBook::Genesis]
    );
    assert_eq!(
        bible.get_verse(BibleBook::Genesis, 7, 9).unwrap().text(),
        "In {the} beginning God created."
    );
    assert!(bible.get_chapter(BibleBook::Genesis, 1).is_err());
    assert_eq!(bible.annotations()["provider"], json!({"slug": "example"}));
    assert_eq!(
        bible.metadata().additional["customMetadata"],
        json!({"revision": 2})
    );

    let encoded = bible.to_json_value();
    assert_eq!(encoded["bookOrder"], json!(["ex", "gn"]));
    assert_eq!(
        encoded["books"]["gn"]["chapters"]["7"]["verses"]["9"]["paragraphStart"],
        true
    );
    assert_eq!(Bible::from_json_value(encoded).unwrap(), bible);
}

#[test]
fn absent_book_order_uses_canonical_order() {
    let bible = Bible::from_json_value(json!({
        "books": {
            "ex": {"chapters": {"1": {"1": "Exodus"}}},
            "gn": {"chapters": {"1": {"1": "Genesis"}}}
        }
    }))
    .unwrap();
    assert_eq!(
        bible.books().iter().map(Book::book).collect::<Vec<_>>(),
        [BibleBook::Genesis, BibleBook::Exodus]
    );
    assert_eq!(bible.to_json_value()["bookOrder"], json!(["gn", "ex"]));
}

#[test]
fn malformed_documents_return_path_aware_errors_without_panicking() {
    let io_error = Bible::from_path(
        "tests/fixtures/does-not-exist.json",
        BibleLoadOptions::default(),
    )
    .unwrap_err();
    assert!(io_error
        .source()
        .and_then(|cause| cause.downcast_ref::<std::io::Error>())
        .is_some());

    for document in [
        json!({"books": {"unknown": {"chapters": {"1": {"1": "x"}}}}}),
        json!({"books": {"gn": {"name": " ", "chapters": {"1": {"1": "x"}}}}}),
        json!({"books": {"gn": {"chapters": {"0": {"1": "x"}}}}}),
        json!({"books": {"gn": {"chapters": {"1": {"0": "x"}}}}}),
    ] {
        let result = catch_unwind(AssertUnwindSafe(|| Bible::from_json_value(document)));
        assert!(result.is_ok(), "malformed content must never panic");
        let error = result.unwrap().unwrap_err();
        assert_eq!(error.code(), BibleDataFormatErrorCode::InvalidValue);
        assert!(error.path().starts_with("$.books"));
    }

    let duplicate = Bible::from_json_value(json!({
        "books": {
            "gn": {"chapters": {"1": {"1": "first"}}},
            "GEN": {"chapters": {"1": {"1": "duplicate"}}}
        }
    }))
    .unwrap_err();
    assert_eq!(duplicate.path(), "$.books.GEN");

    let duplicate_number = Bible::from_json_value(json!({
        "books": {"gn": {"chapters": {
            "1": {"1": "first"},
            "01": {"1": "duplicate numeric chapter"}
        }}}
    }))
    .unwrap_err();
    assert_eq!(
        duplicate_number.code(),
        BibleDataFormatErrorCode::InvalidValue
    );
    assert_eq!(duplicate_number.path(), "$.books.gn.chapters[\"01\"]");

    let incomplete_order = Bible::from_json_value(json!({
        "bookOrder": ["gn"],
        "books": {
            "gn": {"chapters": {"1": {"1": "Genesis"}}},
            "ex": {"chapters": {"1": {"1": "Exodus"}}}
        }
    }))
    .unwrap_err();
    assert_eq!(incomplete_order.path(), "$.bookOrder");

    let ambiguous_titles = Bible::from_json_value(json!({
        "books": {
            "gn": {"name": "Same", "chapters": {"1": {"1": "Genesis"}}},
            "ex": {"name": " same ", "chapters": {"1": {"1": "Exodus"}}}
        }
    }))
    .unwrap_err();
    assert_eq!(ambiguous_titles.path(), "$.books");
}

#[test]
fn permissive_only_relaxes_skeletal_content() {
    let strict_error = Bible::from_json_value(json!({"books": {}})).unwrap_err();
    assert_eq!(strict_error.code(), BibleDataFormatErrorCode::InvalidValue);
    assert_eq!(strict_error.path(), "$.books");
    assert_eq!(strict_error.value(), Some(&json!({})));

    let options = BibleLoadOptions {
        validation: BibleDataValidationOptions::PERMISSIVE,
        search_index_mode: SearchIndexMode::Disabled,
    };
    let bible = Bible::from_json_value_with_options(json!({"books": {}}), options).unwrap();
    assert!(bible.books().is_empty());
    assert!(bible.search("anything").is_empty());
    assert!(!bible.has_search_index());

    let error = Bible::from_json_value_with_options(json!({"books": null}), options).unwrap_err();
    assert_eq!(error.code(), BibleDataFormatErrorCode::InvalidType);
    assert_eq!(error.path(), "$.books");

    // Permissive validation relaxes empty content, never structural types.
    let error = Bible::from_json_value_with_options(json!({"books": []}), options).unwrap_err();
    assert_eq!(error.code(), BibleDataFormatErrorCode::InvalidType);
    assert_eq!(error.path(), "$.books");
    assert_eq!(error.value(), Some(&json!([])));

    let empty_book =
        Bible::from_json_value_with_options(json!({"books": {"gn": {"chapters": {}}}}), options)
            .unwrap();
    assert_eq!(empty_book.stats().verses_per_book[&BibleBook::Genesis], 0);
}

#[test]
fn schema_version_and_explicit_null_structures_are_never_relaxed() {
    let unsupported = Bible::from_json_value(json!({
        "schemaVersion": 2,
        "books": {"gn": {"chapters": {"1": {"1": "text"}}}}
    }))
    .unwrap_err();
    assert_eq!(unsupported.code(), BibleDataFormatErrorCode::InvalidValue);
    assert_eq!(unsupported.path(), "$.schemaVersion");
    assert_eq!(unsupported.value(), Some(&json!(2)));

    let options = BibleLoadOptions {
        validation: BibleDataValidationOptions::PERMISSIVE,
        ..BibleLoadOptions::default()
    };
    let invalid_documents = [
        json!({"schemaVersion": null, "books": {}}),
        json!({"books": null}),
        json!({"books": {"gn": {"chapters": null}}}),
        json!({"books": {"gn": {"chapters": {"1": {"verses": null}}}}}),
        json!({"books": {"gn": {"chapters": {"1": {"1": {"text": null}}}}}}),
    ];
    for document in invalid_documents {
        let error = Bible::from_json_value_with_options(document, options).unwrap_err();
        assert_eq!(error.code(), BibleDataFormatErrorCode::InvalidType);
    }

    let mut null_order = annotated_bible().to_json_value();
    null_order["bookOrder"] = serde_json::Value::Null;
    let error = Bible::from_json_value_with_options(null_order, options).unwrap_err();
    assert_eq!(error.code(), BibleDataFormatErrorCode::InvalidType);
    assert_eq!(error.path(), "$.bookOrder");
}

#[test]
fn file_loading_reports_stable_progress_phases() {
    assert!(bible_io::BibleLoadProgress::new(BibleLoadPhase::Reading, f32::NAN, 0.0).is_err());
    assert!(bible_io::BibleLoadProgress::new(BibleLoadPhase::Reading, 0.5, 1.1).is_err());
    let mut progress = Vec::new();
    let bible = Bible::from_path_with_progress(
        "tests/fixtures/en_kjv.json",
        BibleLoadOptions::default(),
        |value| progress.push(value),
    )
    .unwrap();
    assert_eq!(bible.id(), "kjv");
    assert_eq!(
        progress.iter().map(|value| value.phase).collect::<Vec<_>>(),
        [
            BibleLoadPhase::Reading,
            BibleLoadPhase::Reading,
            BibleLoadPhase::Processing,
            BibleLoadPhase::Processing,
            BibleLoadPhase::Complete,
        ]
    );
    assert_eq!(
        progress
            .iter()
            .map(|value| value.fraction)
            .collect::<Vec<_>>(),
        [0.0, 0.65, 0.65, 1.0, 1.0]
    );
    assert_eq!(
        progress
            .iter()
            .map(|value| value.phase_fraction)
            .collect::<Vec<_>>(),
        [0.0, 1.0, 0.0, 1.0, 1.0]
    );
}

#[test]
fn legacy_array_shape_remains_loadable_and_serializes_as_maps() {
    let bible = Bible::from_json_value(json!({
        "books": {"gn": {"name": "Genesis", "chapters": [["one", "two"], ["three"]]}}
    }))
    .unwrap();
    assert_eq!(
        bible.get_verse(BibleBook::Genesis, 2, 1).unwrap().text(),
        "three"
    );
    assert_eq!(
        bible.to_json_value()["books"]["gn"]["chapters"]["1"]["2"],
        "two"
    );
}

#[test]
fn navigation_ranges_and_keys_follow_edition_order() {
    let bible = annotated_bible();
    let exodus = BibleLocation::new(BibleBook::Exodus, 2, Some(4)).unwrap();
    let genesis = BibleLocation::new(BibleBook::Genesis, 7, Some(9)).unwrap();
    assert_eq!(bible.next_verse(exodus).unwrap(), Some(genesis));
    assert_eq!(bible.previous_verse(genesis).unwrap(), Some(exodus));
    assert!(bible.has_next_verse(exodus).unwrap());
    assert!(!bible.has_next_verse(genesis).unwrap());

    let verses = bible
        .get_verse_range_by_reference("Exodus Local 2:4-Genesis Local 7:9")
        .unwrap();
    assert_eq!(verses.len(), 2);
    let parsed = bible
        .parse_reference("Exodus Local 2:4-Genesis Local 7:9")
        .unwrap();
    assert!(matches!(parsed, EditionReference::Range(_)));
    assert_eq!(bible.resolve_edition_reference(parsed).unwrap(), verses);
    assert!(bible
        .parse_reference_detailed("Exodus Local 2:4-Genesis Local 7:9")
        .unwrap()
        .metadata()
        .is_none());
    assert!(matches!(
        bible
            .parse_reference_with_language("Exodus Local 2:4-Genesis Local 7:9", Language::English,)
            .unwrap(),
        EditionReference::Range(_)
    ));
    let passage = bible
        .parse_passage("Exodus Local 2:4-Genesis Local 7:9")
        .unwrap();
    assert_eq!(bible.resolve_edition_passage(&passage).unwrap(), verses);
    assert_eq!(
        bible
            .get_passage("Exodus Local 2:4-Genesis Local 7:9")
            .unwrap(),
        verses
    );
    assert_eq!(
        bible.key_for_location(genesis).unwrap().edition_id(),
        "example-2026"
    );
    assert_eq!(
        bible
            .key_for_verse(bible.get_verse(BibleBook::Genesis, 7, 9).unwrap())
            .unwrap()
            .edition_id(),
        "example-2026"
    );
    assert_eq!(bible.format_location(genesis).unwrap(), "Genesis Local 7:9");
}

#[test]
fn rich_passages_preserve_selection_and_sequence_duplicates() {
    let bible = Bible::from_json_value(json!({
        "language": "English",
        "books": {
            "gn": {"chapters": {"1": {"1": "g11", "2": "g12"}, "2": {"1": "g21"}}},
            "ex": {"chapters": {"1": {"1": "e11", "2": "e12"}}}
        }
    }))
    .unwrap();
    assert_eq!(
        bible
            .get_passage("Genesis 1:1,1-2; Genesis 1:2")
            .unwrap()
            .iter()
            .map(|verse| verse.number())
            .collect::<Vec<_>>(),
        [1, 1, 2, 2]
    );
    assert!(bible.get_passage("Genesis 1-3").is_err());
}

#[test]
fn model_values_are_sorted_validated_copyable_and_unicode_aware() {
    let verses = vec![
        Verse::checked(BibleBook::Genesis, 4, 9, "κόσμος 神创造", JsonMap::new()).unwrap(),
        Verse::checked(BibleBook::Genesis, 4, 2, "Second", JsonMap::new()).unwrap(),
    ];
    let chapter = Chapter::checked(BibleBook::Genesis, 4, verses, JsonMap::new()).unwrap();
    assert_eq!(
        chapter
            .verses()
            .iter()
            .map(Verse::number)
            .collect::<Vec<_>>(),
        [2, 9]
    );
    assert!(chapter.get_verse(1).is_none());
    assert!(chapter.get_verse(9).unwrap().contains_word("ΚΌΣΜΟΣ"));
    assert!(chapter.get_verse(9).unwrap().contains_text("创造"));
    assert!(!chapter.get_verse(9).unwrap().contains_word("创造"));

    let book = Book::checked(BibleBook::Genesis, "Genesis", vec![chapter], JsonMap::new()).unwrap();
    assert_eq!(book.verse_count(), 2);
    assert_eq!(book.stats().chapter_count, 1);
    assert_eq!(
        book.with_title("Genesis custom").unwrap().title(),
        "Genesis custom"
    );
    let bible = Bible::from_books(
        vec![book],
        bible_io::Language::English,
        bible_io::BibleMetadata::default(),
        JsonMap::new(),
        SearchIndexMode::Disabled,
    )
    .unwrap();
    assert_eq!(bible.language(), "English");
    assert_eq!(bible.language_code(), Some("en"));
}
