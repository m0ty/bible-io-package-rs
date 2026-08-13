use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    path::PathBuf,
    sync::OnceLock,
};

use bible_io::{
    text_search::{
        contains_normalized_text, find_normalized_substring_ranges, normalize_search_text,
        tokenize_search_text,
    },
    Bible, BibleBook, Book, Chapter, JsonMap, SearchHit, SearchMode, SearchOptions, SearchResults,
    TextRange, Verse,
};
use serde_json::json;

fn value_hash(value: &impl Hash) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

fn verse(number: usize, text: &str) -> Verse {
    Verse::checked(BibleBook::Genesis, 1, number, text, JsonMap::new()).unwrap()
}

fn book_with(verses: Vec<Verse>) -> Book {
    let chapter = Chapter::checked(BibleBook::Genesis, 1, verses, JsonMap::new()).unwrap();
    Book::checked(BibleBook::Genesis, "Genesis", vec![chapter], JsonMap::new()).unwrap()
}

fn kjv() -> &'static Bible {
    static KJV: OnceLock<Bible> = OnceLock::new();
    KJV.get_or_init(|| {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("en_kjv.json");
        Bible::new(path.to_str().unwrap()).unwrap()
    })
}

#[test]
fn search_options_have_structural_value_semantics_and_typed_copy_updates() {
    const OPTIONS: SearchOptions = SearchOptions {
        mode: SearchMode::Any,
        case_sensitive: false,
        whole_words: false,
        max_results: Some(3),
        offset: 0,
        book: None,
        chapter: None,
        verse: None,
        normalize_unicode: true,
        ignore_diacritics: false,
    };
    assert_eq!(OPTIONS.mode, SearchMode::Any);
    assert_eq!(OPTIONS.max_results, Some(3));

    let first = SearchOptions {
        mode: SearchMode::All,
        max_results: Some(20),
        offset: 5,
        book: Some(BibleBook::Genesis),
        chapter: Some(1),
        normalize_unicode: false,
        ignore_diacritics: true,
        ..SearchOptions::default()
    };
    let equal = first.clone();
    assert_eq!(first, equal);
    assert_eq!(value_hash(&first), value_hash(&equal));

    // Public typed fields are Rust's copy-with equivalent. They support both
    // replacement and explicit clearing without a dynamic sentinel API.
    let mut changed = first.clone();
    changed.offset = 4;
    assert_eq!(changed.max_results, Some(20));
    assert_eq!(changed.offset, 4);
    changed.max_results = None;
    changed.book = None;
    changed.chapter = None;
    assert_eq!(changed.max_results, None);
    assert_eq!(changed.book, None);
    assert_eq!(changed.chapter, None);

    let invalid_chapter = SearchOptions {
        chapter: Some(0),
        ..SearchOptions::default()
    };
    assert_eq!(invalid_chapter.validate(), Err("chapter must be positive"));
    let invalid_verse = SearchOptions {
        verse: Some(0),
        ..SearchOptions::default()
    };
    assert_eq!(invalid_verse.validate(), Err("verse must be positive"));
}

#[test]
fn explicit_search_hits_clip_ranges_to_exact_snippet_coordinates() {
    let matched = verse(1, "0123456789abcdefghijklmnop");
    let book = book_with(vec![matched.clone()]);
    let hit = SearchHit::new(
        matched.clone(),
        &book,
        None,
        vec![TextRange::new(2, 5).unwrap(), TextRange::new(7, 9).unwrap()],
        3,
        8,
    )
    .unwrap();
    assert_eq!(hit.snippet(), "34567");
    assert_eq!(hit.snippet_bounds(), TextRange::new(3, 8).unwrap());
    assert_eq!(
        hit.snippet_match_ranges(),
        [TextRange::new(0, 2).unwrap(), TextRange::new(4, 5).unwrap()]
    );
    assert!(hit.has_leading_omission());
    assert!(hit.has_trailing_omission());

    let changed_text = verse(1, "0123456789abcdefghijklmnop!");
    assert!(SearchHit::new(changed_text, &book, None, vec![], 0, 1).is_err());
    assert!(SearchHit::new(
        matched.clone(),
        &book,
        None,
        vec![TextRange::new(0, matched.text().len() + 1).unwrap()],
        0,
        matched.text().len(),
    )
    .is_err());
    assert!(SearchHit::new(
        matched.clone(),
        &book,
        None,
        vec![TextRange::new(3, 6).unwrap(), TextRange::new(5, 7).unwrap()],
        0,
        matched.text().len(),
    )
    .is_err());
    assert!(SearchHit::new(matched, &book, None, vec![], 4, 3).is_err());
}

