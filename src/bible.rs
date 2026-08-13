//! Loading, navigation, reference resolution, search, and serialization.

use std::{
    collections::{HashMap, HashSet},
    fs,
    hash::{Hash, Hasher},
    path::Path,
    sync::{OnceLock, RwLock},
    time::{Duration, SystemTime},
};

use bible_io_references::{
    BookPassage, ChapterPassage, Language, ParseError, ParseErrorKind, Passage, PassageParser,
    PassageSequence, Reference, ReferenceFormatter, ReferenceParser, VersePassage, VerseRange,
    VerseRef,
};
use serde_json::{Map, Value};

use crate::{
    bible_books_enum::BibleBook,
    book::Book,
    chapter::Chapter,
    errors::{BibleDataFormatError, BibleDataFormatErrorCode, ModelError},
    json_value::{hash_json_map, validate_annotations, JsonMap},
    loading::{
        BibleDataValidationOptions, BibleLoadOptions, BibleLoadPhase, BibleLoadProgress,
        CURRENT_BIBLE_SCHEMA_VERSION,
    },
    location::{parse_book_identifier, BibleLocation, BibleVerseKey},
    search::{
        find_match_ranges, fuzzy_match_ranges, fuzzy_matches, matches_search_text, SearchHit,
        SearchIndexMode, SearchOptions, SearchResults,
    },
    search_index::SearchIndex,
    source::{BibleMetadata, BibleSource},
    text_search::{
        build_search_index_terms, extract_unicode_words, normalize_search_text,
        tokenize_search_text,
    },
    verse::Verse,
};

pub use crate::errors::BibleError;

const LEGACY_REFERENCE_ALIASES: &[(&str, BibleBook)] = &[
    ("ge", BibleBook::Genesis),
    ("le", BibleBook::Leviticus),
    ("nu", BibleBook::Numbers),
    ("sos", BibleBook::SongOfSolomon),
    ("songofsongs", BibleBook::SongOfSolomon),
    ("da", BibleBook::Daniel),
    ("joe", BibleBook::Joel),
    ("1thes", BibleBook::FirstThessalonians),
    ("2thes", BibleBook::SecondThessalonians),
    ("jam", BibleBook::James),
    ("estg", BibleBook::EstherAdditions),
    ("dan3", BibleBook::DanielSongOfThree),
];

/// An inclusive range ordered by the loaded edition, including custom canons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EditionVerseRange {
    /// Inclusive first verse.
    pub start: VerseRef,
    /// Inclusive final verse.
    pub end: VerseRef,
}

/// A parsed verse or range whose ordering may follow a custom edition canon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EditionReference {
    /// One verse.
    Verse(VerseRef),
    /// An inclusive range ordered by the loaded edition.
    Range(EditionVerseRange),
}

/// An immutable, edition-ordered selection of resolved verses.
///
/// This type intentionally exposes only shared slice access. It preserves the
/// source order and duplicates produced by range and passage resolution while
/// preventing callers from pushing, removing, or reordering entries.
///
/// ```compile_fail
/// # use bible_io::VerseSelection;
/// # fn cannot_reorder(mut verses: VerseSelection<'_>) {
/// verses.swap(0, 1);
/// # }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VerseSelection<'a> {
    verses: Box<[&'a Verse]>,
}

impl<'a> VerseSelection<'a> {
    fn from_vec(verses: Vec<&'a Verse>) -> Self {
        Self {
            verses: verses.into_boxed_slice(),
        }
    }

    /// Borrow the resolved verses as an immutable slice.
    #[must_use]
    pub fn as_slice(&self) -> &[&'a Verse] {
        &self.verses
    }

    /// Iterate over the resolved verses in edition order.
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &'a Verse> + ExactSizeIterator + '_ {
        self.verses.iter().copied()
    }

    /// Return the number of resolved verses.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.verses.len()
    }

    /// Return whether the selection contains no verses.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.verses.is_empty()
    }

    /// Borrow the verse at `index`.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&'a Verse> {
        self.verses.get(index).copied()
    }

    /// Borrow the first resolved verse.
    #[must_use]
    pub fn first(&self) -> Option<&'a Verse> {
        self.verses.first().copied()
    }

    /// Borrow the last resolved verse.
    #[must_use]
    pub fn last(&self) -> Option<&'a Verse> {
        self.verses.last().copied()
    }
}

impl<'a> AsRef<[&'a Verse]> for VerseSelection<'a> {
    fn as_ref(&self) -> &[&'a Verse] {
        self.as_slice()
    }
}

impl<'a> std::ops::Deref for VerseSelection<'a> {
    type Target = [&'a Verse];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<'a, 'selection> IntoIterator for &'selection VerseSelection<'a> {
    type Item = &'a Verse;
    type IntoIter = std::iter::Copied<std::slice::Iter<'selection, &'a Verse>>;

    fn into_iter(self) -> Self::IntoIter {
        self.verses.iter().copied()
    }
}

impl<'a> IntoIterator for VerseSelection<'a> {
    type Item = &'a Verse;
    type IntoIter = std::vec::IntoIter<&'a Verse>;

    fn into_iter(self) -> Self::IntoIter {
        self.verses.into_vec().into_iter()
    }
}

/// The resolved result of a narrow Bible reference.
///
/// A single-verse reference retains the direct borrowed-verse shape of Dart's
/// `getByRef`; an inclusive range is represented by an immutable selection.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum BibleReferenceResult<'a> {
    /// One resolved verse.
    Verse(&'a Verse),
    /// An inclusive, edition-ordered verse range.
    Range(VerseSelection<'a>),
}

impl<'a> BibleReferenceResult<'a> {
    /// Borrow the single verse, or return `None` for a range.
    #[must_use]
    pub const fn as_verse(&self) -> Option<&'a Verse> {
        match self {
            Self::Verse(verse) => Some(*verse),
            Self::Range(_) => None,
        }
    }

    /// Borrow the range selection, or return `None` for a single verse.
    #[must_use]
    pub const fn as_range(&self) -> Option<&VerseSelection<'a>> {
        match self {
            Self::Verse(_) => None,
            Self::Range(verses) => Some(verses),
        }
    }

    /// Consume the result and return its range selection, if present.
    #[must_use]
    pub fn into_range(self) -> Option<VerseSelection<'a>> {
        match self {
            Self::Verse(_) => None,
            Self::Range(verses) => Some(verses),
        }
    }
}

/// A parsed passage whose verse range may follow the loaded edition's canon.
///
/// Most values are represented by the shared references package. The
/// `Reference` variant covers the one shape that package deliberately cannot
/// construct: a cross-book range that ascends in a custom edition order while
/// descending in canonical enum order.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EditionPassage {
    /// A passage represented directly by `bible-io-references`.
    Standard(Passage),
    /// A narrow verse or range reference ordered by this edition.
    Reference(EditionReference),
}

/// An edition-aware parsed value with optional language-detection metadata.
///
/// Metadata is absent only for a custom-canon range, because the dependency's
/// metadata wrapper can only be produced after constructing its canonically
/// ordered range type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditionParsed<T> {
    value: T,
    metadata: Option<bible_io_references::ParseMetadata>,
}

impl<T> EditionParsed<T> {
    const fn new(value: T, metadata: Option<bible_io_references::ParseMetadata>) -> Self {
        Self { value, metadata }
    }

    /// Borrow the parsed value.
    #[must_use]
    pub const fn value(&self) -> &T {
        &self.value
    }

    /// Borrow language-detection metadata when the shared parser produced it.
    #[must_use]
    pub const fn metadata(&self) -> Option<&bible_io_references::ParseMetadata> {
        self.metadata.as_ref()
    }

    /// Consume this wrapper and return its value and optional metadata.
    #[must_use]
    pub fn into_parts(self) -> (T, Option<bible_io_references::ParseMetadata>) {
        (self.value, self.metadata)
    }
}

/// Counts and averages for one Bible.
#[derive(Debug, Clone, PartialEq)]
pub struct BibleStats {
    /// Number of loaded books.
    pub book_count: usize,
    /// Number of loaded chapters.
    pub chapter_count: usize,
    /// Number of loaded verses.
    pub verse_count: usize,
    /// Number of Unicode word tokens.
    pub total_words: usize,
    /// Mean verse byte length, rounded to the nearest integer.
    pub average_verse_length: usize,
    /// Verse count grouped by book.
    pub verses_per_book: HashMap<BibleBook, usize>,
}

/// Diagnostics for loaded content and the retained index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BiblePerformanceMetrics {
    /// File/model load time when measured.
    pub load_time: Duration,
    /// Distinct retained index terms.
    pub search_index_size: usize,
    /// Whether an index is currently retained.
    pub search_index_built: bool,
    /// Number of verses.
    pub verse_count: usize,
    /// Number of retained index postings.
    pub posting_count: usize,
    /// Total UTF-8 text bytes.
    pub text_bytes: usize,
    /// Conservative approximate model/index memory in KiB.
    pub memory_usage_kib: usize,
}

/// Complete immutable Bible content with an optionally retained search cache.
#[derive(Debug)]
pub struct Bible {
    books: Vec<Book>,
    index_by_book: HashMap<BibleBook, usize>,
    reference_parser: ReferenceParser,
    passage_parser: PassageParser,
    search_index: RwLock<Option<SearchIndex>>,
    search_index_mode: SearchIndexMode,
    schema_version: u32,
    metadata: BibleMetadata,
    annotations: JsonMap,
    language: Language,
    load_time: Duration,
    created_at: SystemTime,
}

impl Clone for Bible {
    fn clone(&self) -> Self {
        Self {
            books: self.books.clone(),
            index_by_book: self.index_by_book.clone(),
            reference_parser: self.reference_parser.clone(),
            passage_parser: self.passage_parser.clone(),
            search_index: RwLock::new(
                self.search_index
                    .read()
                    .expect("search lock poisoned")
                    .clone(),
            ),
            search_index_mode: self.search_index_mode,
            schema_version: self.schema_version,
            metadata: self.metadata.clone(),
            annotations: self.annotations.clone(),
            language: self.language,
            load_time: self.load_time,
            created_at: self.created_at,
        }
    }
}

