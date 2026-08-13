//! Stable chapter/verse locations and edition-aware persisted-state keys.

use std::{fmt, str::FromStr};

use bible_io_references::{
    Book as BibleBook, ChapterPassage, Passage, Reference, VersePassage, VerseRef,
};
use serde::{Deserialize, Serialize};

use crate::{errors::ModelError, verse::Verse};

/// Stable location for a chapter or verse inside one Bible edition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BibleLocation {
    book: BibleBook,
    chapter: usize,
    verse: Option<usize>,
}

impl BibleLocation {
    /// Construct a validated chapter or verse location.
    pub fn new(book: BibleBook, chapter: usize, verse: Option<usize>) -> Result<Self, ModelError> {
        if chapter == 0 {
            return Err(ModelError::new("chapter", "must be positive"));
        }
        if verse == Some(0) {
            return Err(ModelError::new("verse", "must be positive"));
        }
        Ok(Self {
            book,
            chapter,
            verse,
        })
    }

    /// Construct a location from a loaded verse.
    #[must_use]
    pub fn from_verse(verse: &Verse) -> Self {
        Self {
            book: verse.book(),
            chapter: verse.chapter(),
            verse: Some(verse.number()),
        }
    }

    /// Construct a location from a reference-package verse coordinate.
    #[must_use]
    pub fn from_verse_ref(reference: VerseRef) -> Self {
        Self {
            book: reference.book(),
            chapter: usize::from(reference.chapter()),
            verse: Some(usize::from(reference.verse())),
        }
    }

    /// Return the book identifier.
    #[must_use]
    pub const fn book(self) -> BibleBook {
        self.book
    }

    /// Return the declared chapter number.
    #[must_use]
    pub const fn chapter(self) -> usize {
        self.chapter
    }

    /// Return the declared verse number, or `None` for a chapter location.
    #[must_use]
    pub const fn verse(self) -> Option<usize> {
        self.verse
    }

    /// Return whether this location identifies a verse.
    #[must_use]
    pub const fn has_verse(self) -> bool {
        self.verse.is_some()
    }

    /// Return a validated copy with a different book.
    pub fn with_book(self, book: BibleBook) -> Result<Self, ModelError> {
        Self::new(book, self.chapter, self.verse)
    }

    /// Return a validated copy with a different chapter.
    pub fn with_chapter(self, chapter: usize) -> Result<Self, ModelError> {
        Self::new(self.book, chapter, self.verse)
    }

    /// Return a validated copy with a replacement optional verse number.
    /// Passing `None` explicitly converts the value to a chapter location.
    pub fn with_verse(self, verse: Option<usize>) -> Result<Self, ModelError> {
        Self::new(self.book, self.chapter, verse)
    }

    /// Return a canonical human-readable reference.
    #[must_use]
    pub fn reference(self) -> String {
        self.to_string()
    }

    /// Convert this location to a rich reference-package passage.
    pub fn to_passage(self) -> Result<Passage, ModelError> {
        if self.verse.is_some() {
            return VersePassage::new([Reference::Verse(self.to_verse_ref()?)])
                .map(Passage::from)
                .map_err(|error| ModelError::new("verse", error.to_string()));
        }
        let chapter = u16::try_from(self.chapter)
            .map_err(|_| ModelError::new("chapter", "exceeds the reference limit"))?;
        ChapterPassage::single(self.book, chapter)
            .map(Passage::from)
            .map_err(|error| ModelError::new("chapter", error.to_string()))
    }

    /// Convert a verse location to a reference-package coordinate.
    pub fn to_verse_ref(self) -> Result<VerseRef, ModelError> {
        let verse = self
            .verse
            .ok_or_else(|| ModelError::new("verse", "a verse number is required"))?;
        let chapter = u16::try_from(self.chapter)
            .map_err(|_| ModelError::new("chapter", "exceeds the reference limit"))?;
        let verse = u16::try_from(verse)
            .map_err(|_| ModelError::new("verse", "exceeds the reference limit"))?;
        VerseRef::new(self.book, chapter, verse)
            .map_err(|error| ModelError::new("location", error.to_string()))
    }
}

