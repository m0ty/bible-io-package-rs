use bible_io::{
    Bible, BibleBook, BibleDataFormatErrorCode, BibleError, BibleLoadOptions, BibleLocation,
    EditionVerseRange, SearchIndexMode, SearchOptions, VerseRef,
};
use serde_json::{json, Map, Value};

fn one_verse_book(text: &str) -> Value {
    json!({"chapters": {"1": {"1": text}}})
}

fn three_book_document(order: &[&str]) -> Value {
    json!({
        "language": "English",
        "bookOrder": order,
        "books": {
            "gn": {"chapters": {"3": {"5": "gn"}}},
            "ex": {"chapters": {"2": {"4": "ex"}}},
            "jo": {"chapters": {"7": {"9": "jo"}}}
        }
    })
}

fn coordinate_for(identifier: &str) -> (BibleBook, usize, usize) {
    match identifier {
        "gn" => (BibleBook::Genesis, 3, 5),
        "ex" => (BibleBook::Exodus, 2, 4),
        "jo" => (BibleBook::John, 7, 9),
        _ => panic!("unexpected test identifier: {identifier}"),
    }
}

fn reference_for(identifier: &str) -> VerseRef {
    let (book, chapter, verse) = coordinate_for(identifier);
    VerseRef::new(book, chapter as u16, verse as u16).unwrap()
}

fn location_for(identifier: &str) -> BibleLocation {
    let (book, chapter, verse) = coordinate_for(identifier);
    BibleLocation::new(book, chapter, Some(verse)).unwrap()
}

fn document_with_numeric_keys(chapters: Map<String, Value>) -> Value {
    let mut genesis = Map::new();
    genesis.insert("chapters".to_string(), Value::Object(chapters));
    let mut books = Map::new();
    books.insert("gn".to_string(), Value::Object(genesis));
    let mut root = Map::new();
    root.insert("books".to_string(), Value::Object(books));
    Value::Object(root)
}

#[test]
fn numeric_map_keys_follow_dart_int_parsing_and_canonicalize_on_output() {
    // Dart's int.tryParse accepts surrounding ASCII whitespace and a leading
    // plus. Those spellings identify the same positive integer as the
    // canonical decimal spelling; serialization always emits canonical keys.
    let mut first_verses = Map::new();
    first_verses.insert(" 3 ".to_string(), json!("one-three"));
    first_verses.insert("+1".to_string(), json!("one-one"));
    let mut chapters = Map::new();
    chapters.insert(" +1 ".to_string(), Value::Object(first_verses));
    chapters.insert(" 2 ".to_string(), json!({"01": "two-one"}));

    let bible = Bible::from_json_value(document_with_numeric_keys(chapters)).unwrap();
    assert_eq!(
        bible
            .get_book(BibleBook::Genesis)
            .unwrap()
            .chapters()
            .iter()
            .map(|chapter| chapter.number())
            .collect::<Vec<_>>(),
        [1, 2]
    );
    assert_eq!(
        bible
            .get_verses(BibleBook::Genesis, 1)
            .unwrap()
            .iter()
            .map(|verse| verse.number())
            .collect::<Vec<_>>(),
        [1, 3]
    );
    assert_eq!(
        bible.to_json_value()["books"]["gn"]["chapters"],
        json!({"1": {"1": "one-one", "3": "one-three"}, "2": {"1": "two-one"}})
    );
}

#[test]
fn numeric_map_keys_reject_non_positive_non_integer_and_dart_overflow_values() {
    for invalid in ["", "0", "-1", "1.0", "9223372036854775808"] {
        let mut chapters = Map::new();
        chapters.insert(invalid.to_string(), json!({"1": "text"}));
        let error = Bible::from_json_value(document_with_numeric_keys(chapters)).unwrap_err();
        assert_eq!(
            error.code(),
            BibleDataFormatErrorCode::InvalidValue,
            "{invalid:?}"
        );
        assert_eq!(error.value(), Some(&json!(invalid)), "{invalid:?}");
    }
}