impl PartialEq for Bible {
    fn eq(&self, other: &Self) -> bool {
        self.books == other.books
            && self.schema_version == other.schema_version
            && self.metadata == other.metadata
            && self.annotations == other.annotations
            && self.language == other.language
    }
}

impl Eq for Bible {}

impl Hash for Bible {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.books.hash(state);
        self.schema_version.hash(state);
        self.metadata.hash(state);
        hash_json_map(&self.annotations, state);
        self.language.hash(state);
    }
}

impl Bible {
    /// Load a Bible from a JSON file using strict validation.
    pub fn new(json_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        Self::from_path(json_path, BibleLoadOptions::default())
            .map_err(|error| Box::new(error) as Box<dyn std::error::Error>)
    }

    /// Load a Bible from a path with explicit construction options.
    pub fn from_path(
        path: impl AsRef<Path>,
        options: BibleLoadOptions,
    ) -> Result<Self, BibleDataFormatError> {
        let started = std::time::Instant::now();
        let bytes = fs::read(path.as_ref()).map_err(|error| {
            BibleDataFormatError::new(
                BibleDataFormatErrorCode::InvalidJson,
                "$",
                format!("unable to read Bible file {}", path.as_ref().display()),
            )
            .with_cause(error)
        })?;
        let mut bible = Self::from_json_slice_with_options(&bytes, options)?;
        bible.load_time = started.elapsed();
        Ok(bible)
    }

    /// Load a Bible from a path while reporting stable reading, processing,
    /// and completion phases.
    pub fn from_path_with_progress(
        path: impl AsRef<Path>,
        options: BibleLoadOptions,
        mut on_progress: impl FnMut(BibleLoadProgress),
    ) -> Result<Self, BibleDataFormatError> {
        let started = std::time::Instant::now();
        on_progress(
            BibleLoadProgress::new(BibleLoadPhase::Reading, 0.0, 0.0)
                .expect("constant progress is valid"),
        );
        let bytes = fs::read(path.as_ref()).map_err(|error| {
            BibleDataFormatError::new(
                BibleDataFormatErrorCode::InvalidJson,
                "$",
                format!("unable to read Bible file {}", path.as_ref().display()),
            )
            .with_cause(error)
        })?;
        on_progress(
            BibleLoadProgress::new(BibleLoadPhase::Reading, 0.65, 1.0)
                .expect("constant progress is valid"),
        );
        on_progress(
            BibleLoadProgress::new(BibleLoadPhase::Processing, 0.65, 0.0)
                .expect("constant progress is valid"),
        );
        let mut bible = Self::from_json_slice_with_options(&bytes, options)?;
        bible.load_time = started.elapsed();
        on_progress(
            BibleLoadProgress::new(BibleLoadPhase::Processing, 1.0, 1.0)
                .expect("constant progress is valid"),
        );
        on_progress(
            BibleLoadProgress::new(BibleLoadPhase::Complete, 1.0, 1.0)
                .expect("constant progress is valid"),
        );
        Ok(bible)
    }

    /// Load a Bible from a path and attach explicit catalog provenance.
    pub fn from_path_with_source(
        path: impl AsRef<Path>,
        source: &BibleSource,
        options: BibleLoadOptions,
    ) -> Result<Self, BibleDataFormatError> {
        let started = std::time::Instant::now();
        let bytes = fs::read(path.as_ref()).map_err(|error| {
            BibleDataFormatError::new(
                BibleDataFormatErrorCode::InvalidJson,
                "$",
                format!("unable to read Bible file {}", path.as_ref().display()),
            )
            .with_cause(error)
        })?;
        let mut bible = Self::from_json_slice_with_source(&bytes, source, options)?;
        bible.load_time = started.elapsed();
        Ok(bible)
    }

    /// Load from a path with explicit provenance and progress reporting.
    pub fn from_path_with_source_and_progress(
        path: impl AsRef<Path>,
        source: &BibleSource,
        options: BibleLoadOptions,
        mut on_progress: impl FnMut(BibleLoadProgress),
    ) -> Result<Self, BibleDataFormatError> {
        let started = std::time::Instant::now();
        on_progress(
            BibleLoadProgress::new(BibleLoadPhase::Reading, 0.0, 0.0)
                .expect("constant progress is valid"),
        );
        let bytes = fs::read(path.as_ref()).map_err(|error| {
            BibleDataFormatError::new(
                BibleDataFormatErrorCode::InvalidJson,
                "$",
                format!("unable to read Bible file {}", path.as_ref().display()),
            )
            .with_cause(error)
        })?;
        on_progress(
            BibleLoadProgress::new(BibleLoadPhase::Reading, 0.65, 1.0)
                .expect("constant progress is valid"),
        );
        on_progress(
            BibleLoadProgress::new(BibleLoadPhase::Processing, 0.65, 0.0)
                .expect("constant progress is valid"),
        );
        let mut bible = Self::from_json_slice_with_source(&bytes, source, options)?;
        bible.load_time = started.elapsed();
        on_progress(
            BibleLoadProgress::new(BibleLoadPhase::Processing, 1.0, 1.0)
                .expect("constant progress is valid"),
        );
        on_progress(
            BibleLoadProgress::new(BibleLoadPhase::Complete, 1.0, 1.0)
                .expect("constant progress is valid"),
        );
        Ok(bible)
    }

    /// Decode a Bible from UTF-8 JSON bytes with strict defaults.
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self, BibleDataFormatError> {
        Self::from_json_slice_with_options(bytes, BibleLoadOptions::default())
    }

    /// Decode a Bible from UTF-8 JSON bytes and explicit options.
    pub fn from_json_slice_with_options(
        bytes: &[u8],
        options: BibleLoadOptions,
    ) -> Result<Self, BibleDataFormatError> {
        let text = std::str::from_utf8(bytes).map_err(|error| {
            BibleDataFormatError::new(
                BibleDataFormatErrorCode::InvalidJson,
                "$",
                "Bible bytes must contain valid UTF-8 JSON",
            )
            .with_cause(error)
        })?;
        Self::from_json_str_with_options(text, options)
    }

    /// Decode UTF-8 JSON bytes and attach an explicit catalog source.
    pub fn from_json_slice_with_source(
        bytes: &[u8],
        source: &BibleSource,
        options: BibleLoadOptions,
    ) -> Result<Self, BibleDataFormatError> {
        let text = std::str::from_utf8(bytes).map_err(|error| {
            BibleDataFormatError::new(
                BibleDataFormatErrorCode::InvalidJson,
                "$",
                "Bible bytes must contain valid UTF-8 JSON",
            )
            .with_cause(error)
        })?;
        Self::from_json_str_with_source(text, source, options)
    }

    /// Decode a Bible from JSON text with strict defaults.
    pub fn from_json_str(input: &str) -> Result<Self, BibleDataFormatError> {
        Self::from_json_str_with_options(input, BibleLoadOptions::default())
    }

    /// Decode a Bible from JSON text and explicit options.
    pub fn from_json_str_with_options(
        input: &str,
        options: BibleLoadOptions,
    ) -> Result<Self, BibleDataFormatError> {
        let value: Value = serde_json::from_str(input).map_err(|error| {
            BibleDataFormatError::new(
                BibleDataFormatErrorCode::InvalidJson,
                "$",
                "Bible content is not valid JSON",
            )
            .with_cause(error)
        })?;
        Self::from_json_value_with_options(value, options)
    }

    /// Decode JSON text and attach an explicit catalog source. Explicit
    /// source provenance replaces an embedded source while nested metadata
    /// fields retain precedence for edition-specific values.
    pub fn from_json_str_with_source(
        input: &str,
        source: &BibleSource,
        options: BibleLoadOptions,
    ) -> Result<Self, BibleDataFormatError> {
        let value: Value = serde_json::from_str(input).map_err(|error| {
            BibleDataFormatError::new(
                BibleDataFormatErrorCode::InvalidJson,
                "$",
                "Bible content is not valid JSON",
            )
            .with_cause(error)
        })?;
        Self::from_json_value_with_source(value, source, options)
    }

    /// Decode a Bible from an already parsed JSON value with strict defaults.
    pub fn from_json_value(value: Value) -> Result<Self, BibleDataFormatError> {
        Self::from_json_value_with_options(value, BibleLoadOptions::default())
    }

    /// Decode a Bible from a JSON value and explicit options.
    pub fn from_json_value_with_options(
        value: Value,
        options: BibleLoadOptions,
    ) -> Result<Self, BibleDataFormatError> {
        Self::from_json_value_with_optional_source(value, None, options)
    }

    /// Decode a JSON value while attaching an explicit catalog source.
    pub fn from_json_value_with_source(
        value: Value,
        source: &BibleSource,
        options: BibleLoadOptions,
    ) -> Result<Self, BibleDataFormatError> {
        Self::from_json_value_with_optional_source(value, Some(source), options)
    }

