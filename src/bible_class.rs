use std::collections::HashMap;
use std::fmt;
use std::fs;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use simd_json::serde::from_str as simd_from_str;

use crate::bible_books_enum::BibleBook;

/// Internal structure for deserializing JSON data from Bible files.
#[derive(Serialize, Deserialize, Debug)]
struct FileDataEntry {
    chapters: Vec<Vec<String>>,
    name: String,
}

/// Represents a single verse from the Bible.
///
/// A verse contains the text content and its verse number within a chapter.
#[derive(Debug)]
pub struct Verse {
    verse_text: String,
    verse_number: usize,
}

impl Verse {
    /// Creates a new verse with the given text and verse number.
    ///
    /// # Arguments
    ///
    /// * `verse_text` - The text content of the verse
    /// * `verse_number` - The verse number within its chapter
    pub fn new(verse_text: String, verse_number: usize) -> Self {
        Verse {
            verse_text,
            verse_number,
        }
    }
}

impl fmt::Display for Verse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.verse_number, self.verse_text)
    }
}

/// Represents a chapter from a Bible book.
///
/// A chapter contains multiple verses and has a chapter number.
#[derive(Debug)]
pub struct Chapter {
    verses: Vec<Verse>,
    chapter_number: usize,
}

impl Chapter {
    /// Creates a new chapter with the given verses and chapter number.
    ///
    /// # Arguments
    ///
    /// * `verses` - A vector of verses in this chapter
    /// * `chapter_number` - The chapter number within the book
    pub fn new(verses: Vec<Verse>, chapter_number: usize) -> Self {
        Chapter {
            verses,
            chapter_number,
        }
    }

    /// Returns a reference to all verses in this chapter.
    ///
    /// # Returns
    ///
    /// A reference to the vector of verses in this chapter.
    pub fn get_verses(&self) -> &Vec<Verse> {
        &self.verses
    }

    /// Returns a specific verse by its verse number.
    ///
    /// # Arguments
    ///
    /// * `verse_number` - The verse number to retrieve
    ///
    /// # Returns
    ///
    /// An optional reference to the verse if found, None otherwise.
    pub fn get_verse(&self, verse_number: usize) -> Option<&Verse> {
        self.verses.iter().find(|v| v.verse_number == verse_number)
    }
}

impl fmt::Display for Chapter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let verses_str: String = self
            .verses
            .iter()
            .map(|v| format!("{}", v))
            .collect::<Vec<String>>()
            .join("\n");
        write!(f, "Chapter {}:\n{}", self.chapter_number, verses_str)
    }
}

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
            abbrev,
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

    /// Returns a specific chapter by its chapter number.
    ///
    /// # Arguments
    ///
    /// * `chapter_number` - The chapter number to retrieve
    ///
    /// # Returns
    ///
    /// An optional reference to the chapter if found, None otherwise.
    pub fn get_chapter(&self, chapter_number: usize) -> Option<&Chapter> {
        self.chapters
            .iter()
            .find(|c| c.chapter_number == chapter_number)
    }

    /// Returns all verses from a specific chapter.
    ///
    /// # Arguments
    ///
    /// * `chapter_number` - The chapter number to retrieve verses from
    ///
    /// # Returns
    ///
    /// An optional reference to the vector of verses if the chapter exists, None otherwise.
    pub fn get_verses(&self, chapter_number: usize) -> Option<&Vec<Verse>> {
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
    /// An optional reference to the verse if found, None otherwise.
    pub fn get_verse(&self, chapter_number: usize, verse_number: usize) -> Option<&Verse> {
        self.get_chapter(chapter_number)
            .and_then(|c| c.get_verse(verse_number))
    }
}

impl fmt::Display for Book {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Book: {} ({})", self.title, self.abbrev)
    }
}

/// Represents the complete Bible with all books, chapters, and verses.
///
/// The Bible struct provides efficient access to any verse, chapter, or book
/// using O(1) lookup times for book access by abbreviation.
#[derive(Debug)]
pub struct Bible {
    // Preserves the JSON order
    books: Vec<Book>,
    // Abbrev -> index in `books` (for O(1) lookup by key)
    index_by_abbrev: HashMap<String, usize>,
}