#[test]
fn every_complete_book_order_permutation_drives_iteration_navigation_and_ranges() {
    let permutations = [
        ["gn", "ex", "jo"],
        ["gn", "jo", "ex"],
        ["ex", "gn", "jo"],
        ["ex", "jo", "gn"],
        ["jo", "gn", "ex"],
        ["jo", "ex", "gn"],
    ];

    for order in permutations {
        let bible = Bible::from_json_value(three_book_document(&order)).unwrap();
        assert_eq!(
            bible
                .all_verses()
                .map(|verse| verse.text())
                .collect::<Vec<_>>(),
            order,
            "iteration order for {order:?}"
        );
        assert_eq!(bible.to_json_value()["bookOrder"], json!(order));

        let locations = order.map(location_for);
        assert_eq!(bible.previous_verse(locations[0]).unwrap(), None);
        assert_eq!(bible.next_verse(locations[0]).unwrap(), Some(locations[1]));
        assert_eq!(
            bible.previous_verse(locations[1]).unwrap(),
            Some(locations[0])
        );
        assert_eq!(bible.next_verse(locations[1]).unwrap(), Some(locations[2]));
        assert_eq!(bible.next_verse(locations[2]).unwrap(), None);

        let range = bible
            .resolve_edition_range(EditionVerseRange {
                start: reference_for(order[0]),
                end: reference_for(order[2]),
            })
            .unwrap();
        assert_eq!(
            range.iter().map(|verse| verse.text()).collect::<Vec<_>>(),
            order,
            "typed range order for {order:?}"
        );

        let first = location_for(order[0]);
        let last = location_for(order[2]);
        let textual = format!(
            "{} {}:{}-{} {}:{}",
            first.book().full_name(),
            first.chapter(),
            first.verse().unwrap(),
            last.book().full_name(),
            last.chapter(),
            last.verse().unwrap()
        );
        assert_eq!(
            bible
                .get_verse_range_by_reference(&textual)
                .unwrap()
                .iter()
                .map(|verse| verse.text())
                .collect::<Vec<_>>(),
            order,
            "text range {textual:?} for {order:?}"
        );
    }
}

#[test]
fn book_order_errors_preserve_dart_code_path_and_offending_value() {
    let base_books = json!({
        "gn": one_verse_book("gn"),
        "ex": one_verse_book("ex"),
        "jo": one_verse_book("jo")
    });
    let cases = [
        (
            json!(["gn", 7, "ex"]),
            BibleDataFormatErrorCode::InvalidType,
            "$.bookOrder[1]",
            json!(7),
        ),
        (
            json!(["gn", " ", "ex"]),
            BibleDataFormatErrorCode::InvalidValue,
            "$.bookOrder[1]",
            json!(" "),
        ),
        (
            json!(["gn", "GEN", "ex"]),
            BibleDataFormatErrorCode::InvalidValue,
            "$.bookOrder[1]",
            json!("GEN"),
        ),
        (
            json!(["gn", "mt", "ex"]),
            BibleDataFormatErrorCode::InvalidValue,
            "$.bookOrder[1]",
            json!("mt"),
        ),
    ];
    for (order, code, path, value) in cases {
        let error = Bible::from_json_value(json!({
            "bookOrder": order,
            "books": base_books.clone()
        }))
        .unwrap_err();
        assert_eq!(error.code(), code);
        assert_eq!(error.path(), path);
        assert_eq!(error.value(), Some(&value));
    }
}

#[test]
fn incomplete_book_order_reports_the_missing_identifiers_as_its_value() {
    let incomplete = Bible::from_json_value(json!({
        "bookOrder": ["gn", "ex"],
        "books": {
            "gn": one_verse_book("gn"),
            "ex": one_verse_book("ex"),
            "jo": one_verse_book("jo")
        }
    }))
    .unwrap_err();
    assert_eq!(incomplete.code(), BibleDataFormatErrorCode::InvalidValue);
    assert_eq!(incomplete.path(), "$.bookOrder");
    assert_eq!(incomplete.value(), Some(&json!(["jo"])));
}

