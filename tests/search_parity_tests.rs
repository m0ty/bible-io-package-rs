use std::{collections::HashSet, ops::Range};

use bible_io::{
    bible_io_references::Language,
    text_search::{
        find_normalized_substring_ranges, is_within_levenshtein_distance,
        tokenize_search_text_with_ranges,
    },
    Bible, BibleBook, BibleMetadata, Book, Chapter, JsonMap, SearchHit, SearchIndexMode,
    SearchMode, SearchOptions, SearchResults, TextRange, Verse,
};
use unicode_segmentation::UnicodeSegmentation;

fn bible_with_text(text: &str) -> Bible {
    let verse = Verse::checked(BibleBook::Genesis, 1, 1, text, JsonMap::new()).unwrap();
    let chapter = Chapter::checked(BibleBook::Genesis, 1, vec![verse], JsonMap::new()).unwrap();
    let book = Book::checked(BibleBook::Genesis, "Genesis", vec![chapter], JsonMap::new()).unwrap();
    Bible::from_books(
        vec![book],
        Language::English,
        BibleMetadata::default(),
        JsonMap::new(),
        SearchIndexMode::Disabled,
    )
    .unwrap()
}

fn bible_with_repeated_text() -> Bible {
    let verses = (1..=3)
        .map(|number| {
            Verse::checked(BibleBook::Genesis, 1, number, "faith", JsonMap::new()).unwrap()
        })
        .collect();
    let chapter = Chapter::checked(BibleBook::Genesis, 1, verses, JsonMap::new()).unwrap();
    let book = Book::checked(BibleBook::Genesis, "Genesis", vec![chapter], JsonMap::new()).unwrap();
    Bible::from_books(
        vec![book],
        Language::English,
        BibleMetadata::default(),
        JsonMap::new(),
        SearchIndexMode::Disabled,
    )
    .unwrap()
}

fn source_slices<'a>(text: &'a str, ranges: &[TextRange]) -> Vec<&'a str> {
    ranges
        .iter()
        .map(|range| &text[range.start()..range.end()])
        .collect()
}

fn byte_slices<'a>(text: &'a str, ranges: &[Range<usize>]) -> Vec<&'a str> {
    ranges.iter().map(|range| &text[range.clone()]).collect()
}

#[test]
fn normalized_ranges_map_expansions_and_canonical_forms_to_source_bytes() {
    let text = "Straße Cafe\u{301} κόσμος ſcripture";

    let sharp_s = find_normalized_substring_ranges(text, "STRASSE", false, true, false);
    assert_eq!(byte_slices(text, &sharp_s), ["Straße"]);

    let composed = find_normalized_substring_ranges(text, "CAFÉ", false, true, false);
    assert_eq!(byte_slices(text, &composed), ["Cafe\u{301}"]);

    let folded = find_normalized_substring_ranges(text, "cafe", false, true, true);
    assert_eq!(byte_slices(text, &folded), ["Cafe\u{301}"]);

    let sigma = find_normalized_substring_ranges(text, "ΚΌΣΜΟΣ", false, true, false);
    assert_eq!(byte_slices(text, &sigma), ["κόσμος"]);

    let long_s = find_normalized_substring_ranges(text, "SCRIPTURE", false, true, false);
    assert_eq!(byte_slices(text, &long_s), ["ſcripture"]);

    // Both normalized `s` scalars produced by one sharp-s map to one source
    // grapheme, so highlighting never receives duplicate overlapping ranges.
    let expanded_character_ranges = find_normalized_substring_ranges("ß", "s", false, true, false);
    assert_eq!(expanded_character_ranges.len(), 1);
    assert_eq!(expanded_character_ranges[0], 0..2);
}

#[test]
fn token_ranges_remain_utf8_source_ranges_with_combining_marks() {
    let text = "x בְּרֵאשִׁית १२३";
    let tokens = tokenize_search_text_with_ranges(text, false, true, false);
    let raw = tokens
        .iter()
        .map(|token| &text[token.start..token.end])
        .collect::<Vec<_>>();
    assert_eq!(raw, ["x", "בְּרֵאשִׁית", "१२३"]);
    assert_eq!(
        tokens
            .iter()
            .map(|token| token.raw.as_str())
            .collect::<Vec<_>>(),
        raw
    );
}