    fn from_json_value_with_optional_source(
        value: Value,
        source: Option<&BibleSource>,
        options: BibleLoadOptions,
    ) -> Result<Self, BibleDataFormatError> {
        let root = value.as_object().ok_or_else(|| {
            BibleDataFormatError::new(
                BibleDataFormatErrorCode::InvalidType,
                "$",
                "Bible JSON must have an object at its root",
            )
            .with_value(value.clone())
        })?;
        let schema_version = match root.get("schemaVersion") {
            None => CURRENT_BIBLE_SCHEMA_VERSION,
            Some(Value::Number(number)) => number
                .as_u64()
                .and_then(|value| u32::try_from(value).ok())
                .ok_or_else(|| {
                    data_error(
                        BibleDataFormatErrorCode::InvalidType,
                        "$.schemaVersion",
                        "schemaVersion must be an integer",
                        root.get("schemaVersion"),
                    )
                })?,
            other => {
                return Err(data_error(
                    BibleDataFormatErrorCode::InvalidType,
                    "$.schemaVersion",
                    "schemaVersion must be an integer",
                    other,
                ))
            }
        };
        if schema_version != CURRENT_BIBLE_SCHEMA_VERSION {
            return Err(data_error(
                BibleDataFormatErrorCode::InvalidValue,
                "$.schemaVersion",
                "unsupported Bible schema version",
                root.get("schemaVersion"),
            ));
        }

        let metadata = read_metadata(root, source)?;
        let language = resolve_language(root.get("language"), &metadata)?;
        let books_value = root.get("books");
        let books_object = match books_value {
            Some(Value::Object(object)) => object,
            None if !options.validation.require_books => EMPTY_MAP.get_or_init(Map::new),
            None => {
                return Err(BibleDataFormatError::new(
                    BibleDataFormatErrorCode::MissingField,
                    "$.books",
                    "Bible content must declare a books object",
                ))
            }
            other => {
                return Err(data_error(
                    BibleDataFormatErrorCode::InvalidType,
                    "$.books",
                    "Bible books must be an object",
                    other,
                ))
            }
        };
        if options.validation.require_books && books_object.is_empty() {
            return Err(data_error(
                BibleDataFormatErrorCode::InvalidValue,
                "$.books",
                "Bible content must contain at least one book",
                books_value,
            ));
        }
        let mut parsed = HashMap::new();
        let mut declared_order = Vec::with_capacity(books_object.len());
        for (identifier, raw_book) in books_object {
            let path = json_path("$.books", identifier);
            let book = parse_book_identifier(identifier).ok_or_else(|| {
                data_error(
                    BibleDataFormatErrorCode::InvalidValue,
                    &path,
                    "unsupported Bible book identifier",
                    Some(&Value::String(identifier.clone())),
                )
            })?;
            if parsed.contains_key(&book) {
                return Err(data_error(
                    BibleDataFormatErrorCode::InvalidValue,
                    &path,
                    "the same Bible book is declared more than once",
                    Some(&Value::String(identifier.clone())),
                ));
            }
            declared_order.push(book);
            parsed.insert(book, read_book(book, raw_book, &path, options.validation)?);
        }
        let order = read_book_order(root.get("bookOrder"), &parsed, &declared_order)?;
        let books: Vec<_> = order
            .into_iter()
            .map(|book| parsed.remove(&book).unwrap())
            .collect();
        validate_aliases(&books)?;
        let annotations = additional_fields(root, ROOT_FIELDS);
        Self::from_parts(
            books,
            language,
            metadata,
            schema_version,
            annotations,
            options.search_index_mode,
            Duration::ZERO,
        )
    }

    /// Construct a Bible directly from validated model values.
    pub fn from_books(
        books: Vec<Book>,
        language: Language,
        metadata: BibleMetadata,
        annotations: JsonMap,
        search_index_mode: SearchIndexMode,
    ) -> Result<Self, BibleDataFormatError> {
        let metadata = crate::source::merge_bible_metadata(
            Some(&metadata),
            None,
            (!language.is_auto()).then(|| language.display_name()),
            (!language.is_auto()).then(|| language.code()),
        )?;
        validate_aliases(&books)?;
        Self::from_parts(
            books,
            language,
            metadata,
            CURRENT_BIBLE_SCHEMA_VERSION,
            annotations,
            search_index_mode,
            Duration::ZERO,
        )
    }

    /// Derive a validated Bible value with selected replacements.
    pub fn copy_with(
        &self,
        books: Option<Vec<Book>>,
        language: Option<Language>,
        metadata: Option<BibleMetadata>,
        annotations: Option<JsonMap>,
        search_index_mode: Option<SearchIndexMode>,
    ) -> Result<Self, BibleDataFormatError> {
        let metadata = metadata.unwrap_or_else(|| self.metadata.clone());
        metadata.validate("$.metadata")?;
        let books = books.unwrap_or_else(|| self.books.clone());
        validate_aliases(&books)?;
        Self::from_parts(
            books,
            language.unwrap_or(self.language),
            metadata,
            self.schema_version,
            annotations.unwrap_or_else(|| self.annotations.clone()),
            search_index_mode.unwrap_or(self.search_index_mode),
            Duration::ZERO,
        )
    }

    fn from_parts(
        books: Vec<Book>,
        language: Language,
        metadata: BibleMetadata,
        schema_version: u32,
        annotations: JsonMap,
        search_index_mode: SearchIndexMode,
        load_time: Duration,
    ) -> Result<Self, BibleDataFormatError> {
        validate_annotations(&annotations, ROOT_FIELDS).map_err(|error| {
            BibleDataFormatError::new(
                BibleDataFormatErrorCode::ReservedField,
                "$",
                "Bible annotations must not shadow structural fields",
            )
            .with_cause(error)
        })?;
        let mut index_by_book = HashMap::new();
        for (index, book) in books.iter().enumerate() {
            if index_by_book.insert(book.book(), index).is_some() {
                return Err(BibleDataFormatError::new(
                    BibleDataFormatErrorCode::InvalidValue,
                    "$.books",
                    "Bible books must have unique identifiers",
                ));
            }
        }
        let reference_parser = build_reference_parser(&books, language)?;
        let passage_parser = PassageParser::from_reference_parser(reference_parser.clone());
        let bible = Self {
            books,
            index_by_book,
            reference_parser,
            passage_parser,
            search_index: RwLock::new(None),
            search_index_mode,
            schema_version,
            metadata,
            annotations,
            language,
            load_time,
            created_at: SystemTime::now(),
        };
        if search_index_mode == SearchIndexMode::Eager {
            *bible.search_index.write().expect("search lock poisoned") =
                Some(bible.build_search_index());
        }
        Ok(bible)
    }

    /// Return the versioned content-schema number.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }
    /// Return the full edition metadata.
    #[must_use]
    pub fn metadata(&self) -> &BibleMetadata {
        &self.metadata
    }
    /// Return losslessly retained root annotations.
    #[must_use]
    pub fn annotations(&self) -> &JsonMap {
        &self.annotations
    }
    /// Return the resolved reference language.
    #[must_use]
    pub const fn language_id(&self) -> Language {
        self.language
    }
    /// Return the configured index lifecycle policy.
    #[must_use]
    pub const fn search_index_mode(&self) -> SearchIndexMode {
        self.search_index_mode
    }
    /// Return when this in-memory Bible value was constructed.
    #[must_use]
    pub const fn created_at(&self) -> SystemTime {
        self.created_at
    }
    /// Return the edition ID, or an empty string for legacy content without one.
    #[must_use]
    pub fn id(&self) -> &str {
        self.metadata.id.as_deref().unwrap_or("")
    }
    /// Return the translation display name, or an empty string.
    #[must_use]
    pub fn name(&self) -> &str {
        self.metadata.translation_name.as_deref().unwrap_or("")
    }
    /// Return the translation description, or an empty string.
    #[must_use]
    pub fn description(&self) -> &str {
        self.metadata.description.as_deref().unwrap_or("")
    }
    /// Return the language display name, or an empty string.
    #[must_use]
    pub fn language(&self) -> &str {
        self.metadata.language_name.as_deref().unwrap_or("")
    }
    /// Return the optional ISO language code.
    #[must_use]
    pub fn language_code(&self) -> Option<&str> {
        self.metadata.language_code.as_deref()
    }
    /// Return the optional translation abbreviation.
    #[must_use]
    pub fn abbreviation(&self) -> Option<&str> {
        self.metadata.abbreviation.as_deref()
    }
    /// Return the optional publication year.
    #[must_use]
    pub const fn year(&self) -> Option<i32> {
        self.metadata.year
    }
    /// Return the optional source/provenance name.
    #[must_use]
    pub fn source_name(&self) -> Option<&str> {
        self.metadata.source_name.as_deref()
    }
    /// Return the optional content copyright statement.
    #[must_use]
    pub fn copyright(&self) -> Option<&str> {
        self.metadata.copyright.as_deref()
    }
    /// Return the optional content license.
    #[must_use]
    pub fn license(&self) -> Option<&str> {
        self.metadata.license.as_deref()
    }
    /// Return the optional canon label.
    #[must_use]
    pub fn canon(&self) -> Option<&str> {
        self.metadata.canon.as_deref()
    }
    /// Return the optional version date.
    #[must_use]
    pub fn version_date(&self) -> Option<&str> {
        self.metadata.version_date.as_deref()
    }
    /// Return the resolved text-direction hint.
    #[must_use]
    pub const fn direction(&self) -> crate::source::TextDirectionHint {
        self.metadata.direction
    }
    /// Return nested source provenance, when available.
    #[must_use]
    pub fn source(&self) -> Option<&BibleSource> {
        self.metadata.source.as_ref()
    }
    /// Return all books in edition order.
    #[must_use]
    pub fn books(&self) -> &[Book] {
        &self.books
    }
    /// Iterate every verse in edition order.
    pub fn all_verses(&self) -> impl Iterator<Item = &Verse> {
        self.books
            .iter()
            .flat_map(|book| book.chapters())
            .flat_map(Chapter::get_verses)
    }
    /// Fetch a book by canonical identifier.
    pub fn get_book(&self, book: BibleBook) -> Result<&Book, BibleError> {
        self.index_by_book
            .get(&book)
            .map(|index| &self.books[*index])
            .ok_or_else(|| BibleError::BookNotFound {
                book_abbrev: book.abbreviation().to_string(),
                book_name: book.full_name().to_string(),
                translation: self.name().to_string(),
            })
    }
    /// Fetch a book by any supported compact, full, OSIS, or USFM identifier.
    pub fn get_book_by_abbrev(&self, identifier: &str) -> Result<&Book, BibleError> {
        let book = parse_book_identifier(identifier).ok_or_else(|| BibleError::BookNotFound {
            book_abbrev: identifier.to_ascii_lowercase(),
            book_name: identifier.to_string(),
            translation: self.name().to_string(),
        })?;
        self.get_book(book)
    }
    /// Fetch a book by 1-based edition position.
    pub fn get_book_by_id(&self, position: usize) -> Result<&Book, BibleError> {
        self.books
            .get(position.wrapping_sub(1))
            .ok_or_else(|| BibleError::BookNotFound {
                book_abbrev: position.to_string(),
                book_name: position.to_string(),
                translation: self.name().to_string(),
            })
    }
    /// Fetch a chapter by declared number.
    pub fn get_chapter(&self, book: BibleBook, chapter: usize) -> Result<&Chapter, BibleError> {
        self.get_book(book)?.get_chapter(chapter)
    }
    /// Fetch every verse in a declared chapter.
    pub fn get_verses(&self, book: BibleBook, chapter: usize) -> Result<&[Verse], BibleError> {
        self.get_book(book)?.get_verses(chapter)
    }
    /// Fetch a verse by declared coordinates.
    pub fn get_verse(
        &self,
        book: BibleBook,
        chapter: usize,
        verse: usize,
    ) -> Result<&Verse, BibleError> {
        self.get_book(book)?.get_verse(chapter, verse)
    }
    /// Fetch a chapter at a stable location.
    pub fn get_chapter_at(&self, location: BibleLocation) -> Result<&Chapter, BibleError> {
        self.get_chapter(location.book(), location.chapter())
    }
    /// Fetch a verse at a stable location.
    pub fn get_verse_at(&self, location: BibleLocation) -> Result<&Verse, BibleError> {
        self.get_verse(
            location.book(),
            location.chapter(),
            location.verse().ok_or(BibleError::VerseRequired)?,
        )
    }
    /// Return whether a declared chapter or verse exists.
    #[must_use]
    pub fn contains_reference(&self, location: BibleLocation) -> bool {
        location.verse().map_or_else(
            || self.get_chapter_at(location).is_ok(),
            |_| self.get_verse_at(location).is_ok(),
        )
    }

