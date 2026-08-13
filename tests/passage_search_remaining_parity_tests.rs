use bible_io::{
    reference_from_osis_identifier, reference_from_usfm_identifier,
    text_search::{
        build_search_index_terms, is_within_levenshtein_distance, search_index_lookup_key,
    },
    Bible, BibleBook, BibleError, BibleLocation, BibleMetadata, Book, Chapter, EditionReference,
    JsonMap, Language, ParseErrorKind, Passage, Reference, SearchIndexMode, SearchMode,
    SearchOptions, Verse, VersePassage, VerseRef,
};
use serde_json::{json, Value};

fn passage_bible() -> Bible {
    Bible::from_json_value(json!({
        "language": "English",
        "books": {
            "gn": {
                "name": "Genesis",
                "chapters": {
                    "2": {"1": "Genesis 2:1"},
                    "1": {"2": "Genesis 1:2", "1": "Genesis 1:1"}
                }
            },
            "ex": {
                "name": "Exodus",
                "chapters": {
                    "1": {"2": "Exodus 1:2", "1": "Exodus 1:1"}
                }
            }
        }
    }))
    .unwrap()
}

fn mode_bible() -> Bible {
    Bible::from_json_value(mode_bible_json()).unwrap()
}

fn mode_bible_json() -> Value {
    json!({
        "language": "English",
        "books": {
            "gn": {
                "chapters": {
                    "1": {
                        "1": "alpha beta gamma",
                        "2": "alpha gamma beta",
                        "3": "alpha delta",
                        "4": "beta delta"
                    }
                }
            }
        }
    })
}

fn unicode_bible() -> Bible {
    Bible::from_json_value(json!({
        "language": "English",
        "books": {
            "gn": {
                "chapters": {
                    "1": {
                        "1": "A scatter pattern.",
                        "2": "A cat-like sister's well-being.",
                        "3": "мир и слово",
                        "4": "мировой порядок",
                        "5": "Cafe\u{301} grace.",
                        "6": "في البدء خلق الله السماوات والأرض",
                        "7": "В начале сотворил Бог небо и землю",
                        "8": "起初，神创造天地。",
                        "9": "神は天地を創造された",
                        "10": "พระเจ้าทรงสร้างฟ้าสวรรค์"
                    },
                    "2": {"1": "alpha in chapter two"}
                }
            },
            "ex": {
                "chapters": {
                    "1": {"1": "alpha in Exodus"}
                }
            }
        }
    }))
    .unwrap()
}

fn coordinates(verses: &[Verse]) -> Vec<(BibleBook, usize, usize)> {
    verses
        .iter()
        .map(|verse| (verse.book(), verse.chapter(), verse.number()))
        .collect()
}

fn borrowed_texts<'a>(verses: &[&'a Verse]) -> Vec<&'a str> {
    verses.iter().map(|verse| verse.text()).collect()
}

#[test]
fn resolves_every_supported_passage_shape_and_reference_range_form() {
    let bible = passage_bible();

    assert_eq!(
        borrowed_texts(&bible.get_passage("Genesis").unwrap()),
        ["Genesis 1:1", "Genesis 1:2", "Genesis 2:1"]
    );
    assert_eq!(
        borrowed_texts(&bible.get_passage("Genesis 1-2").unwrap()),
        ["Genesis 1:1", "Genesis 1:2", "Genesis 2:1"]
    );
    assert_eq!(
        borrowed_texts(&bible.get_passage("Genesis 1:1,2:1").unwrap()),
        ["Genesis 1:1", "Genesis 2:1"]
    );
    assert_eq!(
        borrowed_texts(&bible.get_passage("Genesis 1:2; Exodus 1").unwrap()),
        ["Genesis 1:2", "Exodus 1:1", "Exodus 1:2"]
    );

    assert_eq!(
        borrowed_texts(&bible.get_verse_range_by_reference("Genesis 1:1-2").unwrap()),
        ["Genesis 1:1", "Genesis 1:2"]
    );
    assert_eq!(
        borrowed_texts(
            &bible
                .get_verse_range_by_reference("Genesis 1:2-2:1")
                .unwrap()
        ),
        ["Genesis 1:2", "Genesis 2:1"]
    );
}