fn endpoint_bible() -> Bible {
    Bible::from_json_value(json!({
        "language": "English",
        "bookOrder": ["ex", "gn"],
        "books": {
            "ex": {"chapters": {"2": {"2": "e22", "5": "e25"}}},
            "gn": {"chapters": {
                "1": {"1": "g11", "4": "g14"},
                "3": {"2": "g32"}
            }}
        }
    }))
    .unwrap()
}

#[test]
fn edition_ranges_validate_both_endpoints_before_collecting_and_reject_descents() {
    let bible = endpoint_bible();
    let r = |book, chapter, verse| VerseRef::new(book, chapter, verse).unwrap();
    let valid_start = r(BibleBook::Exodus, 2, 2);
    let valid_end = r(BibleBook::Genesis, 3, 2);

    let valid = bible
        .resolve_edition_range(EditionVerseRange {
            start: valid_start,
            end: valid_end,
        })
        .unwrap();
    assert_eq!(
        valid.iter().map(|verse| verse.text()).collect::<Vec<_>>(),
        ["e22", "e25", "g11", "g14", "g32"]
    );

    let cases = [
        (
            EditionVerseRange {
                start: r(BibleBook::John, 1, 1),
                end: valid_end,
            },
            "start book",
            0,
        ),
        (
            EditionVerseRange {
                start: r(BibleBook::Exodus, 9, 1),
                end: valid_end,
            },
            "start chapter",
            1,
        ),
        (
            EditionVerseRange {
                start: r(BibleBook::Exodus, 2, 1),
                end: valid_end,
            },
            "start verse",
            2,
        ),
        (
            EditionVerseRange {
                start: valid_start,
                end: r(BibleBook::John, 1, 1),
            },
            "end book",
            0,
        ),
        (
            EditionVerseRange {
                start: valid_start,
                end: r(BibleBook::Genesis, 2, 1),
            },
            "end chapter",
            1,
        ),
        (
            EditionVerseRange {
                start: valid_start,
                end: r(BibleBook::Genesis, 3, 1),
            },
            "end verse",
            2,
        ),
    ];
    for (range, label, kind) in cases {
        let error = bible.resolve_edition_range(range).unwrap_err();
        assert!(
            matches!(
                (&error, kind),
                (BibleError::BookNotFound { .. }, 0)
                    | (BibleError::ChapterOutOfBounds { .. }, 1)
                    | (BibleError::VerseOutOfBounds { .. }, 2)
            ),
            "wrong error for {label}: {error:?}"
        );
    }

    for descending in [
        EditionVerseRange {
            start: r(BibleBook::Genesis, 1, 1),
            end: r(BibleBook::Exodus, 2, 5),
        },
        EditionVerseRange {
            start: r(BibleBook::Genesis, 3, 2),
            end: r(BibleBook::Genesis, 1, 4),
        },
        EditionVerseRange {
            start: r(BibleBook::Genesis, 1, 4),
            end: r(BibleBook::Genesis, 1, 1),
        },
    ] {
        assert!(matches!(
            bible.resolve_edition_range(descending),
            Err(BibleError::InvalidRange { .. })
        ));
    }

    let same = bible
        .resolve_edition_range(EditionVerseRange {
            start: valid_start,
            end: valid_start,
        })
        .unwrap();
    assert_eq!(same.len(), 1);
    assert_eq!(same[0].text(), "e22");

    assert!(matches!(
        bible.get_passage("Genesis 1-3"),
        Err(BibleError::ChapterOutOfBounds { chapter: 2, .. })
    ));
}

fn search_document() -> Value {
    json!({
        "language": "English",
        "bookOrder": ["ex", "gn", "jo"],
        "books": {
            "ex": {"chapters": {"1": {
                "1": "Alpha beta",
                "2": "alpha delta"
            }}},
            "gn": {"chapters": {"2": {
                "3": "beta gamma",
                "7": "ALPHA BETA",
                "9": "Cafe\u{301} grace"
            }}},
            "jo": {"chapters": {"4": {
                "1": "A scatter pattern",
                "2": "A cat nap",
                "3": "起初神创造天地",
                "4": "神创造天和平"
            }}}
        }
    })
}