    /// Parse a narrow verse or range reference with edition aliases and order.
    pub fn parse_reference(&self, input: &str) -> Result<EditionReference, BibleError> {
        self.parse_edition_reference(input)
    }
    /// Parse a reference and retain language-detection and ambiguity metadata.
    pub fn parse_reference_detailed(
        &self,
        input: &str,
    ) -> Result<EditionParsed<EditionReference>, BibleError> {
        self.parse_edition_reference_detailed(input)
    }
    /// Parse a reference using one explicit language and this edition's order.
    pub fn parse_reference_with_language(
        &self,
        input: &str,
        language: Language,
    ) -> Result<EditionReference, BibleError> {
        self.parse_edition_reference_with_language(input, language)
    }
    /// Parse a reference in one explicit language and retain parse metadata.
    pub fn parse_reference_detailed_with_language(
        &self,
        input: &str,
        language: Language,
    ) -> Result<EditionParsed<EditionReference>, BibleError> {
        self.parse_edition_reference_detailed_with_language(input, language)
    }
    /// Parse the dependency's canonically ordered reference type directly.
    pub fn parse_canonical_reference(&self, input: &str) -> Result<Reference, ParseError> {
        self.reference_parser.parse(input)
    }
    /// Parse the dependency's canonical reference and retain detection metadata.
    pub fn parse_canonical_reference_detailed(
        &self,
        input: &str,
    ) -> Result<bible_io_references::Parsed<Reference>, ParseError> {
        self.reference_parser.parse_detailed(input)
    }
    /// Parse the dependency's canonical reference in one explicit language.
    pub fn parse_canonical_reference_with_language(
        &self,
        input: &str,
        language: Language,
    ) -> Result<Reference, ParseError> {
        self.reference_parser.parse_with_language(input, language)
    }
    /// Parse a canonical reference in one language and retain metadata.
    pub fn parse_canonical_reference_detailed_with_language(
        &self,
        input: &str,
        language: Language,
    ) -> Result<bible_io_references::Parsed<Reference>, ParseError> {
        self.reference_parser
            .parse_detailed_with_language(input, language)
    }
    /// Parse a verse or range while allowing the range to use this edition's
    /// declared book order rather than the dependency's canonical order.
    pub fn parse_edition_reference(&self, input: &str) -> Result<EditionReference, BibleError> {
        self.parse_edition_reference_impl(input, None)
            .map(EditionParsed::into_parts)
            .map(|(value, _)| value)
    }
    /// Parse an edition-aware reference with one explicit input language.
    pub fn parse_edition_reference_with_language(
        &self,
        input: &str,
        language: Language,
    ) -> Result<EditionReference, BibleError> {
        self.parse_edition_reference_impl(input, Some(language))
            .map(EditionParsed::into_parts)
            .map(|(value, _)| value)
    }
    /// Parse an edition-aware reference while retaining dependency parse
    /// metadata when the shared parser can represent it.
    pub fn parse_edition_reference_detailed(
        &self,
        input: &str,
    ) -> Result<EditionParsed<EditionReference>, BibleError> {
        self.parse_edition_reference_impl(input, None)
    }
    /// Parse an edition-aware reference in one language and retain metadata.
    pub fn parse_edition_reference_detailed_with_language(
        &self,
        input: &str,
        language: Language,
    ) -> Result<EditionParsed<EditionReference>, BibleError> {
        self.parse_edition_reference_impl(input, Some(language))
    }
    /// Parse a rich passage expression with edition aliases and book order.
    pub fn parse_passage(&self, input: &str) -> Result<EditionPassage, BibleError> {
        self.parse_edition_passage_impl(input, None)
            .map(EditionParsed::into_parts)
            .map(|(value, _)| value)
    }
    /// Parse a rich passage and retain language-detection metadata.
    pub fn parse_passage_detailed(
        &self,
        input: &str,
    ) -> Result<EditionParsed<EditionPassage>, BibleError> {
        self.parse_edition_passage_impl(input, None)
    }
    /// Parse a rich passage using one explicit language.
    pub fn parse_passage_with_language(
        &self,
        input: &str,
        language: Language,
    ) -> Result<EditionPassage, BibleError> {
        self.parse_edition_passage_impl(input, Some(language))
            .map(EditionParsed::into_parts)
            .map(|(value, _)| value)
    }
    /// Parse a rich passage in one explicit language and retain metadata.
    pub fn parse_passage_detailed_with_language(
        &self,
        input: &str,
        language: Language,
    ) -> Result<EditionParsed<EditionPassage>, BibleError> {
        self.parse_edition_passage_impl(input, Some(language))
    }
    /// Parse the dependency's canonically ordered passage type directly.
    pub fn parse_canonical_passage(&self, input: &str) -> Result<Passage, ParseError> {
        self.passage_parser.parse(input)
    }
    /// Parse a canonical passage and retain language-detection metadata.
    pub fn parse_canonical_passage_detailed(
        &self,
        input: &str,
    ) -> Result<bible_io_references::Parsed<Passage>, ParseError> {
        self.passage_parser.parse_detailed(input)
    }
    /// Parse a canonical passage with one explicit input language.
    pub fn parse_canonical_passage_with_language(
        &self,
        input: &str,
        language: Language,
    ) -> Result<Passage, ParseError> {
        self.passage_parser.parse_with_language(input, language)
    }
    /// Parse a canonical passage in one language and retain metadata.
    pub fn parse_canonical_passage_detailed_with_language(
        &self,
        input: &str,
        language: Language,
    ) -> Result<bible_io_references::Parsed<Passage>, ParseError> {
        self.passage_parser
            .parse_detailed_with_language(input, language)
    }
    /// Retrieve a specific verse from a human-readable reference.
    pub fn get_verse_by_reference(&self, reference: &str) -> Result<&Verse, BibleError> {
        let input = reference.trim();
        let parsed = self.reference_parser.parse_verse(input).map_err(|error| {
            if error.kind() == ParseErrorKind::UnknownBook {
                BibleError::BookNotFound {
                    book_abbrev: reference_book_token(input).to_ascii_lowercase(),
                    book_name: reference_book_token(input).to_string(),
                    translation: self.name().to_string(),
                }
            } else {
                BibleError::ReferenceParse {
                    input: input.to_string(),
                    cause: error,
                }
            }
        })?;
        self.resolve_verse_reference(parsed)
    }
    /// Parse and resolve one verse, returning `None` for any invalid or absent reference.
    #[must_use]
    pub fn verse_or_none(&self, reference: &str) -> Option<&Verse> {
        self.get_verse_by_reference(reference).ok()
    }
    /// Resolve a typed verse coordinate.
    pub fn resolve_verse_reference(&self, reference: VerseRef) -> Result<&Verse, BibleError> {
        self.get_verse(
            reference.book(),
            usize::from(reference.chapter()),
            usize::from(reference.verse()),
        )
    }
    /// Resolve a typed narrow reference into edition-ordered verses.
    pub fn resolve_reference(&self, reference: Reference) -> Result<Vec<&Verse>, BibleError> {
        match reference {
            Reference::Verse(verse) => Ok(vec![self.resolve_verse_reference(verse)?]),
            Reference::Range(range) => self.resolve_verse_range(range),
        }
    }
    /// Resolve an edition-aware typed reference into edition-ordered verses.
    pub fn resolve_edition_reference(
        &self,
        reference: EditionReference,
    ) -> Result<Vec<&Verse>, BibleError> {
        match reference {
            EditionReference::Verse(verse) => Ok(vec![self.resolve_verse_reference(verse)?]),
            EditionReference::Range(range) => self.resolve_edition_range(range),
        }
    }
    /// Resolve a typed reference while preserving its single-versus-range shape.
    pub fn resolve_reference_result(
        &self,
        reference: Reference,
    ) -> Result<BibleReferenceResult<'_>, BibleError> {
        match reference {
            Reference::Verse(verse) => Ok(BibleReferenceResult::Verse(
                self.resolve_verse_reference(verse)?,
            )),
            Reference::Range(range) => Ok(BibleReferenceResult::Range(
                self.resolve_verse_range_selection(range)?,
            )),
        }
    }
    /// Resolve an edition-aware reference while retaining its result shape.
    pub fn resolve_edition_reference_result(
        &self,
        reference: EditionReference,
    ) -> Result<BibleReferenceResult<'_>, BibleError> {
        match reference {
            EditionReference::Verse(verse) => Ok(BibleReferenceResult::Verse(
                self.resolve_verse_reference(verse)?,
            )),
            EditionReference::Range(range) => Ok(BibleReferenceResult::Range(
                self.resolve_edition_range_selection(range)?,
            )),
        }
    }
    /// Resolve a typed reference into an immutable verse selection.
    pub fn resolve_reference_selection(
        &self,
        reference: Reference,
    ) -> Result<VerseSelection<'_>, BibleError> {
        self.resolve_reference(reference)
            .map(VerseSelection::from_vec)
    }
    /// Resolve an edition-aware reference into an immutable verse selection.
    pub fn resolve_edition_reference_selection(
        &self,
        reference: EditionReference,
    ) -> Result<VerseSelection<'_>, BibleError> {
        self.resolve_edition_reference(reference)
            .map(VerseSelection::from_vec)
    }
    /// Resolve an inclusive canonical range in loaded edition order.
    pub fn resolve_verse_range(&self, range: VerseRange) -> Result<Vec<&Verse>, BibleError> {
        self.resolve_edition_range(EditionVerseRange {
            start: range.start(),
            end: range.end(),
        })
    }
    /// Resolve an inclusive canonical range into an immutable selection.
    pub fn resolve_verse_range_selection(
        &self,
        range: VerseRange,
    ) -> Result<VerseSelection<'_>, BibleError> {
        self.resolve_verse_range(range)
            .map(VerseSelection::from_vec)
    }
    /// Resolve an inclusive range whose ordering follows this edition.
    pub fn resolve_edition_range(
        &self,
        range: EditionVerseRange,
    ) -> Result<Vec<&Verse>, BibleError> {
        self.resolve_verse_reference(range.start)?;
        self.resolve_verse_reference(range.end)?;
        let start = self.location_order(BibleLocation::from_verse_ref(range.start))?;
        let end = self.location_order(BibleLocation::from_verse_ref(range.end))?;
        if start > end {
            return Err(BibleError::InvalidRange {
                message: "verse range start must come before the end in edition order".to_string(),
            });
        }
        Ok(self
            .all_verses()
            .filter(|verse| {
                let order = self
                    .location_order(verse.location())
                    .expect("loaded verse has an order");
                order >= start && order <= end
            })
            .collect())
    }
    /// Resolve an edition-ordered range into an immutable selection.
    pub fn resolve_edition_range_selection(
        &self,
        range: EditionVerseRange,
    ) -> Result<VerseSelection<'_>, BibleError> {
        self.resolve_edition_range(range)
            .map(VerseSelection::from_vec)
    }
    /// Parse and resolve either one verse or an inclusive verse range.
    pub fn get_by_reference(&self, input: &str) -> Result<BibleReferenceResult<'_>, BibleError> {
        self.resolve_edition_reference_result(self.parse_reference(input)?)
    }
    /// Parse and resolve either one verse or a range in one explicit language.
    pub fn get_by_reference_with_language(
        &self,
        input: &str,
        language: Language,
    ) -> Result<BibleReferenceResult<'_>, BibleError> {
        self.resolve_edition_reference_result(self.parse_reference_with_language(input, language)?)
    }
    /// Parse and resolve a range, including a range ordered by custom bookOrder.
    pub fn get_verse_range_by_reference(&self, input: &str) -> Result<Vec<&Verse>, BibleError> {
        match self.reference_parser.parse_range(input) {
            Ok(range) => self.resolve_verse_range(range),
            Err(cause) => match self.parse_edition_range(input, None) {
                Ok(range) => self.resolve_edition_range(range),
                Err(_) => Err(BibleError::ReferenceParse {
                    input: input.trim().to_string(),
                    cause,
                }),
            },
        }
    }
    /// Parse and resolve a range into an immutable verse selection.
    pub fn get_verse_range_selection_by_reference(
        &self,
        input: &str,
    ) -> Result<VerseSelection<'_>, BibleError> {
        self.get_verse_range_by_reference(input)
            .map(VerseSelection::from_vec)
    }
    /// Parse and resolve a verse range, returning `None` on failure.
    #[must_use]
    pub fn verses_or_none(&self, input: &str) -> Option<Vec<&Verse>> {
        self.get_verse_range_by_reference(input).ok()
    }
    /// Resolve a rich passage and preserve selection/sequence duplicates.
    pub fn resolve_passage(&self, passage: &Passage) -> Result<Vec<&Verse>, BibleError> {
        match passage {
            Passage::Book(value) => self.resolve_book_passage(*value),
            Passage::Chapter(value) => self.resolve_chapter_passage(*value),
            Passage::Verses(value) => self.resolve_verse_passage(value),
            Passage::Sequence(value) => self.resolve_passage_sequence(value),
        }
    }
    /// Resolve a rich passage into an immutable, source-ordered selection.
    pub fn resolve_passage_selection(
        &self,
        passage: &Passage,
    ) -> Result<VerseSelection<'_>, BibleError> {
        self.resolve_passage(passage).map(VerseSelection::from_vec)
    }
    /// Resolve a passage parsed with this edition's custom canon semantics.
    pub fn resolve_edition_passage(
        &self,
        passage: &EditionPassage,
    ) -> Result<Vec<&Verse>, BibleError> {
        match passage {
            EditionPassage::Standard(passage) => self.resolve_passage(passage),
            EditionPassage::Reference(reference) => self.resolve_edition_reference(*reference),
        }
    }
    /// Resolve an edition-aware passage into an immutable selection.
    pub fn resolve_edition_passage_selection(
        &self,
        passage: &EditionPassage,
    ) -> Result<VerseSelection<'_>, BibleError> {
        self.resolve_edition_passage(passage)
            .map(VerseSelection::from_vec)
    }
    /// Parse and resolve a rich passage expression.
    pub fn get_passage(&self, input: &str) -> Result<Vec<&Verse>, BibleError> {
        let passage = self.parse_passage(input)?;
        self.resolve_edition_passage(&passage)
    }
    /// Parse and resolve a rich passage into an immutable selection.
    pub fn get_passage_selection(&self, input: &str) -> Result<VerseSelection<'_>, BibleError> {
        let passage = self.parse_passage(input)?;
        self.resolve_edition_passage_selection(&passage)
    }

    fn resolve_book_passage(&self, passage: BookPassage) -> Result<Vec<&Verse>, BibleError> {
        Ok(self
            .get_book(passage.book())?
            .chapters()
            .iter()
            .flat_map(Chapter::get_verses)
            .collect())
    }
    fn resolve_chapter_passage(&self, passage: ChapterPassage) -> Result<Vec<&Verse>, BibleError> {
        let end = passage.end_chapter().unwrap_or(passage.start_chapter());
        let mut verses = Vec::new();
        for chapter in passage.start_chapter()..=end {
            verses.extend(self.get_verses(passage.book(), usize::from(chapter))?);
        }
        Ok(verses)
    }
    fn resolve_verse_passage(&self, passage: &VersePassage) -> Result<Vec<&Verse>, BibleError> {
        let mut verses = Vec::new();
        for reference in passage.selections() {
            verses.extend(self.resolve_reference(*reference)?);
        }
        Ok(verses)
    }
    fn resolve_passage_sequence(
        &self,
        sequence: &PassageSequence,
    ) -> Result<Vec<&Verse>, BibleError> {
        let mut verses = Vec::new();
        for passage in sequence.passages() {
            verses.extend(self.resolve_passage(passage)?);
        }
        Ok(verses)
    }

    /// Format a loaded location with its edition-specific book name.
    pub fn format_location(&self, location: BibleLocation) -> Result<String, BibleError> {
        if location.has_verse() {
            self.get_verse_at(location)?;
        } else {
            self.get_chapter_at(location)?;
        }
        let name = self.get_book(location.book())?.title();
        Ok(location.verse().map_or_else(
            || format!("{name} {}", location.chapter()),
            |verse| format!("{name} {}:{verse}", location.chapter()),
        ))
    }
    /// Format with a chosen reference-package formatter.
    pub fn format_location_with(
        &self,
        location: BibleLocation,
        formatter: ReferenceFormatter,
    ) -> Result<String, BibleError> {
        if location.has_verse() {
            let reference = location
                .to_verse_ref()
                .map_err(|_| BibleError::VerseRequired)?;
            self.resolve_verse_reference(reference)?;
            Ok(formatter.format(reference).to_string())
        } else {
            self.get_chapter_at(location)?;
            Ok(format!(
                "{} {}",
                formatter.book_name(location.book()),
                location.chapter()
            ))
        }
    }
    /// Create an edition-aware persisted-state key for a loaded verse.
    pub fn key_for_verse(&self, verse: &Verse) -> Result<BibleVerseKey, BibleError> {
        let id = self
            .metadata
            .id
            .as_deref()
            .filter(|id| !id.trim().is_empty())
            .ok_or(BibleError::MissingEditionId)?;
        if self.get_verse(verse.book(), verse.chapter(), verse.number())? != verse {
            return Err(BibleError::InvalidReference {
                input: "verse does not belong to this edition".to_string(),
            });
        }
        BibleVerseKey::from_verse(id, verse).map_err(|error| BibleError::InvalidReference {
            input: error.to_string(),
        })
    }
    /// Create a persisted key from a validated verse location.
    pub fn key_for_location(&self, location: BibleLocation) -> Result<BibleVerseKey, BibleError> {
        self.key_for_verse(self.get_verse_at(location)?)
    }

    /// Return the next declared chapter, crossing book boundaries.
    pub fn next_chapter(
        &self,
        current: BibleLocation,
    ) -> Result<Option<BibleLocation>, BibleError> {
        self.adjacent_chapter(current, true)
    }
    /// Return the previous declared chapter, crossing book boundaries.
    pub fn previous_chapter(
        &self,
        current: BibleLocation,
    ) -> Result<Option<BibleLocation>, BibleError> {
        self.adjacent_chapter(current, false)
    }
    /// Return whether another declared chapter follows this location.
    pub fn has_next_chapter(&self, current: BibleLocation) -> Result<bool, BibleError> {
        self.next_chapter(current).map(|value| value.is_some())
    }
    /// Return whether another declared chapter precedes this location.
    pub fn has_previous_chapter(&self, current: BibleLocation) -> Result<bool, BibleError> {
        self.previous_chapter(current).map(|value| value.is_some())
    }
    fn adjacent_chapter(
        &self,
        current: BibleLocation,
        next: bool,
    ) -> Result<Option<BibleLocation>, BibleError> {
        let book_index = *self
            .index_by_book
            .get(&current.book())
            .ok_or_else(|| self.missing_book(current.book()))?;
        let book = &self.books[book_index];
        let chapter_index = book
            .chapters()
            .binary_search_by_key(&current.chapter(), Chapter::number)
            .map_err(|_| book.get_chapter(current.chapter()).unwrap_err())?;
        if next {
            if let Some(chapter) = book.chapters().get(chapter_index + 1) {
                return Ok(Some(
                    BibleLocation::new(current.book(), chapter.number(), None).unwrap(),
                ));
            }
            for book in &self.books[book_index + 1..] {
                if let Some(chapter) = book.chapters().first() {
                    return Ok(Some(
                        BibleLocation::new(book.book(), chapter.number(), None).unwrap(),
                    ));
                }
            }
        } else {
            if chapter_index > 0 {
                let chapter = &book.chapters()[chapter_index - 1];
                return Ok(Some(
                    BibleLocation::new(current.book(), chapter.number(), None).unwrap(),
                ));
            }
            for book in self.books[..book_index].iter().rev() {
                if let Some(chapter) = book.chapters().last() {
                    return Ok(Some(
                        BibleLocation::new(book.book(), chapter.number(), None).unwrap(),
                    ));
                }
            }
        }
        Ok(None)
    }
    /// Return the next declared verse, crossing sparse chapters and books.
    pub fn next_verse(&self, current: BibleLocation) -> Result<Option<BibleLocation>, BibleError> {
        self.adjacent_verse(current, true)
    }
    /// Return the previous declared verse.
    pub fn previous_verse(
        &self,
        current: BibleLocation,
    ) -> Result<Option<BibleLocation>, BibleError> {
        self.adjacent_verse(current, false)
    }
    /// Return whether another declared verse follows this location.
    pub fn has_next_verse(&self, current: BibleLocation) -> Result<bool, BibleError> {
        self.next_verse(current).map(|value| value.is_some())
    }
    /// Return whether another declared verse precedes this location.
    pub fn has_previous_verse(&self, current: BibleLocation) -> Result<bool, BibleError> {
        self.previous_verse(current).map(|value| value.is_some())
    }
    fn adjacent_verse(
        &self,
        current: BibleLocation,
        next: bool,
    ) -> Result<Option<BibleLocation>, BibleError> {
        self.get_verse_at(current)?;
        let locations: Vec<_> = self.all_verses().map(Verse::location).collect();
        let index = locations
            .iter()
            .position(|location| *location == current)
            .expect("validated location is loaded");
        Ok(if next {
            locations.get(index + 1).copied()
        } else {
            index
                .checked_sub(1)
                .and_then(|index| locations.get(index).copied())
        })
    }

    /// Return whether a search index is currently retained.
    #[must_use]
    pub fn has_search_index(&self) -> bool {
        self.search_index
            .read()
            .expect("search lock poisoned")
            .is_some()
    }
    /// Build and retain the index unless indexing is disabled.
    pub fn prewarm_search_index(&self) {
        if self.search_index_mode != SearchIndexMode::Disabled && !self.has_search_index() {
            *self.search_index.write().expect("search lock poisoned") =
                Some(self.build_search_index());
        }
    }
    /// Release the retained index.
    pub fn clear_search_index(&self) {
        *self.search_index.write().expect("search lock poisoned") = None;
    }
    /// Build a reusable default all-term index.
    #[must_use]
    pub fn build_search_index(&self) -> SearchIndex {
        let mut index: HashMap<String, Vec<_>> = HashMap::new();
        for verse in self.all_verses() {
            let location = (verse.book(), verse.chapter(), verse.number());
            for term in build_search_index_terms(verse.text(), 3) {
                index.entry(term).or_default().push(location);
            }
        }
        SearchIndex::new(index)
    }
    /// Fast all-distinct-term search in stable edition order.
    #[must_use]
    pub fn search(&self, query: &str) -> Vec<Verse> {
        let options = SearchOptions {
            mode: crate::search::SearchMode::All,
            ..SearchOptions::default()
        };
        if tokenize_search_text(query, false, true, false).is_empty() {
            return Vec::new();
        }
        if self.search_index_mode == SearchIndexMode::Disabled {
            return self.search_internal(query, &options, None).into_verses();
        }
        self.prewarm_search_index();
        self.search_index
            .read()
            .expect("search lock poisoned")
            .as_ref()
            .expect("index was prewarmed")
            .search(query)
            .into_iter()
            .filter_map(|(book, chapter, verse)| self.get_verse(book, chapter, verse).ok())
            .filter(|verse| matches_search_text(verse.text(), query, &options))
            .cloned()
            .collect()
    }
    /// Return loaded books paired with their zero-based edition positions.
    pub fn books_with_index(&self) -> impl Iterator<Item = (usize, &Book)> {
        self.books.iter().enumerate()
    }
    /// Return books containing one complete normalized word.
    #[must_use]
    pub fn books_containing(&self, word: &str) -> Vec<&Book> {
        self.books
            .iter()
            .filter(|book| !book.chapters_containing(word).is_empty())
            .collect()
    }
    /// Advanced search with explicit modes, scope, normalization, and paging.
    pub fn search_with_options(
        &self,
        query: &str,
        options: &SearchOptions,
    ) -> Result<SearchResults, ModelError> {
        options
            .validate()
            .map_err(|message| ModelError::new("options", message))?;
        Ok(self.search_internal(query, options, None))
    }
    /// Typo-tolerant search using bounded Unicode-scalar edit distance.
    pub fn fuzzy_search(
        &self,
        query: &str,
        max_distance: usize,
        options: &SearchOptions,
    ) -> Result<SearchResults, ModelError> {
        options
            .validate()
            .map_err(|message| ModelError::new("options", message))?;
        Ok(self.search_internal(query, options, Some(max_distance)))
    }
    fn search_internal(
        &self,
        query: &str,
        options: &SearchOptions,
        fuzzy: Option<usize>,
    ) -> SearchResults {
        let has_text = !query.trim().is_empty();
        let limit = options.max_results.unwrap_or(usize::MAX);
        let mut matched_count = 0_usize;
        let mut page = Vec::new();
        let mut has_more = false;
        for verse in self.all_verses().filter(|verse| {
            self.matches_scope(verse, options)
                && if !has_text {
                    fuzzy.is_none()
                } else if let Some(distance) = fuzzy {
                    fuzzy_matches(verse.text(), query, options, distance)
                } else {
                    matches_search_text(verse.text(), query, options)
                }
        }) {
            if matched_count < options.offset {
                matched_count += 1;
                continue;
            }
            if page.len() == limit {
                has_more = true;
                break;
            }
            page.push(verse);
            matched_count += 1;
        }
        let total_count = (!has_more).then_some(matched_count);
        let hits: Vec<_> = page
            .iter()
            .map(|verse| {
                let ranges = fuzzy.map_or_else(
                    || find_match_ranges(verse.text(), query, options),
                    |distance| {
                        fuzzy_match_ranges(verse.text(), query, options, distance)
                            .unwrap_or_default()
                    },
                );
                SearchHit::with_context(
                    (*verse).clone(),
                    self.get_book(verse.book()).expect("loaded book"),
                    ranges,
                    160,
                )
                .expect("internally constructed hit is valid")
            })
            .collect();
        SearchResults::from_hits(
            query,
            hits,
            options.offset,
            options.max_results,
            total_count,
            has_more,
        )
        .expect("internally constructed search page is valid")
    }
    fn matches_scope(&self, verse: &Verse, options: &SearchOptions) -> bool {
        options.book.is_none_or(|book| book == verse.book())
            && options
                .chapter
                .is_none_or(|chapter| chapter == verse.chapter())
            && options.verse.is_none_or(|number| number == verse.number())
    }

    /// Return aggregate counts and averages.
    #[must_use]
    pub fn stats(&self) -> BibleStats {
        let verses: Vec<_> = self.all_verses().collect();
        let total_characters: usize = verses.iter().map(|verse| verse.len()).sum();
        let mut verses_per_book = self
            .books
            .iter()
            .map(|book| (book.book(), 0))
            .collect::<HashMap<_, _>>();
        for verse in &verses {
            *verses_per_book.entry(verse.book()).or_insert(0) += 1;
        }
        BibleStats {
            book_count: self.books.len(),
            chapter_count: self.books.iter().map(|book| book.chapters().len()).sum(),
            verse_count: verses.len(),
            total_words: verses
                .iter()
                .map(|verse| extract_unicode_words(verse.text()).len())
                .sum(),
            average_verse_length: if verses.is_empty() {
                0
            } else {
                (total_characters as f64 / verses.len() as f64).round() as usize
            },
            verses_per_book,
        }
    }
    /// Return load and retained-index diagnostics.
    #[must_use]
    pub fn performance_metrics(&self) -> BiblePerformanceMetrics {
        let guard = self.search_index.read().expect("search lock poisoned");
        let verses: Vec<_> = self.all_verses().collect();
        let text_bytes = verses.iter().map(|verse| verse.text().len()).sum::<usize>();
        let posting_count = guard.as_ref().map_or(0, SearchIndex::posting_count);
        let search_index_size = guard.as_ref().map_or(0, SearchIndex::len);
        BiblePerformanceMetrics {
            load_time: self.load_time,
            search_index_size,
            search_index_built: guard.is_some(),
            verse_count: verses.len(),
            posting_count,
            text_bytes,
            memory_usage_kib: (text_bytes + verses.len() * 64 + posting_count * 24).div_ceil(1024),
        }
    }
    /// Serialize the complete versioned content contract without data loss.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(&self.to_json_value())
    }
    /// Return the complete versioned content as a JSON value.
    #[must_use]
    pub fn to_json_value(&self) -> Value {
        let mut root = self.annotations.clone();
        root.insert(
            "schemaVersion".to_string(),
            Value::from(self.schema_version),
        );
        root.insert(
            "language".to_string(),
            Value::String(self.language.display_name().to_string()),
        );
        root.insert(
            "metadata".to_string(),
            serde_json::to_value(&self.metadata).expect("metadata contains JSON values"),
        );
        root.insert(
            "bookOrder".to_string(),
            Value::Array(
                self.books
                    .iter()
                    .map(|book| Value::String(book.abbrev().to_string()))
                    .collect(),
            ),
        );
        root.insert(
            "books".to_string(),
            Value::Object(
                self.books
                    .iter()
                    .map(|book| (book.abbrev().to_string(), book.to_json_value()))
                    .collect(),
            ),
        );
        Value::Object(root)
    }

    fn parse_edition_reference_impl(
        &self,
        input: &str,
        language: Option<Language>,
    ) -> Result<EditionParsed<EditionReference>, BibleError> {
        let parsed = match language {
            Some(language) => self
                .reference_parser
                .parse_detailed_with_language(input, language),
            None => self.reference_parser.parse_detailed(input),
        };
        match parsed {
            Ok(parsed) => {
                let (reference, metadata) = parsed.into_parts();
                let reference = match reference {
                    Reference::Verse(verse) => EditionReference::Verse(verse),
                    Reference::Range(range) => EditionReference::Range(EditionVerseRange {
                        start: range.start(),
                        end: range.end(),
                    }),
                };
                Ok(EditionParsed::new(reference, Some(metadata)))
            }
            Err(cause) if cause.kind() == ParseErrorKind::CrossBookRangeNotAscending => self
                .parse_edition_range(input, language)
                .map(|range| EditionParsed::new(EditionReference::Range(range), None))
                .map_err(|_| BibleError::ReferenceParse {
                    input: input.trim().to_string(),
                    cause,
                }),
            Err(cause) => Err(BibleError::ReferenceParse {
                input: input.trim().to_string(),
                cause,
            }),
        }
    }

    fn parse_edition_passage_impl(
        &self,
        input: &str,
        language: Option<Language>,
    ) -> Result<EditionParsed<EditionPassage>, BibleError> {
        let parsed = match language {
            Some(language) => self
                .passage_parser
                .parse_detailed_with_language(input, language),
            None => self.passage_parser.parse_detailed(input),
        };
        match parsed {
            Ok(parsed) => {
                let (passage, metadata) = parsed.into_parts();
                Ok(EditionParsed::new(
                    EditionPassage::Standard(passage),
                    Some(metadata),
                ))
            }
            Err(cause) if cause.kind() == ParseErrorKind::CrossBookRangeNotAscending => self
                .parse_edition_range(input, language)
                .map(|range| {
                    EditionParsed::new(
                        EditionPassage::Reference(EditionReference::Range(range)),
                        None,
                    )
                })
                .map_err(|_| BibleError::ReferenceParse {
                    input: input.trim().to_string(),
                    cause,
                }),
            Err(cause) => Err(BibleError::ReferenceParse {
                input: input.trim().to_string(),
                cause,
            }),
        }
    }

    fn parse_edition_range(
        &self,
        input: &str,
        language: Option<Language>,
    ) -> Result<EditionVerseRange, BibleError> {
        for (index, character) in input
            .char_indices()
            .filter(|(_, character)| matches!(character, '-' | '–' | '—' | '―'))
        {
            let left = input[..index].trim();
            let right = input[index + character.len_utf8()..].trim();
            let parse_verse = |value| match language {
                Some(language) => self
                    .reference_parser
                    .parse_verse_with_language(value, language),
                None => self.reference_parser.parse_verse(value),
            };
            if let (Ok(start), Ok(end)) = (parse_verse(left), parse_verse(right)) {
                let range = EditionVerseRange { start, end };
                if let (Ok(start_order), Ok(end_order)) = (
                    self.location_order(BibleLocation::from_verse_ref(start)),
                    self.location_order(BibleLocation::from_verse_ref(end)),
                ) {
                    if start.book() != end.book() && start_order < end_order {
                        return Ok(range);
                    }
                }
            }
        }
        Err(BibleError::InvalidReference {
            input: input.to_string(),
        })
    }
    fn location_order(&self, location: BibleLocation) -> Result<(usize, usize, usize), BibleError> {
        let book = *self
            .index_by_book
            .get(&location.book())
            .ok_or_else(|| self.missing_book(location.book()))?;
        Ok((book, location.chapter(), location.verse().unwrap_or(0)))
    }
    fn missing_book(&self, book: BibleBook) -> BibleError {
        BibleError::BookNotFound {
            book_abbrev: book.abbreviation().to_string(),
            book_name: book.full_name().to_string(),
            translation: self.name().to_string(),
        }
    }
}

