use bible_io::{
    verse_range_ref_from_str, verse_ref_from_str, BibleBook, ParseError, ParseErrorKind,
    VerseRange, VerseRef,
};

#[test]
fn references_1_1_1_api_is_available_through_public_reexports() {
    let verse = verse_ref_from_str("\u{200f}John \u{0663}\u{ff1a}\u{0661}\u{0666}").unwrap();
    assert_eq!(verse, VerseRef::new(BibleBook::John, 3, 16).unwrap());

    let range = verse_range_ref_from_str("John 3:16\u{2013}18").unwrap();
    assert_eq!(range.start(), verse);
    assert_eq!(range.end(), VerseRef::new(BibleBook::John, 3, 18).unwrap());

    let expanded = range
        .with_end(VerseRef::new(BibleBook::John, 4, 1).unwrap())
        .unwrap();
    assert_eq!(expanded.end().chapter(), 4);
    assert!(range.with_start(range.end()).is_err());

    let error = ParseError::new(ParseErrorKind::Unknown, "unclassified parse failure");
    assert_eq!(error.code(), "unknown");

    let nested: VerseRange =
        bible_io::bible_io_references::verse_range_ref_from_str("John 3:16-18").unwrap();
    assert_eq!(nested, range);
}
