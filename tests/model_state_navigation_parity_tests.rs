use std::{
    collections::{hash_map::DefaultHasher, HashSet},
    hash::{Hash, Hasher},
    path::PathBuf,
    sync::OnceLock,
};

use bible_io::{
    Bible, BibleBook, BibleDataFormatErrorCode, BibleError, BibleLocation, BibleMetadata,
    BibleVerseKey, Book, Chapter, JsonMap, Language, SearchIndexMode, Verse, VerseRef,
};
use serde_json::{json, Value};

fn verse(book: BibleBook, chapter: usize, number: usize, text: &str) -> Verse {
    Verse::checked(book, chapter, number, text, JsonMap::new()).unwrap()
}

fn chapter(book: BibleBook, number: usize, verses: Vec<Verse>) -> Chapter {
    Chapter::checked(book, number, verses, JsonMap::new()).unwrap()
}

fn book(book: BibleBook, title: &str, chapters: Vec<Chapter>) -> Book {
    Book::checked(book, title, chapters, JsonMap::new()).unwrap()
}

fn bible(books: Vec<Book>) -> Bible {
    Bible::from_books(
        books,
        Language::English,
        BibleMetadata::default(),
        JsonMap::new(),
        SearchIndexMode::Disabled,
    )
    .unwrap()
}

fn hash_of<T: Hash>(value: &T) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

fn kjv() -> &'static Bible {
    static KJV: OnceLock<Bible> = OnceLock::new();
    KJV.get_or_init(|| {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("en_kjv.json");
        Bible::new(path.to_str().expect("fixture path must be UTF-8"))
            .expect("the repository KJV fixture must load")
    })
}

#[test]
fn checked_model_constructors_enforce_positive_coordinates_and_parents() {
    assert!(Verse::checked(BibleBook::Genesis, 0, 1, "text", JsonMap::new()).is_err());
    assert!(Verse::checked(BibleBook::Genesis, 1, 0, "text", JsonMap::new()).is_err());
    assert!(Chapter::checked(BibleBook::Genesis, 0, vec![], JsonMap::new()).is_err());

    let wrong_book = verse(BibleBook::Exodus, 1, 1, "wrong book");
    assert!(Chapter::checked(BibleBook::Genesis, 1, vec![wrong_book], JsonMap::new()).is_err());
    let wrong_chapter = verse(BibleBook::Genesis, 2, 1, "wrong chapter");
    assert!(Chapter::checked(BibleBook::Genesis, 1, vec![wrong_chapter], JsonMap::new()).is_err());

    let first = verse(BibleBook::Genesis, 1, 3, "first");
    let duplicate = verse(BibleBook::Genesis, 1, 3, "duplicate");
    assert!(Chapter::checked(
        BibleBook::Genesis,
        1,
        vec![first, duplicate],
        JsonMap::new()
    )
    .is_err());

    let exodus = chapter(
        BibleBook::Exodus,
        1,
        vec![verse(BibleBook::Exodus, 1, 1, "Exodus")],
    );
    assert!(Book::checked(BibleBook::Genesis, "Genesis", vec![exodus], JsonMap::new()).is_err());
    assert!(Book::checked(BibleBook::Genesis, "  ", vec![], JsonMap::new()).is_err());

    let chapter_one = chapter(
        BibleBook::Genesis,
        1,
        vec![verse(BibleBook::Genesis, 1, 1, "one")],
    );
    let duplicate_chapter = chapter(
        BibleBook::Genesis,
        1,
        vec![verse(BibleBook::Genesis, 1, 2, "two")],
    );
    assert!(Book::checked(
        BibleBook::Genesis,
        "Genesis",
        vec![chapter_one, duplicate_chapter],
        JsonMap::new()
    )
    .is_err());

    assert!(Book::try_new("unknown", "Unknown", vec![]).is_err());
    assert_eq!(
        Book::try_new("GEN", "Genesis", vec![]).unwrap().book(),
        BibleBook::Genesis
    );
}

