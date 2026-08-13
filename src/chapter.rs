//! A numbered collection of verses.

use std::{
    fmt,
    hash::{Hash, Hasher},
};

use serde_json::Value;

use crate::{
    bible_books_enum::BibleBook,
    errors::ModelError,
    json_value::{hash_json_map, validate_annotations, JsonMap},
    verse::Verse,
};

/// Represents a chapter from a Bible book.
#[derive(Debug, Clone, PartialEq)]
pub struct Chapter {
    book: BibleBook,
    verses: Vec<Verse>,
    chapter_number: usize,
    annotations: JsonMap,
}

/// Aggregate statistics for one chapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChapterStats {
    /// Number of verses.
    pub verse_count: usize,
    /// Number of Unicode word tokens.
    pub total_words: usize,
    /// Mean verse length in Unicode scalar values, rounded to the nearest integer.
    pub average_verse_length: usize,
}

impl Chapter {
    /// Creates a chapter using the first verse's book for compatibility.
    ///
    /// For an empty chapter, Genesis is used as the compatibility book. New
    /// code should prefer [`Chapter::checked`].
    pub fn new(verses: Vec<Verse>, chapter_number: usize) -> Self {
        let book = verses.first().map_or(BibleBook::Genesis, Verse::book);
        Self::checked(book, chapter_number, verses, JsonMap::new())
            .expect("Chapter::new requires positive, unique, matching verse locations")
    }

    /// Creates a sorted chapter and validates every verse relationship.
    pub fn checked(
        book: BibleBook,
        chapter_number: usize,
        mut verses: Vec<Verse>,
        annotations: JsonMap,
    ) -> Result<Self, ModelError> {
        if chapter_number == 0 {
            return Err(ModelError::new("chapter_number", "must be positive"));
        }
        validate_annotations(&annotations, &["verses"])?;
        verses.sort_by_key(Verse::number);
        let mut previous = None;
        for verse in &verses {
            if verse.number() == 0 {
                return Err(ModelError::new("verses", "verse numbers must be positive"));
            }
            if verse.book() != book {
                return Err(ModelError::new(
                    "verses",
                    format!("verse {} belongs to another book", verse.number()),
                ));
            }
            if verse.chapter() != chapter_number {
                return Err(ModelError::new(
                    "verses",
                    format!("verse {} belongs to another chapter", verse.number()),
                ));
            }
            if previous == Some(verse.number()) {
                return Err(ModelError::new(
                    "verses",
                    format!("duplicate verse number {}", verse.number()),
                ));
            }
            previous = Some(verse.number());
        }
        Ok(Self {
            book,
            verses,
            chapter_number,
            annotations,
        })
    }

    /// Returns the book containing this chapter.
    #[must_use]
    pub const fn book(&self) -> BibleBook {
        self.book
    }

    /// Returns this chapter's declared number within its book.
    #[must_use]
    pub const fn number(&self) -> usize {
        self.chapter_number
    }

    /// Returns all verses in declared numeric order.
    #[must_use]
    pub fn get_verses(&self) -> &[Verse] {
        &self.verses
    }

    /// Return all verses in declared numeric order.
    #[must_use]
    pub fn verses(&self) -> &[Verse] {
        &self.verses
    }

    /// Returns a verse by its declared number, including in sparse chapters.
    #[must_use]
    pub fn get_verse(&self, verse_number: usize) -> Option<&Verse> {
        self.verses
            .binary_search_by_key(&verse_number, Verse::number)
            .ok()
            .map(|index| &self.verses[index])
    }

    /// Returns the immutable chapter annotations.
    #[must_use]
    pub fn annotations(&self) -> &JsonMap {
        &self.annotations
    }

    /// Returns verses containing one complete normalized Unicode word.
    #[must_use]
    pub fn search(&self, word: &str) -> Vec<Verse> {
        self.verses
            .iter()
            .filter(|verse| verse.contains_word(word))
            .cloned()
            .collect()
    }

    /// Return whether any verse contains one complete normalized word.
    #[must_use]
    pub fn contains_word(&self, word: &str) -> bool {
        self.verses.iter().any(|verse| verse.contains_word(word))
    }

    /// Return borrowed verses containing one complete normalized word.
    #[must_use]
    pub fn verses_containing(&self, word: &str) -> Vec<&Verse> {
        self.verses
            .iter()
            .filter(|verse| verse.contains_word(word))
            .collect()
    }

    /// Return the canonical chapter reference.
    #[must_use]
    pub fn reference(&self) -> String {
        format!("{} {}", self.book.full_name(), self.chapter_number)
    }

    /// Derive aggregate text statistics.
    #[must_use]
    pub fn stats(&self) -> ChapterStats {
        let character_count = self.verses.iter().map(Verse::len).sum::<usize>();
        ChapterStats {
            verse_count: self.verses.len(),
            total_words: self.verses.iter().map(|verse| verse.words().len()).sum(),
            average_verse_length: if self.verses.is_empty() {
                0
            } else {
                (character_count as f64 / self.verses.len() as f64).round() as usize
            },
        }
    }

    /// Return a validated copy with replacement values.
    pub fn copy_with(
        &self,
        book: Option<BibleBook>,
        chapter_number: Option<usize>,
        verses: Option<Vec<Verse>>,
        annotations: Option<JsonMap>,
    ) -> Result<Self, ModelError> {
        Self::checked(
            book.unwrap_or(self.book),
            chapter_number.unwrap_or(self.chapter_number),
            verses.unwrap_or_else(|| self.verses.clone()),
            annotations.unwrap_or_else(|| self.annotations.clone()),
        )
    }

    /// Return a validated copy with replacement annotations.
    pub fn with_annotations(&self, annotations: JsonMap) -> Result<Self, ModelError> {
        self.copy_with(None, None, None, Some(annotations))
    }

    /// Encodes this chapter in the schema's compatible JSON shape.
    #[must_use]
    pub fn to_json_value(&self) -> Value {
        let verses = self
            .verses
            .iter()
            .map(|verse| (verse.number().to_string(), verse.to_json_value()))
            .collect();
        if self.annotations.is_empty() {
            return Value::Object(verses);
        }
        let mut object = self.annotations.clone();
        object.insert("verses".to_string(), Value::Object(verses));
        Value::Object(object)
    }
}

impl Eq for Chapter {}

impl Hash for Chapter {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.book.hash(state);
        self.chapter_number.hash(state);
        self.verses.hash(state);
        hash_json_map(&self.annotations, state);
    }
}

impl fmt::Display for Chapter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let verses = self
            .verses
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        write!(formatter, "Chapter {}:\n{verses}", self.chapter_number)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sparse_lookup_uses_declared_numbers() {
        let verses = vec![
            Verse::checked(BibleBook::Genesis, 3, 9, "Nine", JsonMap::new()).unwrap(),
            Verse::checked(BibleBook::Genesis, 3, 2, "Two", JsonMap::new()).unwrap(),
        ];
        let chapter = Chapter::checked(BibleBook::Genesis, 3, verses, JsonMap::new()).unwrap();
        assert_eq!(chapter.get_verse(2).unwrap().text(), "Two");
        assert_eq!(chapter.get_verse(9).unwrap().text(), "Nine");
        assert!(chapter.get_verse(1).is_none());
    }
}
