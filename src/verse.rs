//! A validated Bible verse with Unicode text-search helpers and annotations.

use std::{
    fmt,
    hash::{Hash, Hasher},
};

use bible_io_references::VerseRef;
use serde_json::Value;

use crate::bible_books_enum::BibleBook;
use crate::{
    errors::ModelError,
    json_value::{hash_json_map, validate_annotations, JsonMap},
    location::BibleLocation,
    text_search::{contains_normalized_text, tokenize_search_text},
};

/// Represents a single verse from the Bible.
///
/// A verse contains the text content and its reference information within a chapter.
#[derive(Debug, Clone, PartialEq)]
pub struct Verse {
    book: BibleBook,
    chapter_number: usize,
    verse_text: String,
    verse_number: usize,
    annotations: JsonMap,
}

/// Aggregate statistics for one verse.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VerseStats {
    /// Number of Unicode word tokens.
    pub word_count: usize,
    /// Number of Unicode scalar values in the verse text.
    pub character_count: usize,
    /// Mean word length in Unicode scalar values.
    pub average_word_length: f64,
}

impl Verse {
    /// Creates a new verse with the given text and verse number.
    ///
    /// # Arguments
    ///
    /// * `book` - The book this verse belongs to
    /// * `chapter_number` - The chapter number within the book
    /// * `verse_number` - The verse number within its chapter
    /// * `verse_text` - The text content of the verse
    pub fn new(
        book: BibleBook,
        chapter_number: usize,
        verse_number: usize,
        verse_text: String,
    ) -> Self {
        assert!(chapter_number > 0, "chapter_number must be positive");
        assert!(verse_number > 0, "verse_number must be positive");
        Verse {
            book,
            chapter_number,
            verse_text: sanitize_verse_text(verse_text),
            verse_number,
            annotations: JsonMap::new(),
        }
    }

    /// Creates a verse with runtime coordinate and annotation validation.
    ///
    /// Unlike the historical [`Verse::new`] constructor, this lossless
    /// constructor leaves the supplied text unchanged.
    pub fn checked(
        book: BibleBook,
        chapter_number: usize,
        verse_number: usize,
        verse_text: impl Into<String>,
        annotations: JsonMap,
    ) -> Result<Self, ModelError> {
        if chapter_number == 0 {
            return Err(ModelError::new("chapter_number", "must be positive"));
        }
        if verse_number == 0 {
            return Err(ModelError::new("verse_number", "must be positive"));
        }
        validate_annotations(&annotations, &["text"])?;
        Ok(Self {
            book,
            chapter_number,
            verse_number,
            verse_text: verse_text.into(),
            annotations,
        })
    }

    /// Returns the book this verse belongs to.
    pub fn book(&self) -> BibleBook {
        self.book
    }

    /// Returns the chapter number within the book.
    pub fn chapter(&self) -> usize {
        self.chapter_number
    }

    /// Returns the text content of the verse.
    pub fn text(&self) -> &str {
        &self.verse_text
    }

    /// Returns the verse number within its chapter.
    pub fn number(&self) -> usize {
        self.verse_number
    }

    /// Returns the stable location of this verse within an edition.
    #[must_use]
    pub fn location(&self) -> BibleLocation {
        BibleLocation::new(self.book, self.chapter_number, Some(self.verse_number))
            .expect("Verse coordinates are validated when constructed")
    }

    /// Converts this verse location into a reference-package value.
    pub fn to_verse_ref(&self) -> Result<VerseRef, ModelError> {
        let chapter = u16::try_from(self.chapter_number)
            .map_err(|_| ModelError::new("chapter_number", "exceeds the reference limit"))?;
        let verse = u16::try_from(self.verse_number)
            .map_err(|_| ModelError::new("verse_number", "exceeds the reference limit"))?;
        VerseRef::new(self.book, chapter, verse)
            .map_err(|error| ModelError::new("location", error.to_string()))
    }

    /// Returns the immutable annotation object associated with this verse.
    #[must_use]
    pub fn annotations(&self) -> &JsonMap {
        &self.annotations
    }

    /// Returns whether `word` is exactly one normalized Unicode token found in
    /// this verse.
    #[must_use]
    pub fn contains_word(&self, word: &str) -> bool {
        let query = tokenize_search_text(word, false, true, false);
        query.len() == 1
            && tokenize_search_text(&self.verse_text, false, true, false).contains(&query[0])
    }

    /// Returns whether this verse contains a normalized substring.
    #[must_use]
    pub fn contains_text(&self, query: &str) -> bool {
        contains_normalized_text(&self.verse_text, query, false, true, false)
    }