static EMPTY_MAP: OnceLock<Map<String, Value>> = OnceLock::new();

fn build_reference_parser(
    books: &[Book],
    language: Language,
) -> Result<ReferenceParser, BibleDataFormatError> {
    let mut builder = ReferenceParser::builder();
    if !language.is_auto() && language.is_parsing_supported() {
        builder = builder.preferred_languages([language]);
    }
    for &(alias, book) in LEGACY_REFERENCE_ALIASES {
        builder = builder.language_alias(Language::English, alias, book);
    }
    for book in books {
        if !book.title().eq_ignore_ascii_case(book.book().full_name()) {
            builder = if language.is_auto() {
                builder.alias(book.title(), book.book())
            } else {
                builder.language_alias(language, book.title(), book.book())
            };
        }
    }
    builder.build().map_err(|error| {
        BibleDataFormatError::new(
            BibleDataFormatErrorCode::InvalidValue,
            "$.books",
            "loaded book names create an ambiguous reference alias",
        )
        .with_cause(error)
    })
}

fn read_metadata(
    root: &Map<String, Value>,
    source: Option<&BibleSource>,
) -> Result<BibleMetadata, BibleDataFormatError> {
    let document = Value::Object(root.clone());
    let mut metadata = BibleMetadata::from_document_value(&document, source)?;
    // Root-level unknown fields belong to Bible annotations, not the nested
    // metadata extension map. Keep only values that originated in metadata.
    if let Some(Value::Object(nested)) = root.get("metadata") {
        metadata
            .additional
            .retain(|key, _| nested.contains_key(key));
    } else {
        metadata.additional.clear();
    }
    Ok(metadata)
}