#[test]
fn result_pages_validate_pagination_duplicates_and_value_semantics() {
    let first = verse(1, "one");
    let second = verse(2, "two");
    let results = SearchResults::from_verses(
        "query",
        vec![first.clone(), second.clone()],
        2,
        Some(2),
        Some(5),
        true,
    )
    .unwrap();
    let equivalent = SearchResults::from_verses(
        "query",
        vec![verse(1, "one"), verse(2, "two")],
        2,
        Some(2),
        Some(5),
        true,
    )
    .unwrap();
    assert_eq!(results, equivalent);
    assert_eq!(value_hash(&results), value_hash(&equivalent));
    assert!(results.has_previous());
    assert!(results.has_more());
    assert_eq!(results.next_offset(), Some(4));

    let empty = SearchResults::from_verses("none", vec![], 10, None, Some(0), false).unwrap();
    assert!(!empty.has_previous());
    assert_eq!(empty.next_offset(), None);

    assert!(SearchResults::from_verses("q", vec![first.clone()], 0, Some(0), None, false).is_err());
    assert!(SearchResults::from_verses("q", vec![first.clone()], 0, None, Some(1), true).is_err());
    assert!(SearchResults::from_verses(
        "q",
        vec![first, verse(1, "duplicate")],
        0,
        None,
        None,
        false,
    )
    .is_err());
}

#[test]
fn utf8_json_loading_preserves_and_searches_multiple_scripts() {
    let arabic = "في البدء خلق الله السماوات والأرض";
    let russian = "В начале сотворил Бог небо и землю";
    let chinese = "起初，神创造天地。";
    let encoded = serde_json::to_vec(&json!({
        "language": "English",
        "books": {"gn": {"chapters": {"1": {
            "1": arabic,
            "2": russian,
            "3": chinese
        }}}}
    }))
    .unwrap();
    let bible = Bible::from_json_slice(&encoded).unwrap();
    assert_eq!(
        bible.get_verse(BibleBook::Genesis, 1, 1).unwrap().text(),
        arabic
    );
    assert_eq!(
        bible.get_verse(BibleBook::Genesis, 1, 2).unwrap().text(),
        russian
    );
    assert_eq!(
        bible.get_verse(BibleBook::Genesis, 1, 3).unwrap().text(),
        chinese
    );
    for (query, number) in [("الله", 1), ("Бог", 2), ("创造", 3)] {
        assert_eq!(
            bible
                .search(query)
                .iter()
                .map(Verse::number)
                .collect::<Vec<_>>(),
            [number]
        );
        assert_eq!(
            bible
                .search_with_options(query, &SearchOptions::default())
                .unwrap()
                .verses()
                .iter()
                .map(Verse::number)
                .collect::<Vec<_>>(),
            [number]
        );
    }
}

#[test]
fn exact_substrings_remain_distinct_from_whole_word_matching() {
    let bible = Bible::from_json_value(json!({
        "books": {"gn": {"chapters": {"1": {
            "1": "A cat-like word.",
            "2": "A scatter pattern."
        }}}}
    }))
    .unwrap();
    let substring = SearchOptions::default();
    assert_eq!(
        bible
            .search_with_options("cat", &substring)
            .unwrap()
            .verses()
            .iter()
            .map(Verse::number)
            .collect::<Vec<_>>(),
        [1, 2]
    );
    let whole = SearchOptions {
        whole_words: true,
        ..SearchOptions::default()
    };
    assert_eq!(
        bible
            .search_with_options("cat", &whole)
            .unwrap()
            .verses()
            .iter()
            .map(Verse::number)
            .collect::<Vec<_>>(),
        [1]
    );
}

