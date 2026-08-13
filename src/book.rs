//! A named collection of numbered chapters.

use std::{
    fmt,
    hash::{Hash, Hasher},
};

use serde_json::{Map, Value};

use crate::{
    bible::BibleError,
    bible_books_enum::BibleBook,
    chapter::Chapter,
    errors::ModelError,
    json_value::{hash_json_map, validate_annotations, JsonMap},
    verse::Verse,
};

/// Represents a book of the Bible.
#[derive(Debug, Clone, PartialEq)]
pub struct Book {
    book: BibleBook,
    abbrev: String,
    title: String,
    chapters: Vec<Chapter>,
    annotations: JsonMap,
}

/// Aggregate statistics for one book.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BookStats {
    /// Number of chapters.
    pub chapter_count: usize,
    /// Number of verses.
    pub verse_count: usize,
    /// Number of Unicode word tokens.
    pub total_words: usize,
    /// Mean number of verses per chapter.
    pub average_verses_per_chapter: f64,
}

impl Book {
    /// Creates a book using a compact package abbreviation.
    ///
    /// This compatibility constructor panics on an unknown abbreviation or an
    /// invalid chapter graph. New code should prefer [`Book::checked`].
    pub fn new(abbrev: String, title: String, chapters: Vec<Chapter>) -> Self {
        Self::try_new(abbrev, title, chapters)
            .expect("Book::new requires a supported identifier and valid, matching chapters")
    }

    /// Fallible compatibility constructor accepting any supported identifier.
    pub fn try_new(
        identifier: impl AsRef<str>,
        title: impl Into<String>,
        chapters: Vec<Chapter>,
    ) -> Result<Self, ModelError> {
        let identifier = identifier.as_ref();
        let book = crate::location::parse_book_identifier(identifier)
            .ok_or_else(|| ModelError::new("identifier", "is not a supported Bible book"))?;
        Self::checked(book, title, chapters, JsonMap::new())
    }

    /// Creates a sorted book and validates every chapter relationship.
    pub fn checked(
        book: BibleBook,
        title: impl Into<String>,
        mut chapters: Vec<Chapter>,
        annotations: JsonMap,
    ) -> Result<Self, ModelError> {
        let title = title.into();
        if title.trim().is_empty() {
            return Err(ModelError::new("title", "must not be blank"));
        }
        validate_annotations(&annotations, &["name", "chapters"])?;
        chapters.sort_by_key(Chapter::number);
        let mut previous = None;
        for chapter in &chapters {
            if chapter.book() != book {
                return Err(ModelError::new(
                    "chapters",
                    format!("chapter {} belongs to another book", chapter.number()),
                ));
            }
            if previous == Some(chapter.number()) {
                return Err(ModelError::new(
                    "chapters",
                    format!("duplicate chapter number {}", chapter.number()),
                ));
            }
            previous = Some(chapter.number());
        }
        Ok(Self {
            book,
            abbrev: book.abbreviation().to_string(),
            title,
            chapters,
            annotations,
        })
    }

    /// Returns this book's canonical identifier.
    #[must_use]
    pub const fn book(&self) -> BibleBook {
        self.book
    }

    /// Returns the compact package abbreviation.
    #[must_use]
    pub fn abbrev(&self) -> &str {
        &self.abbrev
    }

    /// Returns the loaded display title.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Returns all chapters in declared numeric order.
    #[must_use]
    pub fn chapters(&self) -> &[Chapter] {
        &self.chapters
    }

    /// Iterate all verses in declared chapter and verse order.
    pub fn all_verses(&self) -> impl Iterator<Item = &Verse> {
        self.chapters.iter().flat_map(Chapter::get_verses)
    }

    /// Return the total number of verses in this book.
    #[must_use]
    pub fn verse_count(&self) -> usize {
        self.chapters
            .iter()
            .map(|chapter| chapter.verses().len())
            .sum()
    }

    /// Returns the immutable book annotations.
    #[must_use]
    pub fn annotations(&self) -> &JsonMap {
        &self.annotations
    }