impl Bible {
    /// Returns a book by its BibleBook enum value.
    ///
    /// # Arguments
    ///
    /// * `book` - The BibleBook enum value
    ///
    /// # Returns
    ///
    /// An optional reference to the book if found, None otherwise.
    pub fn get_book(&self, book: BibleBook) -> Option<&Book> {
        self.get_book_by_abbrev(book.as_str())
    }

    /// Returns a book by its abbreviation string.
    ///
    /// # Arguments
    ///
    /// * `abbrev` - The book's abbreviation (e.g., "gn", "ps", "mt")
    ///
    /// # Returns
    ///
    /// An optional reference to the book if found, None otherwise.
    pub fn get_book_by_abbrev(&self, abbrev: &str) -> Option<&Book> {
        self.index_by_abbrev
            .get(abbrev)
            .and_then(|&i| self.books.get(i))
    }

    /// Returns all verses from a specific book and chapter.
    ///
    /// # Arguments
    ///
    /// * `book` - The BibleBook enum value
    /// * `chapter_number` - The chapter number
    ///
    /// # Returns
    ///
    /// An optional reference to the vector of verses if found, None otherwise.
    pub fn get_verses(&self, book: BibleBook, chapter_number: usize) -> Option<&Vec<Verse>> {
        self.get_book(book)
            .and_then(|b| b.get_verses(chapter_number))
    }

    /// Returns a specific verse by book, chapter, and verse number.
    ///
    /// # Arguments
    ///
    /// * `book` - The BibleBook enum value
    /// * `chapter_number` - The chapter number
    /// * `verse_number` - The verse number within the chapter
    ///
    /// # Returns
    ///
    /// An optional reference to the verse if found, None otherwise.
    pub fn get_verse(
        &self,
        book: BibleBook,
        chapter_number: usize,
        verse_number: usize,
    ) -> Option<&Verse> {
        self.get_book(book)
            .and_then(|b| b.get_verse(chapter_number, verse_number))
    }

    fn new_from_map(map: IndexMap<String, FileDataEntry>) -> Self {
        // Iterate in map order (IndexMap preserves insertion order)
        let mut books = Vec::with_capacity(map.len());

        for (abbrev, entry) in map.iter() {
            let chapters = entry
                .chapters
                .iter()
                .enumerate()
                .map(|(chapter_idx, verses)| {
                    let verses = verses
                        .iter()
                        .enumerate()
                        .map(|(verse_idx, verse_text)| {
                            Verse::new(verse_text.clone(), verse_idx + 1)
                        })
                        .collect();
                    Chapter::new(verses, chapter_idx + 1)
                })
                .collect::<Vec<_>>();

            books.push(Book::new(abbrev.clone(), entry.name.clone(), chapters));
        }

        // Build abbrev index
        let mut index_by_abbrev = HashMap::with_capacity(books.len());
        for (i, b) in books.iter().enumerate() {
            index_by_abbrev.insert(b.abbrev.clone(), i);
        }

        Bible {
            books,
            index_by_abbrev,
        }
    }