#[test]
fn passage_resolution_validates_every_loaded_endpoint_and_chapter() {
    let bible = passage_bible();

    assert!(matches!(
        bible.get_passage("Genesis 1-3"),
        Err(BibleError::ChapterOutOfBounds { chapter: 3, .. })
    ));
    assert!(matches!(
        bible.get_verse_range_by_reference("Genesis 1:1-3:1"),
        Err(BibleError::ChapterOutOfBounds { chapter: 3, .. })
    ));
    assert!(matches!(
        bible.get_verse_range_by_reference("Genesis 1:1-2:2"),
        Err(BibleError::VerseOutOfBounds {
            chapter: 2,
            verse: 2,
            ..
        })
    ));
}

#[test]
fn resolves_dependency_osis_and_usfm_reference_values() {
    let bible = passage_bible();

    let osis = reference_from_osis_identifier("Gen.1.2-Exod.1.1").unwrap();
    assert_eq!(
        borrowed_texts(&bible.resolve_reference(osis).unwrap()),
        ["Genesis 1:2", "Genesis 2:1", "Exodus 1:1"]
    );

    let usfm = reference_from_usfm_identifier("GEN-EXO 1:2-1:1").unwrap();
    assert_eq!(
        borrowed_texts(&bible.resolve_reference(usfm).unwrap()),
        ["Genesis 1:2", "Genesis 2:1", "Exodus 1:1"]
    );
}

#[test]
fn detailed_reference_parsing_detects_language_and_explicit_language_is_strict() {
    let bible = passage_bible();
    let detected = bible.parse_reference_detailed("Génesis 1:1").unwrap();
    assert!(matches!(detected.value(), EditionReference::Verse(_)));
    assert!(detected
        .metadata()
        .and_then(|value| value.detected_language())
        .is_some());

    let explicit = bible
        .parse_reference_detailed_with_language("Génesis 1:1", Language::Spanish)
        .unwrap();
    assert_eq!(
        explicit
            .metadata()
            .and_then(|value| value.detected_language()),
        Some(Language::Spanish)
    );

    assert!(matches!(
        bible.parse_reference("not a reference"),
        Err(BibleError::ReferenceParse { .. })
    ));

    let verse = Verse::checked(BibleBook::Genesis, 1, 1, "Loaded alias", JsonMap::new()).unwrap();
    let chapter = Chapter::checked(BibleBook::Genesis, 1, vec![verse], JsonMap::new()).unwrap();
    let book = Book::checked(
        BibleBook::Genesis,
        "My Genesis",
        vec![chapter],
        JsonMap::new(),
    )
    .unwrap();
    let greek = Bible::from_books(
        vec![book],
        Language::Greek,
        BibleMetadata::default(),
        JsonMap::new(),
        SearchIndexMode::Disabled,
    )
    .unwrap();

    assert_eq!(
        greek
            .get_verse_by_reference("My Genesis 1:1")
            .unwrap()
            .text(),
        "Loaded alias"
    );
    assert!(matches!(
        greek.parse_reference_with_language("Genesis 1:1", Language::Esperanto),
        Err(BibleError::ReferenceParse { cause, .. })
            if cause.kind() == ParseErrorKind::UnsupportedLanguage
    ));
}

#[test]
fn bible_location_converts_to_verse_and_chapter_passages() {
    let reference = VerseRef::new(BibleBook::Genesis, 1, 2).unwrap();
    let location = BibleLocation::from_verse_ref(reference);
    assert_eq!(location.to_verse_ref().unwrap(), reference);
    assert_eq!(
        location.to_passage().unwrap(),
        Passage::from(VersePassage::new([Reference::Verse(reference)]).unwrap())
    );

    let chapter = BibleLocation::new(BibleBook::Genesis, 2, None).unwrap();
    assert_eq!(
        chapter.to_passage().unwrap(),
        Passage::from(bible_io::ChapterPassage::single(BibleBook::Genesis, 2).unwrap())
    );
    assert!(chapter.to_verse_ref().is_err());
    assert!(BibleLocation::new(BibleBook::Genesis, 0, None).is_err());
}

