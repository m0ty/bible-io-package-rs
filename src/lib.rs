//! A Rust library for working with Bible text data structures
//!
//! This library provides structures and functionality for parsing and working with Bible text data,
//! including books, chapters, and verses.

pub mod bible;
pub mod bible_books_enum;
pub mod book;
pub mod chapter;
pub mod errors;
pub mod json_value;
pub mod loading;
pub mod location;
pub mod search;
pub mod search_index;
pub mod source;
pub mod text_search;
pub mod verse;

// Expose the shared reference package and retain the established BibleBook name.
pub use bible_io_references;

// Re-export main types for easier access
pub use bible::{
    Bible, BiblePerformanceMetrics, BibleReferenceResult, BibleStats, EditionParsed,
    EditionPassage, EditionReference, EditionVerseRange, VerseSelection,
};
pub use bible_books_enum::BibleBook;
pub use book::{Book, BookStats};
pub use chapter::{Chapter, ChapterStats};
pub use errors::{BibleDataFormatError, BibleDataFormatErrorCode, BibleError, ModelError};
pub use json_value::JsonMap;
pub use loading::{
    BibleDataValidationOptions, BibleLoadOptions, BibleLoadPhase, BibleLoadProgress,
    CURRENT_BIBLE_SCHEMA_VERSION,
};
pub use location::{BibleLocation, BibleVerseKey};
pub use search::{SearchHit, SearchIndexMode, SearchMode, SearchOptions, SearchResults, TextRange};
pub use search_index::SearchIndex;
pub use source::{
    merge_bible_metadata, BibleCatalog, BibleMetadata, BibleSource, TextDirectionHint,
};
pub use verse::{Verse, VerseStats};

// Re-export the non-conflicting core of bible-io-references 1.1 so one import
// provides content and reference APIs, matching the Dart entrypoint.
pub use bible_io_references::{
    auto_language_collisions, book_from_osis_identifier, book_from_usfm_identifier,
    localized_books, localized_name, long_name, parse_reference, reference_from_osis_identifier,
    reference_from_usfm_identifier, short_name, verse_range_ref_from_str, verse_ref_from_str,
    AmbiguityPolicy, BookCandidate, BookMatch, BookNameStyle, BookPassage, ChapterPassage,
    Coordinate, CoordinateError, ExtractorConfigError, ExtractorWindow, FormattedBook,
    FormattedPassage, FormattedReference, IdentifierError, IdentifierErrorKind, IdentifierFormat,
    Language, LocalizedBook, MachineIdentifiers, ParseBookError, ParseError, ParseErrorKind,
    ParseLanguageError, ParseMetadata, Parsed, ParserBuilder, Passage, PassageBuildError,
    PassageMatch, PassageParser, PassageSequence, RangeOrderError, Reference, ReferenceExtractor,
    ReferenceExtractorBuilder, ReferenceFormatter, ReferenceParser, VersePassage, VerseRange,
    VerseRangeRef, VerseRef, AUTO_LANGUAGE_PRECEDENCE, DEFAULT_MAX_LOOKAHEAD,
    DEFAULT_MAX_LOOKBEHIND, DEFAULT_SINGLE_CHAPTER_BOOKS, MAX_CHAPTER_NUMBER, MAX_VERSE_NUMBER,
};