#[test]
fn sparse_children_are_sorted_and_looked_up_by_declared_number() {
    let verse_two = verse(BibleBook::Genesis, 4, 2, "Second");
    let verse_nine = verse(BibleBook::Genesis, 4, 9, "Ninth");
    let mut original_verses = vec![verse_nine.clone(), verse_two.clone()];
    let chapter_four = Chapter::checked(
        BibleBook::Genesis,
        4,
        original_verses.clone(),
        JsonMap::new(),
    )
    .unwrap();
    assert_eq!(
        chapter_four
            .verses()
            .iter()
            .map(Verse::number)
            .collect::<Vec<_>>(),
        [2, 9]
    );
    assert_eq!(chapter_four.get_verse(2), Some(&verse_two));
    assert_eq!(chapter_four.get_verse(9), Some(&verse_nine));
    assert!(chapter_four.get_verse(1).is_none());
    assert_eq!(original_verses, [verse_nine, verse_two]);
    original_verses.clear();
    assert_eq!(
        chapter_four.verses(),
        [
            verse(BibleBook::Genesis, 4, 2, "Second"),
            verse(BibleBook::Genesis, 4, 9, "Ninth"),
        ]
    );

    let chapter_two = chapter(
        BibleBook::Genesis,
        2,
        vec![verse(BibleBook::Genesis, 2, 5, "Chapter two")],
    );
    let chapter_seven = chapter(
        BibleBook::Genesis,
        7,
        vec![verse(BibleBook::Genesis, 7, 11, "Chapter seven")],
    );
    let mut original_chapters = vec![chapter_seven.clone(), chapter_two.clone()];
    let genesis = Book::checked(
        BibleBook::Genesis,
        "Genesis",
        original_chapters.clone(),
        JsonMap::new(),
    )
    .unwrap();
    assert_eq!(
        genesis
            .chapters()
            .iter()
            .map(Chapter::number)
            .collect::<Vec<_>>(),
        [2, 7]
    );
    assert_eq!(genesis.get_chapter(2).unwrap(), &chapter_two);
    assert_eq!(genesis.get_chapter(7).unwrap(), &chapter_seven);
    assert_eq!(
        genesis.get_verses(7).unwrap(),
        [verse(BibleBook::Genesis, 7, 11, "Chapter seven")]
    );
    assert_eq!(genesis.get_verse(2, 5).unwrap().text(), "Chapter two");
    assert!(matches!(
        genesis.get_chapter(1),
        Err(BibleError::ChapterOutOfBounds { chapter: 1, .. })
    ));
    assert_eq!(original_chapters, [chapter_seven, chapter_two]);
    original_chapters.clear();
    assert_eq!(
        genesis
            .chapters()
            .iter()
            .map(Chapter::number)
            .collect::<Vec<_>>(),
        [2, 7]
    );
}

#[test]
fn verse_unicode_word_and_substring_semantics_match_the_dart_matrix() {
    let value = verse(
        BibleBook::Genesis,
        1,
        1,
        "κόσμος κοσμικός בְּרֵאשִׁית God's 神创造天地",
    );

    assert!(value.contains_word("ΚΌΣΜΟΣ"));
    assert!(!value.contains_word("κόσ"));
    assert!(value.contains_word("בְּרֵאשִׁית"));
    assert!(!value.contains_word("god's"));
    assert!(value.contains_word("God"));
    assert!(!value.contains_word("创造"));
    assert!(!value.contains_word(""));
    assert!(!value.contains_word("two words"));
    assert!(value.contains_text("κόσ"));
    assert!(value.contains_text("创造"));
    assert!(!value.contains_text(""));
}

#[test]
fn verse_exposes_value_safe_location_and_reference_conversion() {
    let value = verse(BibleBook::Genesis, 2, 3, "text");
    assert_eq!(
        value.location(),
        BibleLocation::new(BibleBook::Genesis, 2, Some(3)).unwrap()
    );
    assert_eq!(
        value.to_verse_ref().unwrap(),
        VerseRef::new(BibleBook::Genesis, 2, 3).unwrap()
    );
}

#[test]
fn verse_annotations_are_owned_deep_values_and_json_shape_is_compatible() {
    let mut source = JsonMap::new();
    source.insert("heading".to_string(), json!("Creation"));
    source.insert("layout".to_string(), json!({"lines": ["first"]}));
    let value =
        Verse::checked(BibleBook::Genesis, 1, 1, "In the beginning", source.clone()).unwrap();

    source.insert("heading".to_string(), json!("Changed"));
    source
        .get_mut("layout")
        .unwrap()
        .as_object_mut()
        .unwrap()
        .insert("lines".to_string(), json!(["changed"]));

    assert_eq!(value.annotations()["heading"], json!("Creation"));
    assert_eq!(
        value.to_json_value(),
        json!({
            "text": "In the beginning",
            "heading": "Creation",
            "layout": {"lines": ["first"]}
        })
    );
    assert_eq!(
        verse(BibleBook::Genesis, 1, 2, "plain").to_json_value(),
        json!("plain")
    );
}