    /// Returns a chapter by its declared number, including in sparse books.
    pub fn get_chapter(&self, chapter_number: usize) -> Result<&Chapter, BibleError> {
        self.chapters
            .binary_search_by_key(&chapter_number, Chapter::number)
            .ok()
            .map(|index| &self.chapters[index])
            .ok_or_else(|| BibleError::ChapterOutOfBounds {
                book_abbrev: self.abbrev.clone(),
                book_name: self.title.clone(),
                chapter: chapter_number,
                max_chapter: self.chapters.last().map_or(0, Chapter::number),
            })
    }

    /// Returns all verses in a declared chapter.
    pub fn get_verses(&self, chapter_number: usize) -> Result<&[Verse], BibleError> {
        self.get_chapter(chapter_number).map(Chapter::get_verses)
    }

    /// Returns a verse by its declared chapter and verse numbers.
    pub fn get_verse(
        &self,
        chapter_number: usize,
        verse_number: usize,
    ) -> Result<&Verse, BibleError> {
        let chapter = self.get_chapter(chapter_number)?;
        chapter
            .get_verse(verse_number)
            .ok_or_else(|| BibleError::VerseOutOfBounds {
                book_abbrev: self.abbrev.clone(),
                book_name: self.title.clone(),
                chapter: chapter_number,
                verse: verse_number,
                max_verse: chapter.get_verses().last().map_or(0, Verse::number),
            })
    }

    /// Returns all verses containing one complete normalized Unicode word.
    #[must_use]
    pub fn search(&self, word: &str) -> Vec<Verse> {
        self.chapters
            .iter()
            .flat_map(|chapter| chapter.search(word))
            .collect()
    }

    /// Return chapters containing one complete normalized word.
    #[must_use]
    pub fn chapters_containing(&self, word: &str) -> Vec<&Chapter> {
        self.chapters
            .iter()
            .filter(|chapter| chapter.contains_word(word))
            .collect()
    }

    /// Derive aggregate text statistics.
    #[must_use]
    pub fn stats(&self) -> BookStats {
        let verse_count = self.verse_count();
        BookStats {
            chapter_count: self.chapters.len(),
            verse_count,
            total_words: self.all_verses().map(|verse| verse.words().len()).sum(),
            average_verses_per_chapter: if self.chapters.is_empty() {
                0.0
            } else {
                verse_count as f64 / self.chapters.len() as f64
            },
        }
    }

    /// Return a validated copy with replacement values.
    pub fn copy_with(
        &self,
        book: Option<BibleBook>,
        title: Option<String>,
        chapters: Option<Vec<Chapter>>,
        annotations: Option<JsonMap>,
    ) -> Result<Self, ModelError> {
        Self::checked(
            book.unwrap_or(self.book),
            title.unwrap_or_else(|| self.title.clone()),
            chapters.unwrap_or_else(|| self.chapters.clone()),
            annotations.unwrap_or_else(|| self.annotations.clone()),
        )
    }

    /// Return a validated copy with a different display title.
    pub fn with_title(&self, title: impl Into<String>) -> Result<Self, ModelError> {
        self.copy_with(None, Some(title.into()), None, None)
    }

    /// Return a validated copy with replacement annotations.
    pub fn with_annotations(&self, annotations: JsonMap) -> Result<Self, ModelError> {
        self.copy_with(None, None, None, Some(annotations))
    }

    /// Encodes this book and its annotations into the versioned JSON shape.
    #[must_use]
    pub fn to_json_value(&self) -> Value {
        let mut object = self.annotations.clone();
        object.insert("name".to_string(), Value::String(self.title.clone()));
        let chapters: Map<String, Value> = self
            .chapters
            .iter()
            .map(|chapter| (chapter.number().to_string(), chapter.to_json_value()))
            .collect();
        object.insert("chapters".to_string(), Value::Object(chapters));
        Value::Object(object)
    }
}

impl Eq for Book {}

impl Hash for Book {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.book.hash(state);
        self.title.hash(state);
        self.chapters.hash(state);
        hash_json_map(&self.annotations, state);
    }
}

impl fmt::Display for Book {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Book: {} ({})", self.title, self.abbrev)
    }
}