#[test]
fn real_fixture_hits_have_display_references_and_every_term_range() {
    let one = SearchOptions {
        max_results: Some(1),
        ..SearchOptions::default()
    };
    let results = kjv().search_with_options("beginning", &one).unwrap();
    let hit = &results.hits()[0];
    assert_eq!(hit.book_name(), "Genesis");
    assert_eq!(hit.reference(), "Genesis 1:1");
    assert!(hit.snippet().contains("beginning"));
    assert_eq!(
        hit.match_ranges()
            .iter()
            .map(|range| &hit.verse().text()[range.start()..range.end()])
            .collect::<Vec<_>>(),
        ["beginning"]
    );
    assert_eq!(results.verses()[0], *hit.verse());

    let all = SearchOptions {
        mode: SearchMode::All,
        max_results: Some(1),
        ..SearchOptions::default()
    };
    let hit = &kjv()
        .search_with_options("heaven earth", &all)
        .unwrap()
        .hits()[0]
        .clone();
    let matched = hit
        .match_ranges()
        .iter()
        .map(|range| &hit.verse().text()[range.start()..range.end()])
        .collect::<std::collections::HashSet<_>>();
    assert!(matched.contains("heaven"));
    assert!(matched.contains("earth"));
}

#[test]
fn fuzzy_normalization_and_pagination_follow_the_same_contract() {
    let bible = Bible::from_json_value(json!({
        "books": {"gn": {"chapters": {"1": {
            "1": "Cafe\u{0301} בָּרָא beginning",
            "2": "beginning",
            "3": "beginning"
        }}}}
    }))
    .unwrap();
    assert_eq!(
        bible
            .fuzzy_search("café", 0, &SearchOptions::default())
            .unwrap()
            .count(),
        1
    );
    assert!(bible
        .fuzzy_search("ברא", 0, &SearchOptions::default())
        .unwrap()
        .is_empty());
    let folded = SearchOptions {
        ignore_diacritics: true,
        ..SearchOptions::default()
    };
    assert_eq!(bible.fuzzy_search("ברא", 0, &folded).unwrap().count(), 1);

    let middle = SearchOptions {
        max_results: Some(1),
        offset: 1,
        ..SearchOptions::default()
    };
    let page = bible.fuzzy_search("begining", 1, &middle).unwrap();
    assert_eq!(page.verses()[0].number(), 2);
    assert_eq!(page.offset(), 1);
    assert_eq!(page.limit(), Some(1));
    assert!(page.has_more());
    assert_eq!(page.next_offset(), Some(2));
    assert!(!page.hits()[0].match_ranges().is_empty());
    assert!(!page.hits()[0].snippet_match_ranges().is_empty());

    let final_page = SearchOptions {
        max_results: Some(2),
        offset: 2,
        ..SearchOptions::default()
    };
    let page = bible.fuzzy_search("begining", 1, &final_page).unwrap();
    assert_eq!(page.total_count(), Some(3));
    assert!(!page.has_more());
}

#[test]
fn normalized_text_utilities_follow_source_offsets_and_opt_in_folding() {
    let source = "Cafe\u{301} society";
    assert_eq!(
        tokenize_search_text(source, false, true, false)[0],
        normalize_search_text("Café", false, true, false)
    );
    assert!(contains_normalized_text(source, "CAFÉ", false, true, false));
    let ranges = find_normalized_substring_ranges(source, "fé", false, true, false);
    // Dart offsets are UTF-16 code units (2..5). Rust's documented offset
    // contract uses UTF-8 byte boundaries so callers can slice `&str` safely.
    assert_eq!(ranges.len(), 1);
    assert_eq!(ranges[0], 2..6);
    assert_eq!(&source[ranges[0].clone()], "fe\u{301}");
    assert!(!contains_normalized_text(
        "café", "cafe", false, true, false
    ));
    assert!(contains_normalized_text("café", "cafe", false, true, true));
    assert!(contains_normalized_text(
        "Straße", "STRASSE", false, true, false
    ));
    assert!(contains_normalized_text(
        "κόσμος",
        "ΚΌΣΜΟΣ",
        false,
        true,
        false
    ));
    assert!(contains_normalized_text(
        "ſcripture",
        "SCRIPTURE",
        false,
        true,
        false
    ));
}