#[test]
fn structural_annotations_are_reserved_at_every_model_level() {
    let mut verse_annotations = JsonMap::new();
    verse_annotations.insert("text".to_string(), json!("replacement"));
    assert!(Verse::checked(BibleBook::Genesis, 1, 1, "text", verse_annotations).is_err());

    let mut chapter_annotations = JsonMap::new();
    chapter_annotations.insert("verses".to_string(), json!([]));
    assert!(Chapter::checked(BibleBook::Genesis, 1, vec![], chapter_annotations).is_err());

    for reserved in ["name", "chapters"] {
        let mut annotations = JsonMap::new();
        annotations.insert(reserved.to_string(), Value::Null);
        assert!(Book::checked(BibleBook::Genesis, "Genesis", vec![], annotations).is_err());
    }

    let mut root_annotations = JsonMap::new();
    root_annotations.insert("books".to_string(), json!({}));
    let error = Bible::from_books(
        vec![],
        Language::English,
        BibleMetadata::default(),
        root_annotations,
        SearchIndexMode::Disabled,
    )
    .unwrap_err();
    assert_eq!(error.code(), BibleDataFormatErrorCode::ReservedField);
    assert_eq!(error.path(), "$");
}

#[test]
fn model_values_clone_copy_validate_and_hash_structurally() {
    let mut annotations_one = JsonMap::new();
    annotations_one.insert("z".to_string(), json!({"nested": [1, 2]}));
    annotations_one.insert("a".to_string(), json!(true));
    let mut annotations_two = JsonMap::new();
    annotations_two.insert("a".to_string(), json!(true));
    annotations_two.insert("z".to_string(), json!({"nested": [1, 2]}));

    let first =
        Verse::checked(BibleBook::Genesis, 1, 1, "κόσμος", annotations_one.clone()).unwrap();
    let equal = Verse::checked(BibleBook::Genesis, 1, 1, "κόσμος", annotations_two).unwrap();
    assert_eq!(first, equal);
    assert_eq!(hash_of(&first), hash_of(&equal));
    assert_eq!(
        first.copy_with(None, None, None, None, None).unwrap(),
        first
    );
    assert_eq!(first.with_text("updated").text(), "updated");
    assert!(first
        .copy_with(None, None, None, None, Some(JsonMap::new()))
        .unwrap()
        .annotations()
        .is_empty());
    assert!(first.copy_with(None, None, Some(0), None, None).is_err());

    annotations_one.insert("a".to_string(), json!(false));
    assert_eq!(first.annotations().get("a"), Some(&json!(true)));
    assert_eq!(
        first.to_json_value(),
        json!({"text": "κόσμος", "z": {"nested": [1, 2]}, "a": true})
    );

    let first_chapter =
        Chapter::checked(BibleBook::Genesis, 1, vec![first.clone()], JsonMap::new()).unwrap();
    let equal_chapter =
        Chapter::checked(BibleBook::Genesis, 1, vec![equal], JsonMap::new()).unwrap();
    assert_eq!(first_chapter, equal_chapter);
    assert_eq!(hash_of(&first_chapter), hash_of(&equal_chapter));
    assert!(first_chapter.copy_with(None, Some(0), None, None).is_err());

    let first_book = book(BibleBook::Genesis, "Genesis", vec![first_chapter]);
    let equal_book = book(BibleBook::Genesis, "Genesis", vec![equal_chapter]);
    assert_eq!(first_book, equal_book);
    assert_eq!(hash_of(&first_book), hash_of(&equal_book));
    assert_eq!(
        first_book.with_title("Beginning").unwrap().title(),
        "Beginning"
    );
    assert!(first_book.with_title(" ").is_err());
    assert!(first_book
        .copy_with(Some(BibleBook::Exodus), None, None, None)
        .is_err());

    let original_bible = bible(vec![first_book]);
    let cloned_bible = original_bible.clone();
    let copied_bible = original_bible
        .copy_with(None, None, None, None, None)
        .unwrap();
    assert_eq!(original_bible, cloned_bible);
    assert_eq!(original_bible, copied_bible);
    assert_eq!(hash_of(&original_bible), hash_of(&cloned_bible));
}