#[test]
fn basic_and_advanced_search_modes_match_the_dart_contract() {
    let bible = mode_bible();

    assert_eq!(
        coordinates(&bible.search("alpha beta")),
        [(BibleBook::Genesis, 1, 1), (BibleBook::Genesis, 1, 2)]
    );
    assert!(bible.search("nonexistentword12345").is_empty());

    let exact = SearchOptions {
        mode: SearchMode::Exact,
        ..SearchOptions::default()
    };
    assert_eq!(
        coordinates(
            bible
                .search_with_options("alpha beta", &exact)
                .unwrap()
                .verses()
        ),
        [(BibleBook::Genesis, 1, 1)]
    );

    let all = SearchOptions {
        mode: SearchMode::All,
        ..SearchOptions::default()
    };
    assert_eq!(
        coordinates(
            bible
                .search_with_options("alpha beta", &all)
                .unwrap()
                .verses()
        ),
        [(BibleBook::Genesis, 1, 1), (BibleBook::Genesis, 1, 2)]
    );

    let any = SearchOptions {
        mode: SearchMode::Any,
        ..SearchOptions::default()
    };
    assert_eq!(
        coordinates(
            bible
                .search_with_options("alpha beta", &any)
                .unwrap()
                .verses()
        ),
        [
            (BibleBook::Genesis, 1, 1),
            (BibleBook::Genesis, 1, 2),
            (BibleBook::Genesis, 1, 3),
            (BibleBook::Genesis, 1, 4)
        ]
    );
}

#[test]
fn search_scopes_blank_queries_and_zero_sized_pages_are_stable() {
    let bible = unicode_bible();
    let scoped = SearchOptions {
        book: Some(BibleBook::Genesis),
        chapter: Some(2),
        verse: Some(1),
        ..SearchOptions::default()
    };
    assert_eq!(
        coordinates(bible.search_with_options("", &scoped).unwrap().verses()),
        [(BibleBook::Genesis, 2, 1)]
    );
    assert_eq!(
        coordinates(
            bible
                .search_with_options("alpha", &scoped)
                .unwrap()
                .verses()
        ),
        [(BibleBook::Genesis, 2, 1)]
    );

    let zero_page = SearchOptions {
        mode: SearchMode::All,
        max_results: Some(0),
        ..SearchOptions::default()
    };
    let results = bible.search_with_options("alpha", &zero_page).unwrap();
    assert!(results.is_empty());
    assert!(results.has_more());
    assert_eq!(results.total_count(), None);
    assert_eq!(results.next_offset(), None);

    let final_blank_page = SearchOptions {
        book: Some(BibleBook::Genesis),
        chapter: Some(1),
        max_results: Some(1),
        offset: 9,
        ..SearchOptions::default()
    };
    let results = bible.search_with_options("   ", &final_blank_page).unwrap();
    assert_eq!(coordinates(results.verses()), [(BibleBook::Genesis, 1, 10)]);
    assert!(!results.has_more());
    assert_eq!(results.total_count(), Some(10));
    assert!(results.has_previous());
}

#[test]
fn indexed_search_is_equivalent_and_pagination_keeps_edition_order() {
    let bible = mode_bible();
    let direct = coordinates(&bible.search("alpha beta"));
    assert_eq!(bible.build_search_index().search("alpha beta"), direct);

    let page = SearchOptions {
        mode: SearchMode::Any,
        max_results: Some(2),
        ..SearchOptions::default()
    };
    let results = bible.search_with_options("beta alpha", &page).unwrap();
    assert_eq!(
        coordinates(results.verses()),
        [(BibleBook::Genesis, 1, 1), (BibleBook::Genesis, 1, 2)]
    );
    assert!(results.has_more());
    assert_eq!(results.next_offset(), Some(2));
}