fn resolve_language(
    raw: Option<&Value>,
    metadata: &BibleMetadata,
) -> Result<Language, BibleDataFormatError> {
    if let Some(value) = raw {
        if !value.is_string() {
            return Err(data_error(
                BibleDataFormatErrorCode::InvalidType,
                "$.language",
                "Bible language must be a string",
                Some(value),
            ));
        }
    }
    for candidate in [
        raw.and_then(Value::as_str),
        metadata.language_code.as_deref(),
        metadata.language_name.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        if let Ok(language) = candidate.parse() {
            return Ok(language);
        }
    }
    Ok(Language::Auto)
}

fn read_book(
    book: BibleBook,
    value: &Value,
    path: &str,
    validation: BibleDataValidationOptions,
) -> Result<Book, BibleDataFormatError> {
    let object = value.as_object().ok_or_else(|| {
        data_error(
            BibleDataFormatErrorCode::InvalidType,
            path,
            "a Bible book must be an object",
            Some(value),
        )
    })?;
    let title = match object.get("name") {
        None => book.full_name().to_string(),
        Some(Value::String(name)) if !name.trim().is_empty() => name.clone(),
        Some(Value::String(name)) => {
            return Err(data_error(
                BibleDataFormatErrorCode::InvalidValue,
                &format!("{path}.name"),
                "book name must be a non-blank string",
                Some(&Value::String(name.clone())),
            ));
        }
        other => {
            return Err(data_error(
                BibleDataFormatErrorCode::InvalidType,
                &format!("{path}.name"),
                "book name must be a non-blank string",
                other,
            ));
        }
    };

    let chapters_value = object.get("chapters");
    let mut chapters = Vec::new();
    match chapters_value {
        Some(Value::Object(chapters_object)) => {
            if validation.require_chapters && chapters_object.is_empty() {
                return Err(data_error(
                    BibleDataFormatErrorCode::InvalidValue,
                    &format!("{path}.chapters"),
                    "a Bible book must contain at least one chapter",
                    chapters_value,
                ));
            }
            let mut numbers = HashSet::new();
            for (key, chapter) in chapters_object {
                let chapter_path = json_path(&format!("{path}.chapters"), key);
                let number = positive_key(key, &chapter_path)?;
                if !numbers.insert(number) {
                    return Err(data_error(
                        BibleDataFormatErrorCode::InvalidValue,
                        &chapter_path,
                        "duplicate numeric chapter number",
                        Some(&Value::String(key.clone())),
                    ));
                }
                chapters.push(read_chapter(
                    book,
                    number,
                    chapter,
                    &chapter_path,
                    validation,
                )?);
            }
        }
        // Preserve the package's pre-1.1 array input while always serializing
        // the unambiguous versioned map representation.
        Some(Value::Array(chapter_values)) => {
            if validation.require_chapters && chapter_values.is_empty() {
                return Err(data_error(
                    BibleDataFormatErrorCode::InvalidValue,
                    &format!("{path}.chapters"),
                    "a Bible book must contain at least one chapter",
                    chapters_value,
                ));
            }
            for (index, chapter) in chapter_values.iter().enumerate() {
                let number = index + 1;
                chapters.push(read_chapter(
                    book,
                    number,
                    chapter,
                    &format!("{path}.chapters[{index}]"),
                    validation,
                )?);
            }
        }
        None if !validation.require_chapters => {}
        None => {
            return Err(BibleDataFormatError::new(
                BibleDataFormatErrorCode::MissingField,
                format!("{path}.chapters"),
                "a Bible book must declare chapters",
            ));
        }
        other => {
            return Err(data_error(
                BibleDataFormatErrorCode::InvalidType,
                &format!("{path}.chapters"),
                "book chapters must be an object or legacy array",
                other,
            ));
        }
    }
    Book::checked(
        book,
        title,
        chapters,
        additional_fields(object, &["name", "chapters"]),
    )
    .map_err(|error| {
        BibleDataFormatError::new(
            BibleDataFormatErrorCode::InvalidValue,
            path,
            "Bible book violates model invariants",
        )
        .with_cause(error)
    })
}