#[test]
fn exact_whole_word_matches_are_ordered_source_spans() {
    let bible = bible_with_text("God,  created heaven; again God created.");
    let options = SearchOptions {
        mode: SearchMode::Exact,
        whole_words: true,
        ..SearchOptions::default()
    };

    let results = bible.search_with_options("GOD CREATED", &options).unwrap();
    let hit = &results.hits()[0];
    assert_eq!(
        source_slices(hit.verse().text(), hit.match_ranges()),
        ["God,  created", "God created"]
    );
    assert!(bible
        .search_with_options("created God", &options)
        .unwrap()
        .is_empty());
}

#[test]
fn truncated_search_pages_leave_total_count_unknown_until_the_final_page() {
    let bible = bible_with_repeated_text();
    let first_options = SearchOptions {
        max_results: Some(1),
        ..SearchOptions::default()
    };
    let first = bible.search_with_options("faith", &first_options).unwrap();
    assert_eq!(first.count(), 1);
    assert!(first.has_more());
    assert_eq!(first.total_count(), None);
    assert_eq!(first.next_offset(), Some(1));

    let final_options = SearchOptions {
        max_results: Some(1),
        offset: 2,
        ..SearchOptions::default()
    };
    let final_page = bible.search_with_options("faith", &final_options).unwrap();
    assert!(!final_page.has_more());
    assert_eq!(final_page.total_count(), Some(3));
}

#[test]
fn unspaced_search_and_fuzzy_search_report_the_precise_substring() {
    let bible = bible_with_text("起初神创造天地");
    let options = SearchOptions {
        mode: SearchMode::All,
        ..SearchOptions::default()
    };

    let exact = bible.search_with_options("创造", &options).unwrap();
    let exact_hit = &exact.hits()[0];
    assert_eq!(
        source_slices(exact_hit.verse().text(), exact_hit.match_ranges()),
        ["创造"]
    );

    let fuzzy = bible.fuzzy_search("创迼", 1, &options).unwrap();
    let fuzzy_hit = &fuzzy.hits()[0];
    assert_eq!(
        source_slices(fuzzy_hit.verse().text(), fuzzy_hit.match_ranges()),
        ["创造"]
    );

    let whole_word_options = SearchOptions {
        whole_words: true,
        ..options
    };
    assert!(bible
        .fuzzy_search("创造", 0, &whole_word_options)
        .unwrap()
        .is_empty());
}

#[test]
fn fuzzy_ranges_follow_all_any_and_ordered_exact_semantics() {
    let bible = bible_with_text("In the beginníng God truly created; God created heaven.");
    let all = SearchOptions {
        mode: SearchMode::All,
        ignore_diacritics: true,
        ..SearchOptions::default()
    };
    let results = bible.fuzzy_search("begining creatd", 1, &all).unwrap();
    assert_eq!(
        source_slices(
            results.hits()[0].verse().text(),
            results.hits()[0].match_ranges()
        ),
        ["beginníng", "created"]
    );

    let any = SearchOptions {
        mode: SearchMode::Any,
        ..SearchOptions::default()
    };
    assert!(!bible
        .fuzzy_search("missing creatd", 1, &any)
        .unwrap()
        .is_empty());

    let ordered = SearchOptions {
        mode: SearchMode::Exact,
        ..SearchOptions::default()
    };
    let results = bible.fuzzy_search("God created", 0, &ordered).unwrap();
    assert_eq!(
        source_slices(
            results.hits()[0].verse().text(),
            results.hits()[0].match_ranges()
        ),
        ["God created"]
    );
    assert!(bible
        .fuzzy_search("created beginning", 0, &ordered)
        .unwrap()
        .is_empty());
}

#[test]
fn snippet_bounds_are_exact_grapheme_safe_source_slices() {
    let text = "before 👩‍👩‍👧‍👦 Cafe\u{301} needle after 👨‍👩‍👧‍👦 tail";
    let verse = Verse::checked(BibleBook::Genesis, 1, 1, text, JsonMap::new()).unwrap();
    let chapter =
        Chapter::checked(BibleBook::Genesis, 1, vec![verse.clone()], JsonMap::new()).unwrap();
    let book = Book::checked(BibleBook::Genesis, "Genesis", vec![chapter], JsonMap::new()).unwrap();
    let start = text.find("needle").unwrap();
    let hit = SearchHit::with_context(
        verse,
        &book,
        vec![TextRange::new(start, start + "needle".len()).unwrap()],
        12,
    )
    .unwrap();

    let bounds = hit.snippet_bounds();
    assert_eq!(hit.snippet(), &text[bounds.start()..bounds.end()]);
    assert_eq!(
        source_slices(hit.snippet(), hit.snippet_match_ranges()),
        ["needle"]
    );
    let boundaries = text
        .grapheme_indices(true)
        .map(|(offset, _)| offset)
        .chain(std::iter::once(text.len()))
        .collect::<HashSet<_>>();
    assert!(boundaries.contains(&bounds.start()));
    assert!(boundaries.contains(&bounds.end()));
    assert!(hit.has_leading_omission());
    assert!(hit.has_trailing_omission());
}