#[test]
fn unicode_search_honors_case_diacritics_and_word_boundaries() {
    let bible = unicode_bible();

    let whole = SearchOptions {
        mode: SearchMode::All,
        whole_words: true,
        ..SearchOptions::default()
    };
    for (query, expected) in [
        ("cat", 2),
        ("sister", 2),
        ("well", 2),
        ("being", 2),
        ("мир", 3),
    ] {
        let results = bible.search_with_options(query, &whole).unwrap();
        assert_eq!(
            coordinates(results.verses()),
            [(BibleBook::Genesis, 1, expected)],
            "whole-word query {query}"
        );
    }

    assert_eq!(
        coordinates(
            bible
                .search_with_options("CAFÉ", &SearchOptions::default())
                .unwrap()
                .verses()
        ),
        [(BibleBook::Genesis, 1, 5)]
    );
    assert_eq!(
        coordinates(&bible.search("CAFÉ")),
        [(BibleBook::Genesis, 1, 5)]
    );
    assert!(bible
        .search_with_options("cafe", &SearchOptions::default())
        .unwrap()
        .is_empty());
    let folded = SearchOptions {
        ignore_diacritics: true,
        ..SearchOptions::default()
    };
    assert_eq!(
        coordinates(bible.search_with_options("cafe", &folded).unwrap().verses()),
        [(BibleBook::Genesis, 1, 5)]
    );
    let case_sensitive = SearchOptions {
        case_sensitive: true,
        ..SearchOptions::default()
    };
    assert!(bible
        .search_with_options("café", &case_sensitive)
        .unwrap()
        .is_empty());
}

#[test]
fn searches_arabic_russian_chinese_japanese_and_thai() {
    let bible = unicode_bible();
    let all_terms = SearchOptions {
        mode: SearchMode::All,
        ..SearchOptions::default()
    };

    for (query, expected) in [
        ("الله", 6),
        ("Бог", 7),
        ("创造", 8),
        ("創造", 9),
        ("สร้าง", 10),
    ] {
        assert_eq!(
            coordinates(&bible.search(query)),
            [(BibleBook::Genesis, 1, expected)],
            "Unicode query {query}"
        );
    }

    assert_eq!(
        coordinates(
            bible
                .search_with_options("创造", &all_terms)
                .unwrap()
                .verses()
        ),
        [(BibleBook::Genesis, 1, 8)]
    );
    assert_eq!(
        coordinates(
            bible
                .search_with_options("天地", &all_terms)
                .unwrap()
                .verses()
        ),
        [(BibleBook::Genesis, 1, 8), (BibleBook::Genesis, 1, 9)]
    );
    assert_eq!(
        coordinates(
            bible
                .search_with_options("สร้าง", &all_terms)
                .unwrap()
                .verses()
        ),
        [(BibleBook::Genesis, 1, 10)]
    );

    let latin = Bible::from_json_value(json!({
        "books": {"gn": {"chapters": {"1": {"1": "a scatter pattern"}}}}
    }))
    .unwrap();
    assert!(latin
        .search_with_options("cat", &all_terms)
        .unwrap()
        .is_empty());
}

#[test]
fn fuzzy_blank_zero_page_combining_marks_and_numbers_match_dart() {
    let bible = Bible::from_json_value(json!({
        "language": "English",
        "books": {
            "gn": {"chapters": {"1": {
                "1": "beginning",
                "2": "בְּרֵאשִׁית १२३"
            }}}
        }
    }))
    .unwrap();

    let blank = bible
        .fuzzy_search("   ", 2, &SearchOptions::default())
        .unwrap();
    assert!(blank.is_empty());
    assert_eq!(blank.total_count(), Some(0));

    let zero = SearchOptions {
        max_results: Some(0),
        ..SearchOptions::default()
    };
    let zero_page = bible.fuzzy_search("beginning", 2, &zero).unwrap();
    assert!(zero_page.is_empty());
    assert!(zero_page.has_more());
    assert_eq!(zero_page.total_count(), None);

    assert_eq!(
        bible
            .fuzzy_search("בְּרֵאשִׁית", 0, &SearchOptions::default())
            .unwrap()
            .count(),
        1
    );
    assert_eq!(
        bible
            .fuzzy_search("१२३", 0, &SearchOptions::default())
            .unwrap()
            .count(),
        1
    );

    // This Deseret letter is one Unicode scalar but two UTF-16 code units.
    // Fuzzy edit distance is defined over scalars, independently of storage.
    let supplementary = Bible::from_json_value(json!({
        "books": {"gn": {"chapters": {"1": {"1": "a\u{10400}b"}}}}
    }))
    .unwrap();
    assert_eq!(
        supplementary
            .fuzzy_search("ab", 1, &SearchOptions::default())
            .unwrap()
            .count(),
        1
    );
    assert!(supplementary
        .fuzzy_search("ab", 0, &SearchOptions::default())
        .unwrap()
        .is_empty());
}