fn read_chapter(
    book: BibleBook,
    number: usize,
    value: &Value,
    path: &str,
    validation: BibleDataValidationOptions,
) -> Result<Chapter, BibleDataFormatError> {
    let (verses_value, verses_path, annotations) = match value {
        Value::Object(object) if object.contains_key("verses") => (
            object.get("verses"),
            format!("{path}.verses"),
            additional_fields(object, &["verses"]),
        ),
        Value::Object(_) | Value::Array(_) => (Some(value), path.to_string(), Map::new()),
        _ => {
            return Err(data_error(
                BibleDataFormatErrorCode::InvalidType,
                path,
                "a Bible chapter must be an object or legacy array",
                Some(value),
            ));
        }
    };
    let mut verses = Vec::new();
    match verses_value {
        Some(Value::Object(verses_object)) => {
            if validation.require_verses && verses_object.is_empty() {
                return Err(data_error(
                    BibleDataFormatErrorCode::InvalidValue,
                    &verses_path,
                    "a Bible chapter must contain at least one verse",
                    verses_value,
                ));
            }
            let mut numbers = HashSet::new();
            for (key, verse) in verses_object {
                let verse_path = json_path(&verses_path, key);
                let verse_number = positive_key(key, &verse_path)?;
                if !numbers.insert(verse_number) {
                    return Err(data_error(
                        BibleDataFormatErrorCode::InvalidValue,
                        &verse_path,
                        "duplicate numeric verse number",
                        Some(&Value::String(key.clone())),
                    ));
                }
                verses.push(read_verse(
                    book,
                    number,
                    verse_number,
                    verse,
                    &verse_path,
                    validation,
                )?);
            }
        }
        Some(Value::Array(verse_values)) => {
            if validation.require_verses && verse_values.is_empty() {
                return Err(data_error(
                    BibleDataFormatErrorCode::InvalidValue,
                    &verses_path,
                    "a Bible chapter must contain at least one verse",
                    verses_value,
                ));
            }
            for (index, verse) in verse_values.iter().enumerate() {
                verses.push(read_verse(
                    book,
                    number,
                    index + 1,
                    verse,
                    &format!("{verses_path}[{index}]"),
                    validation,
                )?);
            }
        }
        other => {
            return Err(data_error(
                BibleDataFormatErrorCode::InvalidType,
                &verses_path,
                "chapter verses must be an object or legacy array",
                other,
            ));
        }
    }
    Chapter::checked(book, number, verses, annotations).map_err(|error| {
        BibleDataFormatError::new(
            BibleDataFormatErrorCode::InvalidValue,
            path,
            "Bible chapter violates model invariants",
        )
        .with_cause(error)
    })
}

