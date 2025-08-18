use std::fmt;

use crate::{bible::BibleError, chapter::Chapter, verse::Verse};

/// Represents a book of the Bible.
///
/// A book contains multiple chapters and has an abbreviation and title.
#[derive(Debug)]
pub struct Book {
    abbrev: String, // keep the JSON key, no assumptions about canon
    title: String,
    chapters: Vec<Chapter>,
}

impl Book {
    /// Creates a new book with the given abbreviation, title, and chapters.
    ///
    /// # Arguments
    ///
    /// * `abbrev` - The book's abbreviation (e.g., "gn" for Genesis)
    /// * `title` - The full title of the book
    /// * `chapters` - A vector of chapters in this book
    pub fn new(abbrev: String, title: String, chapters: Vec<Chapter>) -> Self {
        Book {
            abbrev: abbrev.to_ascii_lowercase(),
            title,
            chapters,
        }
    }

    /// Returns the book's abbreviation.
    pub fn abbrev(&self) -> &str {
        &self.abbrev
    }

    /// Returns the book's full title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Returns a slice of all chapters in this book.
    pub fn chapters(&self) -> &[Chapter] {
        &self.chapters
    }

    /// Returns a specific chapter by its chapter number.
    ///
    /// # Arguments
    ///
    /// * `chapter_number` - The chapter number to retrieve
    ///
    /// # Returns
    ///
    /// The requested chapter or a descriptive error if the chapter number is invalid.
    pub fn get_chapter(&self, chapter_number: usize) -> Result<&Chapter, BibleError> {
        if chapter_number == 0 {
            return Err(BibleError::ChapterOutOfBounds {
                book_abbrev: self.abbrev.clone(),
                book_name: self.title.clone(),
                chapter: chapter_number,
                max_chapter: self.chapters.len(),
            });
        }
        self.chapters
            .get(chapter_number - 1)
            .ok_or_else(|| BibleError::ChapterOutOfBounds {
                book_abbrev: self.abbrev.clone(),
                book_name: self.title.clone(),
                chapter: chapter_number,
                max_chapter: self.chapters.len(),
            })
    }

    /// Returns all verses from a specific chapter.
    ///
    /// # Arguments
    ///
    /// * `chapter_number` - The chapter number to retrieve verses from
    ///
    /// # Returns
    ///
    /// The verses from the requested chapter or a descriptive error.
    pub fn get_verses(&self, chapter_number: usize) -> Result<&[Verse], BibleError> {
        self.get_chapter(chapter_number).map(|c| c.get_verses())
    }

    /// Returns a specific verse by chapter and verse number.
    ///
    /// # Arguments
    ///
    /// * `chapter_number` - The chapter number
    /// * `verse_number` - The verse number within the chapter
    ///
    /// # Returns
    ///
    /// The requested verse or a descriptive error if the chapter or verse is invalid.
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
                max_verse: chapter.get_verses().len(),
            })
    }
}

impl fmt::Display for Book {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Book: {} ({})", self.title, self.abbrev)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verse::Verse;

    fn create_test_chapter() -> Chapter {
        let verses = vec![Verse::new("Test".into(), 1)];
        Chapter::new(verses, 1)
    }

    #[test]
    fn test_book_methods() {
        let book = Book::new("GN".into(), "Genesis".into(), vec![create_test_chapter()]);
        assert_eq!(book.abbrev(), "gn");
        assert_eq!(book.title(), "Genesis");
        assert!(book.get_chapter(1).is_ok());
        assert!(book.get_chapter(0).is_err());
    }
}