    /// Return whether at least one supplied word occurs as a whole token.
    #[must_use]
    pub fn contains_any<'a>(&self, words: impl IntoIterator<Item = &'a str>) -> bool {
        words.into_iter().any(|word| self.contains_word(word))
    }

    /// Return whether every supplied word occurs as a whole token.
    #[must_use]
    pub fn contains_all<'a>(&self, words: impl IntoIterator<Item = &'a str>) -> bool {
        words.into_iter().all(|word| self.contains_word(word))
    }

    /// Return the original Unicode word tokens in source order.
    #[must_use]
    pub fn words(&self) -> Vec<String> {
        crate::text_search::extract_unicode_words(&self.verse_text)
            .into_iter()
            .map(str::to_owned)
            .collect()
    }

    /// Return the verse length in Unicode scalar values.
    #[must_use]
    pub fn len(&self) -> usize {
        self.verse_text.chars().count()
    }

    /// Return whether the verse text is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.verse_text.is_empty()
    }

    /// Return the canonical full reference string.
    #[must_use]
    pub fn reference(&self) -> String {
        format!(
            "{} {}:{}",
            self.book.full_name(),
            self.chapter_number,
            self.verse_number
        )
    }

    /// Return the canonical compact reference string.
    #[must_use]
    pub fn short_reference(&self) -> String {
        format!(
            "{}{}:{}",
            self.book.abbreviation(),
            self.chapter_number,
            self.verse_number
        )
    }

    /// Derive aggregate text statistics.
    #[must_use]
    pub fn stats(&self) -> VerseStats {
        let words = self.words();
        let word_characters = words.iter().map(|word| word.chars().count()).sum::<usize>();
        VerseStats {
            word_count: words.len(),
            character_count: self.len(),
            average_word_length: if words.is_empty() {
                0.0
            } else {
                word_characters as f64 / words.len() as f64
            },
        }
    }

    /// Return a validated copy with replacement values.
    pub fn copy_with(
        &self,
        book: Option<BibleBook>,
        chapter_number: Option<usize>,
        verse_number: Option<usize>,
        verse_text: Option<String>,
        annotations: Option<JsonMap>,
    ) -> Result<Self, ModelError> {
        Self::checked(
            book.unwrap_or(self.book),
            chapter_number.unwrap_or(self.chapter_number),
            verse_number.unwrap_or(self.verse_number),
            verse_text.unwrap_or_else(|| self.verse_text.clone()),
            annotations.unwrap_or_else(|| self.annotations.clone()),
        )
    }

    /// Return a validated copy with different text.
    pub fn with_text(&self, verse_text: impl Into<String>) -> Self {
        Self {
            verse_text: verse_text.into(),
            ..self.clone()
        }
    }

    /// Return a validated copy with replacement annotations.
    pub fn with_annotations(&self, annotations: JsonMap) -> Result<Self, ModelError> {
        self.copy_with(None, None, None, None, Some(annotations))
    }

    /// Returns a JSON value compatible with both plain and annotated verses.
    #[must_use]
    pub fn to_json_value(&self) -> Value {
        if self.annotations.is_empty() {
            return Value::String(self.verse_text.clone());
        }
        let mut value = JsonMap::new();
        value.insert("text".to_string(), Value::String(self.verse_text.clone()));
        value.extend(self.annotations.clone());
        Value::Object(value)
    }
}

impl Eq for Verse {}

impl Hash for Verse {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.book.hash(state);
        self.chapter_number.hash(state);
        self.verse_number.hash(state);
        self.verse_text.hash(state);
        hash_json_map(&self.annotations, state);
    }
}

fn sanitize_verse_text(verse_text: String) -> String {
    verse_text
}

impl fmt::Display for Verse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.verse_number, self.verse_text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_and_accessors() {
        let verse = Verse::new(BibleBook::Genesis, 1, 1, "Test".to_string());
        assert_eq!(verse.book(), BibleBook::Genesis);
        assert_eq!(verse.chapter(), 1);
        assert_eq!(verse.text(), "Test");
        assert_eq!(verse.number(), 1);
        assert_eq!(format!("{}", verse), "1: Test");
    }

    #[test]
    fn test_sanitize_verse_text() {
        let verse = Verse::new(BibleBook::Genesis, 1, 1, "In {the} beginning".to_string());
        assert_eq!(verse.text(), "In {the} beginning");
    }

    #[test]
    fn test_clone_independence() {
        let original = Verse::new(BibleBook::Genesis, 1, 42, "Clone me".to_string());
        let cloned = original.clone();

        assert_eq!(original.book(), cloned.book());
        assert_eq!(original.chapter(), cloned.chapter());
        assert_eq!(original.text(), cloned.text());
        assert_eq!(original.number(), cloned.number());

        // Ensure the cloned verse has its own allocation
        assert_ne!(original.text().as_ptr(), cloned.text().as_ptr());
    }
}