#[test]
fn search_value_constructors_reject_invalid_state_without_panicking() {
    assert!(TextRange::new(2, 1).is_err());
    assert!(is_within_levenshtein_distance("a𐐀b", "ab", 1));

    let invalid_options = SearchOptions {
        chapter: Some(0),
        ..SearchOptions::default()
    };
    assert_eq!(invalid_options.validate(), Err("chapter must be positive"));
    let invalid_verse_options = SearchOptions {
        verse: Some(0),
        ..SearchOptions::default()
    };
    assert_eq!(
        invalid_verse_options.validate(),
        Err("verse must be positive")
    );

    let verse = Verse::checked(BibleBook::Genesis, 1, 1, "é text", JsonMap::new()).unwrap();
    let chapter =
        Chapter::checked(BibleBook::Genesis, 1, vec![verse.clone()], JsonMap::new()).unwrap();
    let book = Book::checked(BibleBook::Genesis, "Genesis", vec![chapter], JsonMap::new()).unwrap();

    assert!(SearchHit::with_context(verse.clone(), &book, vec![], 0).is_err());
    assert!(SearchHit::with_context(
        verse.clone(),
        &book,
        vec![TextRange::new(0, 0).unwrap()],
        10,
    )
    .is_err());
    assert!(SearchHit::with_context(
        verse.clone(),
        &book,
        vec![TextRange::new(1, 2).unwrap()],
        10,
    )
    .is_err());

    let hit = SearchHit::with_context(
        verse.clone(),
        &book,
        vec![TextRange::new(0, "é".len()).unwrap()],
        10,
    )
    .unwrap();
    let explicit = SearchHit::new(
        verse.clone(),
        &book,
        Some("Custom reference".to_string()),
        vec![TextRange::new(0, "é".len()).unwrap()],
        0,
        "é".len(),
    )
    .unwrap();
    assert_eq!(explicit.reference(), "Custom reference");
    assert_eq!(explicit.snippet(), "é");
    assert!(SearchHit::new(verse.clone(), &book, None, vec![], 0, 1).is_err());
    let compatibility =
        SearchResults::from_verses("é", vec![verse], 0, Some(1), Some(1), false).unwrap();
    assert_eq!(compatibility.count(), 1);
    assert!(compatibility.hits().is_empty());
    assert!(
        SearchResults::from_hits("é", vec![hit.clone()], usize::MAX, None, None, true,).is_err()
    );
    assert!(SearchResults::from_hits("é", vec![hit.clone(), hit], 0, None, None, false,).is_err());
}

#[test]
fn search_values_are_hashable_and_group_by_canonical_and_display_chapters() {
    let first = Verse::checked(BibleBook::Genesis, 1, 1, "one", JsonMap::new()).unwrap();
    let second = Verse::checked(BibleBook::Genesis, 1, 2, "two", JsonMap::new()).unwrap();
    let chapter = Chapter::checked(
        BibleBook::Genesis,
        1,
        vec![first.clone(), second.clone()],
        JsonMap::new(),
    )
    .unwrap();
    let book = Book::checked(BibleBook::Genesis, "בראשית", vec![chapter], JsonMap::new()).unwrap();
    let first_hit =
        SearchHit::with_context(first, &book, vec![TextRange::new(0, 3).unwrap()], 10).unwrap();
    let second_hit =
        SearchHit::with_context(second, &book, vec![TextRange::new(0, 3).unwrap()], 10).unwrap();
    let results = SearchResults::from_hits(
        "query",
        vec![first_hit.clone(), second_hit],
        0,
        None,
        Some(2),
        false,
    )
    .unwrap();

    assert_eq!(results.by_book()[&BibleBook::Genesis].len(), 2);
    assert_eq!(results.by_chapter()["Genesis 1"].len(), 2);
    assert_eq!(
        results.by_chapter_location()[&(BibleBook::Genesis, 1)].len(),
        2
    );
    assert_eq!(results.by_display_chapter()["בראשית 1"].len(), 2);

    let mut hits = HashSet::new();
    assert!(hits.insert(first_hit.clone()));
    assert!(!hits.insert(first_hit));
    let mut result_pages = HashSet::new();
    assert!(result_pages.insert(results.clone()));
    assert!(!result_pages.insert(results));
}