fn read_verse(
    book: BibleBook,
    chapter: usize,
    number: usize,
    value: &Value,
    path: &str,
    validation: BibleDataValidationOptions,
) -> Result<Verse, BibleDataFormatError> {
    let (text, annotations, text_path) = match value {
        Value::String(text) => (text.clone(), Map::new(), path.to_string()),
        Value::Object(object) => {
            let text = match object.get("text") {
                Some(Value::String(text)) => text.clone(),
                None => {
                    return Err(BibleDataFormatError::new(
                        BibleDataFormatErrorCode::MissingField,
                        format!("{path}.text"),
                        "an annotated verse must declare text",
                    ))
                }
                other => {
                    return Err(data_error(
                        BibleDataFormatErrorCode::InvalidType,
                        &format!("{path}.text"),
                        "verse text must be a string",
                        other,
                    ))
                }
            };
            (
                text,
                additional_fields(object, &["text"]),
                format!("{path}.text"),
            )
        }
        _ => {
            return Err(data_error(
                BibleDataFormatErrorCode::InvalidType,
                path,
                "a verse must be a string or object containing text",
                Some(value),
            ))
        }
    };
    if validation.require_verse_text && text.trim().is_empty() {
        return Err(data_error(
            BibleDataFormatErrorCode::InvalidValue,
            &text_path,
            "verse text must not be blank",
            Some(&Value::String(text.clone())),
        ));
    }
    Verse::checked(book, chapter, number, text, annotations).map_err(|error| {
        BibleDataFormatError::new(
            BibleDataFormatErrorCode::InvalidValue,
            path,
            "Bible verse violates model invariants",
        )
        .with_cause(error)
    })
}

fn read_book_order(
    raw: Option<&Value>,
    books: &HashMap<BibleBook, Book>,
    declared_order: &[BibleBook],
) -> Result<Vec<BibleBook>, BibleDataFormatError> {
    let Some(raw) = raw else {
        let mut order: Vec<_> = books.keys().copied().collect();
        order.sort();
        return Ok(order);
    };
    let values = raw.as_array().ok_or_else(|| {
        data_error(
            BibleDataFormatErrorCode::InvalidType,
            "$.bookOrder",
            "bookOrder must be an array",
            Some(raw),
        )
    })?;
    let mut order = Vec::new();
    let mut seen = HashSet::new();
    for (index, value) in values.iter().enumerate() {
        let path = format!("$.bookOrder[{index}]");
        let identifier = value.as_str().ok_or_else(|| {
            data_error(
                BibleDataFormatErrorCode::InvalidType,
                &path,
                "bookOrder items must be strings",
                Some(value),
            )
        })?;
        if identifier.trim().is_empty() {
            return Err(data_error(
                BibleDataFormatErrorCode::InvalidValue,
                &path,
                "bookOrder items must be non-blank strings",
                Some(value),
            ));
        }
        let book = parse_book_identifier(identifier).ok_or_else(|| {
            data_error(
                BibleDataFormatErrorCode::InvalidValue,
                &path,
                "unsupported Bible book identifier",
                Some(value),
            )
        })?;
        if !books.contains_key(&book) || !seen.insert(book) {
            return Err(data_error(
                BibleDataFormatErrorCode::InvalidValue,
                &path,
                "bookOrder references an absent or duplicate book",
                Some(value),
            ));
        }
        order.push(book);
    }
    if order.len() != books.len() {
        let missing = declared_order
            .iter()
            .filter(|book| !seen.contains(book))
            .map(|book| Value::String(book.abbreviation().to_string()))
            .collect();
        return Err(BibleDataFormatError::new(
            BibleDataFormatErrorCode::InvalidValue,
            "$.bookOrder",
            "bookOrder must list every loaded book exactly once",
        )
        .with_value(Value::Array(missing)));
    }
    Ok(order)
}

fn validate_aliases(books: &[Book]) -> Result<(), BibleDataFormatError> {
    let mut owners = HashMap::new();
    for book in books {
        for term in [
            book.book().full_name(),
            book.book().abbreviation(),
            book.title(),
        ] {
            let normalized = normalize_search_text(
                &term.split_whitespace().collect::<Vec<_>>().join(" "),
                false,
                true,
                false,
            );
            if let Some(owner) = owners.insert(normalized, book.book()) {
                if owner != book.book() {
                    return Err(BibleDataFormatError::new(
                        BibleDataFormatErrorCode::InvalidValue,
                        "$.books",
                        "loaded book names create an ambiguous reference alias",
                    ));
                }
            }
        }
    }
    Ok(())
}
fn positive_key(value: &str, path: &str) -> Result<usize, BibleDataFormatError> {
    value
        .trim()
        .parse::<i64>()
        .ok()
        .filter(|value| *value > 0)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| {
            data_error(
                BibleDataFormatErrorCode::InvalidValue,
                path,
                "chapter and verse keys must be positive integers",
                Some(&Value::String(value.to_string())),
            )
        })
}
fn additional_fields(object: &Map<String, Value>, structural: &[&str]) -> Map<String, Value> {
    object
        .iter()
        .filter(|(key, _)| !structural.contains(&key.as_str()))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}
fn data_error(
    code: BibleDataFormatErrorCode,
    path: &str,
    message: &str,
    value: Option<&Value>,
) -> BibleDataFormatError {
    let error = BibleDataFormatError::new(code, path, message);
    value.map_or(error.clone(), |value| error.with_value(value.clone()))
}
fn json_path(base: &str, key: &str) -> String {
    if key.chars().enumerate().all(|(index, character)| {
        character == '_'
            || character.is_ascii_alphanumeric() && (index > 0 || !character.is_ascii_digit())
    }) {
        format!("{base}.{key}")
    } else {
        format!("{base}[{}]", serde_json::to_string(key).unwrap())
    }
}
fn reference_book_token(reference: &str) -> &str {
    let before = reference
        .rfind([':', '.'])
        .map_or(reference, |index| &reference[..index]);
    let book = before
        .trim_end_matches(|character: char| character.is_whitespace() || character.is_numeric());
    if book.is_empty() {
        reference.trim()
    } else {
        book.trim()
    }
}

const ROOT_FIELDS: &[&str] = &[
    "schemaVersion",
    "bookOrder",
    "books",
    "metadata",
    "source",
    "id",
    "editionId",
    "edition_id",
    "description",
    "summary",
    "language",
    "languageName",
    "language_name",
    "languageCode",
    "language_code",
    "lang",
    "translationName",
    "translation_name",
    "name",
    "title",
    "version",
    "abbreviation",
    "abbr",
    "shortName",
    "short_name",
    "year",
    "direction",
    "textDirection",
    "text_direction",
    "sourceName",
    "source_name",
    "copyright",
    "license",
    "canon",
    "versionDate",
    "version_date",
    "date",
];