#[test]
fn annotated_chapter_and_book_have_deep_value_hash_copy_and_json_semantics() {
    let mut chapter_annotations = JsonMap::new();
    chapter_annotations.insert("heading".to_string(), json!({"kind": "major"}));
    let first_chapter = Chapter::checked(
        BibleBook::Genesis,
        1,
        vec![verse(BibleBook::Genesis, 1, 1, "text")],
        chapter_annotations.clone(),
    )
    .unwrap();
    let equal_chapter = Chapter::checked(
        BibleBook::Genesis,
        1,
        vec![verse(BibleBook::Genesis, 1, 1, "text")],
        chapter_annotations.clone(),
    )
    .unwrap();

    let mut book_annotations = JsonMap::new();
    book_annotations.insert("aliases".to_string(), json!(["Beginning"]));
    let first_book = Book::checked(
        BibleBook::Genesis,
        "Genesis",
        vec![first_chapter.clone()],
        book_annotations.clone(),
    )
    .unwrap();
    let equal_book = Book::checked(
        BibleBook::Genesis,
        "Genesis",
        vec![equal_chapter.clone()],
        book_annotations.clone(),
    )
    .unwrap();

    chapter_annotations
        .get_mut("heading")
        .unwrap()
        .as_object_mut()
        .unwrap()
        .insert("kind".to_string(), json!("changed"));
    book_annotations.insert("aliases".to_string(), json!(["Changed"]));

    assert_eq!(first_chapter, equal_chapter);
    assert_eq!(hash_of(&first_chapter), hash_of(&equal_chapter));
    assert_eq!(first_book, equal_book);
    assert_eq!(hash_of(&first_book), hash_of(&equal_book));
    assert_eq!(
        first_book.to_json_value(),
        json!({
            "aliases": ["Beginning"],
            "name": "Genesis",
            "chapters": {
                "1": {
                    "heading": {"kind": "major"},
                    "verses": {"1": "text"}
                }
            }
        })
    );
    assert_eq!(first_book.annotations()["aliases"], json!(["Beginning"]));
    assert!(first_chapter
        .copy_with(None, None, None, Some(JsonMap::new()))
        .unwrap()
        .annotations()
        .is_empty());
    assert_eq!(
        first_book.with_title("Genesis custom").unwrap().title(),
        "Genesis custom"
    );
}

#[test]
fn bible_construction_defensively_owns_books_and_rejects_duplicates() {
    let genesis = book(
        BibleBook::Genesis,
        "Genesis",
        vec![chapter(
            BibleBook::Genesis,
            1,
            vec![verse(BibleBook::Genesis, 1, 1, "Stable text")],
        )],
    );
    let mut input = vec![genesis.clone()];
    let loaded = bible(input.clone());
    input.clear();
    assert_eq!(loaded.books().len(), 1);
    assert_eq!(
        loaded.get_book(BibleBook::Genesis).unwrap().chapters(),
        genesis.chapters()
    );
    assert_eq!(loaded.search("stable").len(), 1);

    let error = Bible::from_books(
        vec![genesis.clone(), genesis],
        Language::English,
        BibleMetadata::default(),
        JsonMap::new(),
        SearchIndexMode::Disabled,
    )
    .unwrap_err();
    assert_eq!(error.code(), BibleDataFormatErrorCode::InvalidValue);
    assert_eq!(error.path(), "$.books");
}

#[test]
fn legacy_json_accepts_all_book_identifier_forms_and_rejects_alias_duplicates() {
    for identifier in ["gn", "Gen", "GEN", "Genesis"] {
        let mut books = JsonMap::new();
        books.insert(
            identifier.to_string(),
            json!({"chapters": {"1": {"1": identifier}}}),
        );
        let mut document = JsonMap::new();
        document.insert("books".to_string(), Value::Object(books));
        let loaded = Bible::from_json_value(Value::Object(document)).unwrap();
        assert_eq!(
            loaded.get_verse(BibleBook::Genesis, 1, 1).unwrap().text(),
            identifier
        );
    }

    let mut books = JsonMap::new();
    books.insert("gn".to_string(), json!({"chapters": {"1": {"1": "first"}}}));
    books.insert(
        "GEN".to_string(),
        json!({"chapters": {"1": {"1": "duplicate"}}}),
    );
    let error = Bible::from_json_value(json!({"books": books})).unwrap_err();
    assert_eq!(error.code(), BibleDataFormatErrorCode::InvalidValue);
    assert_eq!(error.path(), "$.books.GEN");
}