#[test]
fn verse_words_and_statistics_use_unicode_tokens_only() {
    let verse = Verse::checked(
        BibleBook::Genesis,
        1,
        1,
        "בְּרֵאשִׁית, κόσμος १२३—grace! sister's",
        JsonMap::new(),
    )
    .unwrap();
    assert_eq!(
        verse.words(),
        ["בְּרֵאשִׁית", "κόσμος", "१२३", "grace", "sister", "s"]
    );

    let average = Verse::checked(BibleBook::Genesis, 1, 2, "one, three!", JsonMap::new()).unwrap();
    assert_eq!(average.stats().word_count, 2);
    assert_eq!(average.stats().average_word_length, 4.0);

    let empty = Verse::checked(BibleBook::Genesis, 1, 3, " — !!! ", JsonMap::new()).unwrap();
    assert_eq!(empty.stats().word_count, 0);
    assert_eq!(empty.stats().average_word_length, 0.0);
}

#[test]
fn unspaced_script_lookup_keys_are_compact_and_reversible_through_search() {
    let terms = build_search_index_terms("起初神创造天地", 3);
    assert!(terms.contains("创造"));
    assert_eq!(search_index_lookup_key("创造天地", 3), "创造天");
    assert_eq!(search_index_lookup_key("beginning", 3), "beginning");
}

#[test]
fn bounded_levenshtein_matches_reference_dp_for_all_short_unicode_strings() {
    assert!(!is_within_levenshtein_distance("kitten", "sitting", 2));
    assert!(is_within_levenshtein_distance("kitten", "sitting", 3));
    assert!(is_within_levenshtein_distance("same", "same", 0));
    let samples = short_strings(&['a', 'b', '😀'], 3);
    for first in &samples {
        for second in &samples {
            let expected = reference_levenshtein(first, second);
            for bound in 0..=3 {
                assert_eq!(
                    is_within_levenshtein_distance(first, second, bound),
                    expected <= bound,
                    "{first:?} / {second:?} with bound {bound}"
                );
            }
        }
    }
}

fn short_strings(alphabet: &[char], maximum_length: usize) -> Vec<String> {
    let mut current = vec![String::new()];
    let mut all = current.clone();
    for _ in 1..=maximum_length {
        current = current
            .iter()
            .flat_map(|prefix| {
                alphabet.iter().map(move |character| {
                    let mut value = prefix.clone();
                    value.push(*character);
                    value
                })
            })
            .collect();
        all.extend(current.clone());
    }
    all
}

fn reference_levenshtein(first: &str, second: &str) -> usize {
    let first: Vec<_> = first.chars().collect();
    let second: Vec<_> = second.chars().collect();
    let mut matrix = vec![vec![0; second.len() + 1]; first.len() + 1];
    for (row, values) in matrix.iter_mut().enumerate() {
        values[0] = row;
    }
    for (column, value) in matrix[0].iter_mut().enumerate() {
        *value = column;
    }
    for row in 1..=first.len() {
        for column in 1..=second.len() {
            matrix[row][column] = (matrix[row - 1][column] + 1)
                .min(matrix[row][column - 1] + 1)
                .min(
                    matrix[row - 1][column - 1] + usize::from(first[row - 1] != second[column - 1]),
                );
        }
    }
    matrix[first.len()][second.len()]
}