    /// Creates a new Bible instance from a JSON file.
    ///
    /// # Arguments
    ///
    /// * `json_path` - The path to the JSON file containing Bible data
    ///
    /// # Panics
    ///
    /// This function will panic if the file cannot be read or if the JSON
    /// cannot be parsed. The JSON should have the structure where each book
    /// is a key with an object containing "name" and "chapters" fields.
    pub fn new_from_json(json_path: &str) -> Self {
        // Read entire file into a mutable String
        let mut file_content = fs::read_to_string(json_path)
            .expect("Failed to read the file. Make sure the path is correct.");

        // Parse via SIMD-accelerated serde adapter into an IndexMap to preserve order
        let parsed: IndexMap<String, FileDataEntry> = unsafe {
            simd_from_str(&mut file_content)
                .expect("Failed to parse JSON with simd-json. Check structure & CPU features.")
        };

        Bible::new_from_map(parsed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bible_books_enum::BibleBook;

    fn create_test_verse() -> Verse {
        Verse::new(
            "In the beginning God created the heaven and the earth.".to_string(),
            1,
        )
    }

    fn create_test_chapter() -> Chapter {
        let verses = vec![
            Verse::new("In the beginning God created the heaven and the earth.".to_string(), 1),
            Verse::new("And the earth was without form, and void; and darkness was upon the face of the deep.".to_string(), 2),
            Verse::new("And the Spirit of God moved upon the face of the waters.".to_string(), 3),
        ];
        Chapter::new(verses, 1)
    }

    fn create_test_book() -> Book {
        let chapters = vec![
            create_test_chapter(),
            Chapter::new(
                vec![
                    Verse::new(
                        "Thus the heavens and the earth were finished.".to_string(),
                        1,
                    ),
                    Verse::new("And on the seventh day God ended his work.".to_string(), 2),
                ],
                2,
            ),
        ];
        Book::new("gn".to_string(), "Genesis".to_string(), chapters)
    }

    fn create_test_bible() -> Bible {
        let book = create_test_book();
        let mut index_by_abbrev = HashMap::new();
        index_by_abbrev.insert("gn".to_string(), 0);
        Bible {
            books: vec![book],
            index_by_abbrev,
        }
    }

    #[test]
    fn test_verse_creation() {
        let verse = create_test_verse();
        assert_eq!(
            verse.verse_text,
            "In the beginning God created the heaven and the earth."
        );
        assert_eq!(verse.verse_number, 1);
    }

    #[test]
    fn test_verse_display() {
        let verse = create_test_verse();
        let display = format!("{}", verse);
        assert_eq!(
            display,
            "1: In the beginning God created the heaven and the earth."
        );
    }

    #[test]
    fn test_chapter_creation() {
        let chapter = create_test_chapter();
        assert_eq!(chapter.chapter_number, 1);
        assert_eq!(chapter.verses.len(), 3);
    }

    #[test]
    fn test_chapter_get_verses() {
        let chapter = create_test_chapter();
        let verses = chapter.get_verses();
        assert_eq!(verses.len(), 3);
        assert_eq!(verses[0].verse_number, 1);
        assert_eq!(verses[1].verse_number, 2);
        assert_eq!(verses[2].verse_number, 3);
    }

    #[test]
    fn test_chapter_get_verse() {
        let chapter = create_test_chapter();

        // Test valid verse
        let verse = chapter.get_verse(2);
        assert!(verse.is_some());
        assert_eq!(verse.unwrap().verse_number, 2);

        // Test invalid verse
        let verse = chapter.get_verse(99);
        assert!(verse.is_none());
    }

    #[test]
    fn test_chapter_display() {
        let chapter = create_test_chapter();
        let display = format!("{}", chapter);
        assert!(display.contains("Chapter 1:"));
        assert!(display.contains("1: In the beginning God created the heaven and the earth."));
        assert!(display.contains("2: And the earth was without form, and void; and darkness was upon the face of the deep."));
        assert!(display.contains("3: And the Spirit of God moved upon the face of the waters."));
    }

    #[test]
    fn test_book_creation() {
        let book = create_test_book();
        assert_eq!(book.abbrev(), "gn");
        assert_eq!(book.title(), "Genesis");
        assert_eq!(book.chapters.len(), 2);
    }

    #[test]
    fn test_book_get_chapter() {
        let book = create_test_book();

        // Test valid chapter
        let chapter = book.get_chapter(1);
        assert!(chapter.is_some());
        assert_eq!(chapter.unwrap().chapter_number, 1);

        // Test invalid chapter
        let chapter = book.get_chapter(99);
        assert!(chapter.is_none());
    }

    #[test]
    fn test_book_get_verses() {
        let book = create_test_book();

        // Test valid chapter
        let verses = book.get_verses(1);
        assert!(verses.is_some());
        assert_eq!(verses.unwrap().len(), 3);

        // Test invalid chapter
        let verses = book.get_verses(99);
        assert!(verses.is_none());
    }

    #[test]
    fn test_book_get_verse() {
        let book = create_test_book();

        // Test valid verse
        let verse = book.get_verse(1, 2);
        assert!(verse.is_some());
        assert_eq!(verse.unwrap().verse_number, 2);

        // Test invalid chapter
        let verse = book.get_verse(99, 1);
        assert!(verse.is_none());

        // Test invalid verse
        let verse = book.get_verse(1, 99);
        assert!(verse.is_none());
    }

    #[test]
    fn test_book_display() {
        let book = create_test_book();
        let display = format!("{}", book);
        assert_eq!(display, "Book: Genesis (gn)");
    }

    #[test]
    fn test_bible_creation() {
        let bible = create_test_bible();
        assert_eq!(bible.books.len(), 1);
        assert!(bible.index_by_abbrev.contains_key("gn"));
    }

    #[test]
    fn test_bible_get_book() {
        let bible = create_test_bible();

        // Test valid book
        let book = bible.get_book(BibleBook::Genesis);
        assert!(book.is_some());
        assert_eq!(book.unwrap().abbrev(), "gn");

        // Test invalid book
        let book = bible.get_book(BibleBook::Exodus);
        assert!(book.is_none());
    }

    #[test]
    fn test_bible_get_book_by_abbrev() {
        let bible = create_test_bible();

        // Test valid abbreviation
        let book = bible.get_book_by_abbrev("gn");
        assert!(book.is_some());
        assert_eq!(book.unwrap().title(), "Genesis");

        // Test invalid abbreviation
        let book = bible.get_book_by_abbrev("ex");
        assert!(book.is_none());
    }

    #[test]
    fn test_bible_get_verses() {
        let bible = create_test_bible();

        // Test valid book and chapter
        let verses = bible.get_verses(BibleBook::Genesis, 1);
        assert!(verses.is_some());
        assert_eq!(verses.unwrap().len(), 3);

        // Test invalid book
        let verses = bible.get_verses(BibleBook::Exodus, 1);
        assert!(verses.is_none());

        // Test invalid chapter
        let verses = bible.get_verses(BibleBook::Genesis, 99);
        assert!(verses.is_none());
    }

    #[test]
    fn test_bible_get_verse() {
        let bible = create_test_bible();

        // Test valid verse
        let verse = bible.get_verse(BibleBook::Genesis, 1, 2);
        assert!(verse.is_some());
        assert_eq!(verse.unwrap().verse_number, 2);

        // Test invalid book
        let verse = bible.get_verse(BibleBook::Exodus, 1, 1);
        assert!(verse.is_none());

        // Test invalid chapter
        let verse = bible.get_verse(BibleBook::Genesis, 99, 1);
        assert!(verse.is_none());

        // Test invalid verse
        let verse = bible.get_verse(BibleBook::Genesis, 1, 99);
        assert!(verse.is_none());
    }

    #[test]
    fn test_bible_new_from_map() {
        use indexmap::IndexMap;

        let mut map = IndexMap::new();
        map.insert(
            "gn".to_string(),
            FileDataEntry {
                name: "Genesis".to_string(),
                chapters: vec![
                    vec!["Verse 1".to_string(), "Verse 2".to_string()],
                    vec!["Verse 1".to_string()],
                ],
            },
        );

        let bible = Bible::new_from_map(map);
        assert_eq!(bible.books.len(), 1);
        assert_eq!(bible.books[0].abbrev(), "gn");
        assert_eq!(bible.books[0].title(), "Genesis");
        assert_eq!(bible.books[0].chapters.len(), 2);
        assert_eq!(bible.books[0].chapters[0].verses.len(), 2);
        assert_eq!(bible.books[0].chapters[1].verses.len(), 1);
    }

    #[test]
    fn test_debug_traits() {
        let verse = create_test_verse();
        let chapter = create_test_chapter();
        let book = create_test_book();
        let bible = create_test_bible();

        // Test that Debug trait works for all structs
        let _debug_verse = format!("{:?}", verse);
        let _debug_chapter = format!("{:?}", chapter);
        let _debug_book = format!("{:?}", book);
        let _debug_bible = format!("{:?}", bible);

        // If we get here without panicking, the Debug traits work
        // No assertion needed - if we reach this point, the Debug traits work
    }

    #[test]
    fn test_verse_numbering() {
        let verses = [
            Verse::new("First verse".to_string(), 1),
            Verse::new("Second verse".to_string(), 2),
            Verse::new("Third verse".to_string(), 3),
        ];

        for (i, verse) in verses.iter().enumerate() {
            assert_eq!(verse.verse_number, i + 1);
        }
    }

    #[test]
    fn test_chapter_numbering() {
        let chapters = [
            Chapter::new(vec![], 1),
            Chapter::new(vec![], 2),
            Chapter::new(vec![], 3),
        ];

        for (i, chapter) in chapters.iter().enumerate() {
            assert_eq!(chapter.chapter_number, i + 1);
        }
    }
}