#[test]
fn location_is_copyable_validated_and_has_strict_serde_conversions() {
    fn requires_copy<T: Copy>(_: T) {}

    assert!(BibleLocation::new(BibleBook::Genesis, 0, None).is_err());
    assert!(BibleLocation::new(BibleBook::Genesis, 1, Some(0)).is_err());
    let original = BibleLocation::new(BibleBook::Genesis, 1, Some(2)).unwrap();
    requires_copy(original);
    assert_eq!(original.with_chapter(3).unwrap().verse(), Some(2));
    assert_eq!(
        original.with_verse(None).unwrap(),
        BibleLocation::new(BibleBook::Genesis, 1, None).unwrap()
    );
    assert!(original.with_verse(Some(0)).is_err());
    assert_eq!(original.reference(), "Genesis 1:2");
    assert_eq!(
        original.to_verse_ref().unwrap(),
        VerseRef::new(BibleBook::Genesis, 1, 2).unwrap()
    );
    assert!(original.to_passage().is_ok());

    let json_value = serde_json::to_value(original).unwrap();
    assert_eq!(json_value, json!({"book": "gn", "chapter": 1, "verse": 2}));
    assert_eq!(
        serde_json::from_value::<BibleLocation>(json_value).unwrap(),
        original
    );
    assert_eq!(
        serde_json::from_value::<BibleLocation>(json!({
            "book": "Genesis",
            "chapter": 1
        }))
        .unwrap(),
        BibleLocation::new(BibleBook::Genesis, 1, None).unwrap()
    );
    for malformed in [
        json!({"book": "gn", "chapter": "1"}),
        json!({"book": "unknown", "chapter": 1}),
        json!({"book": "gn", "chapter": 0}),
        json!({"book": "gn", "chapter": 1, "verse": 0}),
        json!({"book": "gn", "chapter": 1, "verse": "2"}),
    ] {
        assert!(serde_json::from_value::<BibleLocation>(malformed).is_err());
    }

    let too_large =
        BibleLocation::new(BibleBook::Genesis, usize::from(u16::MAX) + 1, Some(1)).unwrap();
    assert!(too_large.to_verse_ref().is_err());
}

#[test]
fn verse_keys_are_edition_aware_hashable_and_reject_malformed_state() {
    let location = BibleLocation::new(BibleBook::Genesis, 1, Some(1)).unwrap();
    let key = BibleVerseKey::new("eng-kjv-1769", location).unwrap();
    let restored: BibleVerseKey =
        serde_json::from_value(serde_json::to_value(&key).unwrap()).unwrap();
    assert_eq!(restored, key);
    assert_eq!(hash_of(&restored), hash_of(&key));
    assert_eq!(
        key.to_verse_ref().unwrap(),
        VerseRef::new(BibleBook::Genesis, 1, 1).unwrap()
    );
    assert_eq!(
        BibleVerseKey::from_verse("eng-kjv-1769", &verse(BibleBook::Genesis, 1, 1, "text"))
            .unwrap(),
        key
    );
    assert_eq!(
        key.with_edition_id("eng-web").unwrap().edition_id(),
        "eng-web"
    );
    assert_ne!(key.with_edition_id("eng-web").unwrap(), key);
    assert_eq!(key.with_location(location).unwrap(), key);

    assert!(BibleVerseKey::new(" ", location).is_err());
    assert!(BibleVerseKey::new(" eng-kjv-1769 ", location).is_err());
    assert!(BibleVerseKey::new(
        "eng-kjv-1769",
        BibleLocation::new(BibleBook::Genesis, 1, None).unwrap()
    )
    .is_err());
    for malformed in [
        json!({
            "editionId": "eng-kjv-1769",
            "location": {"book": "gn", "chapter": 1}
        }),
        json!({
            "editionId": 1,
            "location": {"book": "gn", "chapter": 1, "verse": 1}
        }),
        json!({
            "editionId": "eng-kjv-1769",
            "location": {"book": "unknown", "chapter": 1, "verse": 1}
        }),
    ] {
        assert!(serde_json::from_value::<BibleVerseKey>(malformed).is_err());
    }

    let mut keys = HashSet::new();
    assert!(keys.insert(key.clone()));
    assert!(!keys.insert(key));
}