#[derive(Serialize, Deserialize)]
struct BibleLocationJson {
    book: String,
    chapter: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    verse: Option<usize>,
}

impl Serialize for BibleLocation {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        BibleLocationJson {
            book: self.book.abbreviation().to_string(),
            chapter: self.chapter,
            verse: self.verse,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for BibleLocation {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = BibleLocationJson::deserialize(deserializer)?;
        let book = parse_book_identifier(&value.book)
            .ok_or_else(|| serde::de::Error::custom("unknown Bible book"))?;
        Self::new(book, value.chapter, value.verse).map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for BibleLocation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} {}", self.book.full_name(), self.chapter)?;
        if let Some(verse) = self.verse {
            write!(formatter, ":{verse}")?;
        }
        Ok(())
    }
}

/// An edition-aware stable key for bookmarks, notes, and reading progress.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BibleVerseKey {
    edition_id: String,
    location: BibleLocation,
}

impl BibleVerseKey {
    /// Construct a validated persisted-state key.
    pub fn new(edition_id: impl Into<String>, location: BibleLocation) -> Result<Self, ModelError> {
        let edition_id = edition_id.into();
        if edition_id.trim().is_empty() || edition_id.trim() != edition_id {
            return Err(ModelError::new(
                "edition_id",
                "must be non-blank and have no surrounding whitespace",
            ));
        }
        if !location.has_verse() {
            return Err(ModelError::new("location", "must identify a verse"));
        }
        Ok(Self {
            edition_id,
            location,
        })
    }

    /// Construct a key from a loaded verse.
    pub fn from_verse(edition_id: impl Into<String>, verse: &Verse) -> Result<Self, ModelError> {
        Self::new(edition_id, BibleLocation::from_verse(verse))
    }

    /// Return the edition identifier.
    #[must_use]
    pub fn edition_id(&self) -> &str {
        &self.edition_id
    }

    /// Return the verse location.
    #[must_use]
    pub const fn location(&self) -> BibleLocation {
        self.location
    }

    /// Convert the stored location to a reference-package coordinate.
    pub fn to_verse_ref(&self) -> Result<VerseRef, ModelError> {
        self.location.to_verse_ref()
    }

    /// Return a validated copy with a different edition identifier.
    pub fn with_edition_id(&self, edition_id: impl Into<String>) -> Result<Self, ModelError> {
        Self::new(edition_id, self.location)
    }

    /// Return a validated copy with a different verse location.
    pub fn with_location(&self, location: BibleLocation) -> Result<Self, ModelError> {
        Self::new(self.edition_id.clone(), location)
    }
}

impl fmt::Display for BibleVerseKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:{}", self.edition_id, self.location)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BibleVerseKeyJson {
    edition_id: String,
    location: BibleLocation,
}

impl<'de> Deserialize<'de> for BibleVerseKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = BibleVerseKeyJson::deserialize(deserializer)?;
        Self::new(value.edition_id, value.location).map_err(serde::de::Error::custom)
    }
}

pub(crate) fn parse_book_identifier(value: &str) -> Option<BibleBook> {
    let trimmed = value.trim();
    BibleBook::from_abbreviation(trimmed)
        .or_else(|| BibleBook::from_osis(trimmed))
        .or_else(|| BibleBook::from_usfm(trimmed))
        .or_else(|| BibleBook::from_str(trimmed).ok())
        .or_else(|| {
            BibleBook::ALL
                .iter()
                .copied()
                .find(|book| book.full_name().eq_ignore_ascii_case(trimmed))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn location_and_key_round_trip() {
        let location = BibleLocation::new(BibleBook::John, 3, Some(16)).unwrap();
        let key = BibleVerseKey::new("eng-test", location).unwrap();
        let json = serde_json::to_string(&key).unwrap();
        assert_eq!(serde_json::from_str::<BibleVerseKey>(&json).unwrap(), key);
        assert_eq!(key.to_verse_ref().unwrap().book(), BibleBook::John);
    }
}