fn search_bible(mode: SearchIndexMode) -> Bible {
    Bible::from_json_value_with_options(
        search_document(),
        BibleLoadOptions {
            search_index_mode: mode,
            ..BibleLoadOptions::default()
        },
    )
    .unwrap()
}

fn search_coordinates(bible: &Bible, query: &str) -> Vec<(BibleBook, usize, usize)> {
    bible
        .search(query)
        .iter()
        .map(|verse| (verse.book(), verse.chapter(), verse.number()))
        .collect()
}

#[test]
fn indexed_and_scanned_term_search_match_an_independent_query_oracle() {
    type Coordinate = (BibleBook, usize, usize);
    let cases: &[(&str, &[Coordinate])] = &[
        (
            "alpha",
            &[
                (BibleBook::Exodus, 1, 1),
                (BibleBook::Exodus, 1, 2),
                (BibleBook::Genesis, 2, 7),
            ],
        ),
        (
            "ALPHA BETA",
            &[(BibleBook::Exodus, 1, 1), (BibleBook::Genesis, 2, 7)],
        ),
        (
            "alpha alpha",
            &[
                (BibleBook::Exodus, 1, 1),
                (BibleBook::Exodus, 1, 2),
                (BibleBook::Genesis, 2, 7),
            ],
        ),
        (
            "beta, alpha!",
            &[(BibleBook::Exodus, 1, 1), (BibleBook::Genesis, 2, 7)],
        ),
        ("cat", &[(BibleBook::John, 4, 2)]),
        ("CAFÉ", &[(BibleBook::Genesis, 2, 9)]),
        ("cafe", &[]),
        ("创造天地", &[(BibleBook::John, 4, 3)]),
        ("missing", &[]),
        ("   ", &[]),
    ];

    for mode in [
        SearchIndexMode::Eager,
        SearchIndexMode::Lazy,
        SearchIndexMode::Disabled,
    ] {
        let bible = search_bible(mode);
        for &(query, expected) in cases {
            assert_eq!(
                search_coordinates(&bible, query),
                expected,
                "{mode:?} search for {query:?}"
            );
        }
    }
}

#[test]
fn pagination_matrix_has_exact_slices_and_stable_metadata() {
    let bible = Bible::from_json_value(json!({
        "books": {"gn": {"chapters": {"1": {
            "1": "one", "2": "two", "3": "three", "4": "four", "5": "five"
        }}}}
    }))
    .unwrap();
    let expected_numbers = [1, 2, 3, 4, 5];

    for offset in 0..=6 {
        for limit in [None, Some(0), Some(1), Some(2), Some(5), Some(8)] {
            let options = SearchOptions {
                offset,
                max_results: limit,
                ..SearchOptions::default()
            };
            let results = bible.search_with_options("", &options).unwrap();

            let start = offset.min(expected_numbers.len());
            let page_length = limit
                .unwrap_or(expected_numbers.len())
                .min(expected_numbers.len() - start);
            let end = start + page_length;
            let expected_page = &expected_numbers[start..end];
            assert_eq!(
                results
                    .verses()
                    .iter()
                    .map(|verse| verse.number())
                    .collect::<Vec<_>>(),
                expected_page,
                "offset {offset}, limit {limit:?}"
            );
            assert_eq!(results.hits().len(), expected_page.len());

            let has_more = end < expected_numbers.len();
            assert_eq!(results.has_more(), has_more);
            assert_eq!(
                results.total_count(),
                (!has_more).then_some(expected_numbers.len())
            );
            assert_eq!(
                results.next_offset(),
                (has_more && !expected_page.is_empty()).then_some(offset + expected_page.len())
            );
            assert_eq!(results.has_previous(), offset > 0);
        }
    }
}