#[test]
fn sparse_navigation_crosses_empty_chapters_and_books_and_reports_errors() {
    let genesis = book(
        BibleBook::Genesis,
        "Genesis",
        vec![
            chapter(BibleBook::Genesis, 7, vec![]),
            chapter(
                BibleBook::Genesis,
                2,
                vec![
                    verse(BibleBook::Genesis, 2, 5, "five"),
                    verse(BibleBook::Genesis, 2, 2, "two"),
                ],
            ),
        ],
    );
    let exodus = book(BibleBook::Exodus, "Exodus", vec![]);
    let leviticus = book(
        BibleBook::Leviticus,
        "Leviticus",
        vec![chapter(
            BibleBook::Leviticus,
            3,
            vec![verse(BibleBook::Leviticus, 3, 9, "nine")],
        )],
    );
    let loaded = bible(vec![genesis, exodus, leviticus]);

    let genesis_two = BibleLocation::new(BibleBook::Genesis, 2, None).unwrap();
    let genesis_seven = BibleLocation::new(BibleBook::Genesis, 7, None).unwrap();
    let leviticus_three = BibleLocation::new(BibleBook::Leviticus, 3, None).unwrap();
    assert_eq!(
        loaded.next_chapter(genesis_two).unwrap(),
        Some(genesis_seven)
    );
    assert_eq!(
        loaded.next_chapter(genesis_seven).unwrap(),
        Some(leviticus_three)
    );
    assert_eq!(
        loaded.previous_chapter(leviticus_three).unwrap(),
        Some(genesis_seven)
    );
    assert_eq!(loaded.previous_chapter(genesis_two).unwrap(), None);
    assert_eq!(loaded.next_chapter(leviticus_three).unwrap(), None);
    assert!(loaded.has_next_chapter(genesis_seven).unwrap());
    assert!(!loaded.has_previous_chapter(genesis_two).unwrap());

    let genesis_two_two = BibleLocation::new(BibleBook::Genesis, 2, Some(2)).unwrap();
    let genesis_two_five = BibleLocation::new(BibleBook::Genesis, 2, Some(5)).unwrap();
    let leviticus_three_nine = BibleLocation::new(BibleBook::Leviticus, 3, Some(9)).unwrap();
    assert_eq!(
        loaded.next_verse(genesis_two_two).unwrap(),
        Some(genesis_two_five)
    );
    assert_eq!(
        loaded.next_verse(genesis_two_five).unwrap(),
        Some(leviticus_three_nine)
    );
    assert_eq!(
        loaded.previous_verse(leviticus_three_nine).unwrap(),
        Some(genesis_two_five)
    );
    assert_eq!(loaded.previous_verse(genesis_two_two).unwrap(), None);
    assert_eq!(loaded.next_verse(leviticus_three_nine).unwrap(), None);

    assert!(matches!(
        loaded.next_chapter(BibleLocation::new(BibleBook::Exodus, 1, None).unwrap()),
        Err(BibleError::ChapterOutOfBounds { .. })
    ));
    assert!(matches!(
        loaded.next_chapter(BibleLocation::new(BibleBook::Numbers, 1, None).unwrap()),
        Err(BibleError::BookNotFound { .. })
    ));
    assert!(matches!(
        loaded.next_verse(genesis_two),
        Err(BibleError::VerseRequired)
    ));
    assert!(matches!(
        loaded.next_verse(BibleLocation::new(BibleBook::Genesis, 2, Some(1)).unwrap()),
        Err(BibleError::VerseOutOfBounds { .. })
    ));

    let empty = bible(vec![]);
    assert!(empty.books().is_empty());
    assert!(matches!(
        empty.next_chapter(genesis_two),
        Err(BibleError::BookNotFound { .. })
    ));
}

#[test]
fn edition_keys_validate_membership_and_require_an_edition_id() {
    let loaded_verse = verse(BibleBook::Genesis, 1, 1, "loaded");
    let genesis = book(
        BibleBook::Genesis,
        "Genesis",
        vec![chapter(BibleBook::Genesis, 1, vec![loaded_verse.clone()])],
    );
    let metadata = BibleMetadata {
        id: Some("eng-test".to_string()),
        ..BibleMetadata::default()
    };
    let loaded = Bible::from_books(
        vec![genesis.clone()],
        Language::English,
        metadata,
        JsonMap::new(),
        SearchIndexMode::Disabled,
    )
    .unwrap();
    assert_eq!(
        loaded.key_for_verse(&loaded_verse).unwrap().edition_id(),
        "eng-test"
    );
    assert_eq!(
        loaded
            .key_for_location(loaded_verse.location())
            .unwrap()
            .location(),
        loaded_verse.location()
    );
    assert!(matches!(
        loaded.key_for_verse(&loaded_verse.with_text("different")),
        Err(BibleError::InvalidReference { .. })
    ));

    let without_id = bible(vec![genesis]);
    assert_eq!(
        without_id.key_for_verse(&loaded_verse),
        Err(BibleError::MissingEditionId)
    );
}

