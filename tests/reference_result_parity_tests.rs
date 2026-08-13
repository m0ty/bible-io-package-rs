use std::{path::Path, sync::OnceLock};

use bible_io::{Bible, BibleBook, BibleReferenceResult, Language};

fn kjv() -> &'static Bible {
    static KJV: OnceLock<Bible> = OnceLock::new();
    KJV.get_or_init(|| {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("en_kjv.json");
        Bible::new(path.to_str().expect("fixture path must be UTF-8"))
            .expect("the checked-in KJV fixture must load")
    })
}

#[test]
fn get_by_reference_preserves_the_single_verse_result_shape() {
    let result = kjv()
        .get_by_reference("Genesis 1:1")
        .expect("the canonical KJV coordinate must resolve");

    assert!(result.as_range().is_none());
    let verse = result
        .as_verse()
        .expect("a single reference must not be widened to a collection");
    assert_eq!(verse.book(), BibleBook::Genesis);
    assert_eq!(verse.chapter(), 1);
    assert_eq!(verse.number(), 1);
    assert_eq!(
        verse.text(),
        "In the beginning God created the heaven and the earth."
    );

    assert!(matches!(result, BibleReferenceResult::Verse(_)));
}

#[test]
fn get_by_reference_preserves_the_range_shape_and_exact_kjv_order() {
    let result = kjv()
        .get_by_reference("Genesis 1:1-3")
        .expect("the inclusive KJV range must resolve");

    assert!(result.as_verse().is_none());
    let BibleReferenceResult::Range(verses) = result else {
        panic!("a range reference must return the range variant");
    };
    assert_eq!(verses.len(), 3);
    assert!(!verses.is_empty());
    assert_eq!(
        verses
            .iter()
            .map(|verse| (verse.book(), verse.chapter(), verse.number()))
            .collect::<Vec<_>>(),
        [
            (BibleBook::Genesis, 1, 1),
            (BibleBook::Genesis, 1, 2),
            (BibleBook::Genesis, 1, 3),
        ]
    );
    assert_eq!(
        verses.first().expect("range has a first verse").text(),
        "In the beginning God created the heaven and the earth."
    );
    assert_eq!(
        verses.get(1).expect("range has a second verse").text(),
        "And the earth was without form, and void; and darkness {was} upon the face of the deep. And the Spirit of God moved upon the face of the waters."
    );
    assert_eq!(
        verses.last().expect("range has a final verse").text(),
        "And God said, Let there be light: and there was light."
    );
}

#[test]
fn explicit_language_dispatch_and_immutable_companion_apis_match_dart_contracts() {
    let localized = kjv()
        .get_by_reference_with_language("Génesis 1:1", Language::Spanish)
        .expect("an explicit Spanish book name must resolve");
    assert_eq!(
        localized.as_verse().map(|verse| verse.text()),
        Some("In the beginning God created the heaven and the earth.")
    );

    let range = kjv()
        .get_verse_range_selection_by_reference("Genesis 1:3-2:2")
        .expect("the cross-chapter range must resolve");
    assert_eq!(range.len(), 31);
    assert_eq!(
        range.first().map(|verse| (verse.chapter(), verse.number())),
        Some((1, 3))
    );
    assert_eq!(
        range.last().map(|verse| (verse.chapter(), verse.number())),
        Some((2, 2))
    );

    let passage = kjv()
        .get_passage_selection("Genesis 1:1-2; Genesis 1:2")
        .expect("a passage sequence must resolve");
    assert_eq!(passage.len(), 3);
    assert_eq!(
        (&passage)
            .into_iter()
            .map(|verse| verse.number())
            .collect::<Vec<_>>(),
        [1, 2, 2],
        "passage order and meaningful duplicates must be retained"
    );
    assert_eq!(
        passage.as_slice()[0].text(),
        "In the beginning God created the heaven and the earth."
    );
    assert_eq!(
        passage.as_slice()[1].text(),
        "And the earth was without form, and void; and darkness {was} upon the face of the deep. And the Spirit of God moved upon the face of the waters."
    );
    assert_eq!(passage.as_slice()[2].text(), passage.as_slice()[1].text());
}
