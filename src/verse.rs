use std::fmt;

use crate::bible_books_enum::BibleBook;

/// Represents a single verse from the Bible.
///
/// A verse contains the text content and its reference information within a chapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verse {
    book: BibleBook,
    chapter_number: usize,
    verse_text: String,
    verse_number: usize,
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
        Verse {
            book,
            chapter_number,
            verse_text: sanitize_verse_text(verse_text),
            verse_number,
        }
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
}

fn sanitize_verse_text(verse_text: String) -> String {
    verse_text
        .chars()
        .filter(|c| *c != '{' && *c != '}')
        .collect()
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
        assert_eq!(verse.text(), "In the beginning");
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