#[test]
fn full_kjv_graph_and_boundary_access_match_the_canonical_fixture() {
    let loaded = kjv();
    assert_eq!(loaded.books().len(), 66);
    assert_eq!(loaded.stats().book_count, 66);
    assert_eq!(loaded.stats().chapter_count, 1_189);
    // This repository's bundled KJV fixture contains 31,100 verse entries
    // (counted directly from its raw JSON graph).
    assert_eq!(loaded.stats().verse_count, 31_100);

    let mut loaded_book_ids = HashSet::new();
    for (position, loaded_book) in loaded.books().iter().enumerate() {
        assert!(loaded_book_ids.insert(loaded_book.book()));
        assert!(!loaded_book.title().is_empty());
        assert!(!loaded_book.chapters().is_empty());
        assert_eq!(loaded.get_book_by_id(position + 1).unwrap(), loaded_book);
        for (chapter_index, loaded_chapter) in loaded_book.chapters().iter().enumerate() {
            assert_eq!(loaded_chapter.book(), loaded_book.book());
            assert_eq!(loaded_chapter.number(), chapter_index + 1);
            assert!(!loaded_chapter.verses().is_empty());
            for (verse_index, loaded_verse) in loaded_chapter.verses().iter().enumerate() {
                assert_eq!(loaded_verse.book(), loaded_book.book());
                assert_eq!(loaded_verse.chapter(), loaded_chapter.number());
                assert_eq!(loaded_verse.number(), verse_index + 1);
                assert!(!loaded_verse.text().is_empty());
                assert_eq!(
                    loaded
                        .get_verse(
                            loaded_verse.book(),
                            loaded_verse.chapter(),
                            loaded_verse.number()
                        )
                        .unwrap(),
                    loaded_verse
                );
            }
        }
    }

    assert_eq!(loaded.get_book_by_id(1).unwrap().title(), "Genesis");
    assert_eq!(loaded.get_book_by_id(2).unwrap().title(), "Exodus");
    assert_eq!(loaded.get_book_by_id(66).unwrap().title(), "Revelation");
    assert_eq!(
        loaded
            .get_book(BibleBook::Genesis)
            .unwrap()
            .chapters()
            .len(),
        50
    );
    let loaded_titles = loaded
        .books()
        .iter()
        .map(Book::title)
        .collect::<HashSet<_>>();
    for expected in ["Genesis", "Exodus", "John", "Revelation"] {
        assert!(loaded_titles.contains(expected));
    }
    assert!(matches!(
        loaded.get_book_by_id(0),
        Err(BibleError::BookNotFound { .. })
    ));
    assert!(matches!(
        loaded.get_book_by_id(67),
        Err(BibleError::BookNotFound { .. })
    ));
    assert_eq!(
        loaded
            .get_chapter(BibleBook::Genesis, 1)
            .unwrap()
            .verses()
            .len(),
        31
    );
    let genesis_one_verses = loaded.get_verses(BibleBook::Genesis, 1).unwrap();
    assert_eq!(genesis_one_verses.len(), 31);
    assert_eq!(genesis_one_verses.first().unwrap().number(), 1);
    assert_eq!(genesis_one_verses.last().unwrap().number(), 31);
    assert_eq!(
        loaded
            .get_chapter(BibleBook::Revelation, 22)
            .unwrap()
            .number(),
        22
    );
    assert!(matches!(
        loaded.get_chapter(BibleBook::Genesis, 51),
        Err(BibleError::ChapterOutOfBounds { .. })
    ));
    assert!(matches!(
        loaded.get_verse(BibleBook::Genesis, 1, 32),
        Err(BibleError::VerseOutOfBounds { .. })
    ));
    assert!(loaded
        .get_verse(BibleBook::Genesis, 1, 1)
        .unwrap()
        .text()
        .starts_with("In the beginning"));
    assert_eq!(
        loaded.get_verse_by_reference("Genesis 1:1").unwrap().text(),
        "In the beginning God created the heaven and the earth."
    );
    let same_chapter = loaded
        .get_verse_range_by_reference("Genesis 1:1-3")
        .unwrap();
    assert_eq!(same_chapter.len(), 3);
    assert_eq!(same_chapter.first().unwrap().number(), 1);
    assert_eq!(same_chapter.last().unwrap().number(), 3);

    let cross_chapter = loaded
        .get_verse_range_by_reference("Genesis 1:3-2:2")
        .unwrap();
    assert_eq!(cross_chapter.len(), 31);
    assert_eq!(
        (cross_chapter[0].chapter(), cross_chapter[0].number()),
        (1, 3)
    );
    assert_eq!(
        (cross_chapter[28].chapter(), cross_chapter[28].number()),
        (1, 31)
    );
    assert_eq!(
        (cross_chapter[29].chapter(), cross_chapter[29].number()),
        (2, 1)
    );
    assert_eq!(
        (cross_chapter[30].chapter(), cross_chapter[30].number()),
        (2, 2)
    );
    assert_eq!(
        loaded
            .get_verse(BibleBook::Revelation, 22, 21)
            .unwrap()
            .number(),
        21
    );

    let first_chapter = BibleLocation::new(BibleBook::Genesis, 1, None).unwrap();
    let genesis_second_chapter = BibleLocation::new(BibleBook::Genesis, 2, None).unwrap();
    let genesis_last_chapter = BibleLocation::new(BibleBook::Genesis, 50, None).unwrap();
    let exodus_first_chapter = BibleLocation::new(BibleBook::Exodus, 1, None).unwrap();
    let final_chapter = BibleLocation::new(BibleBook::Revelation, 22, None).unwrap();
    assert_eq!(loaded.previous_chapter(first_chapter).unwrap(), None);
    assert_eq!(
        loaded.next_chapter(first_chapter).unwrap(),
        Some(genesis_second_chapter)
    );
    assert_eq!(
        loaded.next_chapter(genesis_last_chapter).unwrap(),
        Some(exodus_first_chapter)
    );
    assert_eq!(
        loaded.previous_chapter(exodus_first_chapter).unwrap(),
        Some(genesis_last_chapter)
    );
    assert_eq!(loaded.next_chapter(final_chapter).unwrap(), None);

    let john_three = BibleLocation::new(BibleBook::John, 3, None).unwrap();
    let john_three_sixteen = BibleLocation::new(BibleBook::John, 3, Some(16)).unwrap();
    assert!(loaded.has_next_chapter(john_three).unwrap());
    assert!(loaded.has_previous_chapter(john_three).unwrap());
    assert_eq!(loaded.get_chapter_at(john_three).unwrap().number(), 3);
    assert!(loaded
        .get_verse_at(john_three_sixteen)
        .unwrap()
        .text()
        .contains("God so loved"));

    let first_verse = BibleLocation::new(BibleBook::Genesis, 1, Some(1)).unwrap();
    let genesis_one_last = BibleLocation::new(BibleBook::Genesis, 1, Some(31)).unwrap();
    let genesis_two_first = BibleLocation::new(BibleBook::Genesis, 2, Some(1)).unwrap();
    let genesis_final = BibleLocation::new(BibleBook::Genesis, 50, Some(26)).unwrap();
    let exodus_first = BibleLocation::new(BibleBook::Exodus, 1, Some(1)).unwrap();
    let final_verse = BibleLocation::new(BibleBook::Revelation, 22, Some(21)).unwrap();
    assert_eq!(loaded.previous_verse(first_verse).unwrap(), None);
    assert_eq!(
        loaded.next_verse(genesis_one_last).unwrap(),
        Some(genesis_two_first)
    );
    assert_eq!(
        loaded.next_verse(genesis_final).unwrap(),
        Some(exodus_first)
    );
    assert_eq!(
        loaded.previous_verse(exodus_first).unwrap(),
        Some(genesis_final)
    );
    assert_eq!(loaded.next_verse(final_verse).unwrap(), None);
    assert!(loaded.contains_reference(first_verse));
    assert!(!loaded.contains_reference(BibleLocation::new(BibleBook::John, 999, None).unwrap()));

    assert!(loaded.search("God").len() > 1_000);
}
