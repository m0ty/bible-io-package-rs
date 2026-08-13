//! Error values returned by Bible IO operations.

use std::{error::Error, fmt, sync::Arc};

use bible_io_references::ParseError;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Stable machine-readable codes for malformed Bible and catalog data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BibleDataFormatErrorCode {
    /// Input was not valid JSON or UTF-8 JSON.
    InvalidJson,
    /// A JSON value had the wrong type.
    InvalidType,
    /// A required field was absent.
    MissingField,
    /// A value violated the content contract.
    InvalidValue,
    /// An identifier appeared more than once.
    DuplicateId,
    /// Extension data used a structural field name.
    ReservedField,
    /// A value could not be represented as JSON.
    NonJsonValue,
}

impl BibleDataFormatErrorCode {
    /// Return the stable snake-case representation of this error code.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidJson => "invalid_json",
            Self::InvalidType => "invalid_type",
            Self::MissingField => "missing_field",
            Self::InvalidValue => "invalid_value",
            Self::DuplicateId => "duplicate_id",
            Self::ReservedField => "reserved_field",
            Self::NonJsonValue => "non_json_value",
        }
    }
}

impl fmt::Display for BibleDataFormatErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// A path-aware violation of the serialized Bible content contract.
#[derive(Debug, Clone, PartialEq)]
pub struct BibleDataFormatError {
    code: BibleDataFormatErrorCode,
    path: String,
    message: String,
    value: Option<Box<Value>>,
    cause: Option<StoredCause>,
}

#[derive(Clone)]
struct StoredCause {
    message: String,
    error: Arc<dyn Error + Send + Sync>,
}

impl fmt::Debug for StoredCause {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("StoredCause")
            .field(&self.message)
            .finish()
    }
}

impl PartialEq for StoredCause {
    fn eq(&self, other: &Self) -> bool {
        self.message == other.message
    }
}

impl fmt::Display for StoredCause {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl BibleDataFormatError {
    /// Construct an error without an offending value.
    pub fn new(
        code: BibleDataFormatErrorCode,
        path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code,
            path: path.into(),
            message: message.into(),
            value: None,
            cause: None,
        }
    }

    /// Attach the offending JSON value.
    #[must_use]
    pub fn with_value(mut self, value: Value) -> Self {
        self.value = Some(Box::new(value));
        self
    }

    /// Attach the underlying error while preserving its concrete type.
    #[must_use]
    pub fn with_cause<E>(mut self, cause: E) -> Self
    where
        E: Error + Send + Sync + 'static,
    {
        self.cause = Some(StoredCause {
            message: cause.to_string(),
            error: Arc::new(cause),
        });
        self
    }

    /// Return the stable machine-readable code.
    #[must_use]
    pub const fn code(&self) -> BibleDataFormatErrorCode {
        self.code
    }

    /// Return the JSONPath-like location of the invalid value.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Return the human-readable explanation.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Return the offending JSON value, when one was captured.
    #[must_use]
    pub fn value(&self) -> Option<&Value> {
        self.value.as_deref()
    }

    /// Return the underlying error text, when one was captured.
    #[must_use]
    pub fn cause(&self) -> Option<&str> {
        self.cause.as_ref().map(|cause| cause.message.as_str())
    }
}

impl fmt::Display for BibleDataFormatError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "BibleDataFormatError({}) at {}: {}",
            self.code, self.path, self.message
        )?;
        if let Some(value) = &self.value {
            write!(formatter, "\nValue: {value}")?;
        }
        if let Some(cause) = &self.cause {
            write!(formatter, "\nCause: {cause}")?;
        }
        Ok(())
    }
}

impl Error for BibleDataFormatError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.cause
            .as_ref()
            .map(|cause| cause.error.as_ref() as &(dyn Error + 'static))
    }
}

/// A validated in-memory model could not be constructed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelError {
    field: &'static str,
    message: String,
}

impl ModelError {
    /// Construct a model validation error.
    pub fn new(field: &'static str, message: impl Into<String>) -> Self {
        Self {
            field,
            message: message.into(),
        }
    }

    /// Return the invalid field name.
    #[must_use]
    pub const fn field(&self) -> &'static str {
        self.field
    }

    /// Return the validation explanation.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for ModelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.field, self.message)
    }
}

impl Error for ModelError {}

/// Errors that can occur when accessing Bible content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BibleError {
    /// The requested book is not present in the specified Bible translation.
    BookNotFound {
        /// Requested compact book identifier.
        book_abbrev: String,
        /// Human-readable book name.
        book_name: String,
        /// Human-readable edition name.
        translation: String,
    },
    /// The requested chapter number does not exist in the specified book.
    ChapterOutOfBounds {
        /// Compact book identifier.
        book_abbrev: String,
        /// Human-readable book name.
        book_name: String,
        /// Requested chapter number.
        chapter: usize,
        /// Greatest declared chapter number, or zero for an empty book.
        max_chapter: usize,
    },
    /// The requested verse number does not exist in the specified chapter.
    VerseOutOfBounds {
        /// Compact book identifier.
        book_abbrev: String,
        /// Human-readable book name.
        book_name: String,
        /// Requested chapter number.
        chapter: usize,
        /// Requested verse number.
        verse: usize,
        /// Greatest declared verse number, or zero for an empty chapter.
        max_verse: usize,
    },
    /// The provided reference string could not be parsed.
    InvalidReference {
        /// Original reference input after surrounding whitespace was removed.
        input: String,
    },
    /// The reference package rejected a human-readable reference.
    ReferenceParse {
        /// Original input after surrounding whitespace was removed.
        input: String,
        /// Structured dependency parse failure.
        cause: ParseError,
    },
    /// A range is descending in the loaded edition's declared order.
    InvalidRange {
        /// Human-readable explanation.
        message: String,
    },
    /// A persisted-state key was requested for an edition without an ID.
    MissingEditionId,
    /// A chapter-only location was used where a verse location was required.
    VerseRequired,
}

impl fmt::Display for BibleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BookNotFound {
                book_abbrev,
                book_name,
                translation,
            } => write!(
                formatter,
                "Book {book_name} ('{book_abbrev}') not found in the '{translation}' Bible translation"
            ),
            Self::ChapterOutOfBounds {
                book_abbrev,
                book_name,
                chapter,
                max_chapter,
            } => write!(
                formatter,
                "Chapter {chapter} is out of bounds for book {book_name} ('{book_abbrev}') (max {max_chapter})"
            ),
            Self::VerseOutOfBounds {
                book_abbrev,
                book_name,
                chapter,
                verse,
                max_verse,
            } => write!(
                formatter,
                "Verse {verse} is out of bounds for book {book_name} ('{book_abbrev}') chapter {chapter} (max {max_verse})"
            ),
            Self::InvalidReference { input } => write!(formatter, "Invalid reference: '{input}'"),
            Self::ReferenceParse { input, cause } => {
                write!(formatter, "Invalid reference '{input}': {cause}")
            }
            Self::InvalidRange { message } => formatter.write_str(message),
            Self::MissingEditionId => formatter.write_str(
                "Bible metadata must define an id before creating persisted keys",
            ),
            Self::VerseRequired => {
                formatter.write_str("the Bible location must identify a verse")
            }
        }
    }
}

impl Error for BibleError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ReferenceParse { cause, .. } => Some(cause),
            _ => None,
        }
    }
}
