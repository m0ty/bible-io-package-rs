use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::str::FromStr;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use simd_json::serde::from_str as simd_from_str;

use crate::bible_books_enum::BibleBook;

/// Errors that can occur when accessing Bible content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BibleError {
    /// The requested book is not present in the specified Bible translation.
    BookNotFound {
        book_abbrev: String,
        book_name: String,
        translation: String,
    },
    /// The requested chapter number does not exist in the specified book.
    ChapterOutOfBounds {
        book_abbrev: String,
        book_name: String,
        chapter: usize,
        max_chapter: usize,
    },
    /// The requested verse number does not exist in the specified chapter of the book.
    VerseOutOfBounds {
        book_abbrev: String,
        book_name: String,
        chapter: usize,
        verse: usize,
        max_verse: usize,
    },
}

impl fmt::Display for BibleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BibleError::BookNotFound {
                book_abbrev,
                book_name,
                translation,
            } => {
                write!(
                    f,
                    "Book {} ('{}') not found in the '{}' Bible translation",
                    book_name, book_abbrev, translation
                )
            }
            BibleError::ChapterOutOfBounds {
                book_abbrev,
                book_name,
                chapter,
                max_chapter,
            } => {
                write!(
                    f,
                    "Chapter {} is out of bounds for book {} ('{}') (max {})",
                    chapter, book_name, book_abbrev, max_chapter
                )
            }
            BibleError::VerseOutOfBounds {
                book_abbrev,
                book_name,
                chapter,
                verse,
                max_verse,
            } => {
                write!(
                    f,
                    "Verse {} is out of bounds for book {} ('{}') chapter {} (max {})",
                    verse, book_name, book_abbrev, chapter, max_verse
                )
            }
        }
    }
}

impl std::error::Error for BibleError {}

#[derive(Deserialize, Debug)]
struct BibleFileRoot {
    id: String,
    name: String,
    description: String,
    language: String,
    books: IndexMap<String, FileDataEntry>,
}

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
    /// The requested chapter or a descriptive error if the chapter number is invalid.
    pub fn get_chapter(&self, chapter_number: usize) -> Result<&Chapter, BibleError> {
        self.chapters
            .iter()
            .find(|c| c.chapter_number == chapter_number)
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
    pub fn get_verses(&self, chapter_number: usize) -> Result<&Vec<Verse>, BibleError> {
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

/// Represents the complete Bible with all books, chapters, and verses.
///
/// The Bible struct provides efficient access to any verse, chapter, or book
#[derive(Debug)]
pub struct Bible {
    books: Vec<Book>,
    index_by_abbrev: HashMap<String, usize>,

    id: String,
    name: String,
    description: String,
    language: String,
}

impl Bible {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    pub fn language(&self) -> &str {
        &self.language
    }

    /// Returns a book by its BibleBook enum value.
    ///
    /// # Arguments
    ///
    /// * `book` - The BibleBook enum value
    ///
    /// # Returns
    ///
    /// The requested book or a descriptive error if it is not present in this translation.
    pub fn get_book(&self, book: BibleBook) -> Result<&Book, BibleError> {
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
    /// The requested book or a descriptive error if it is not present in this translation.
    pub fn get_book_by_abbrev(&self, abbrev: &str) -> Result<&Book, BibleError> {
        let key = abbrev.to_ascii_lowercase();
        self.index_by_abbrev
            .get(key.as_str())
            .and_then(|&i| self.books.get(i))
            .ok_or_else(|| {
                let book_name = BibleBook::from_str(&key)
                    .map(|b| b.full_name().to_string())
                    .unwrap_or_else(|_| key.clone());
                BibleError::BookNotFound {
                    book_abbrev: key.clone(),
                    book_name,
                    translation: self.name.clone(),
                }
            })
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
    /// The verses from the requested chapter or a descriptive error.
    pub fn get_verses(
        &self,
        book: BibleBook,
        chapter_number: usize,
    ) -> Result<&Vec<Verse>, BibleError> {
        self.get_book(book)?.get_verses(chapter_number)
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
    /// The requested verse or a descriptive error if the book, chapter, or verse is invalid.
    pub fn get_verse(
        &self,
        book: BibleBook,
        chapter_number: usize,
        verse_number: usize,
    ) -> Result<&Verse, BibleError> {
        self.get_book(book)?.get_verse(chapter_number, verse_number)
    }

    fn new_from_map_with_meta(
        map: IndexMap<String, FileDataEntry>,
        id: String,
        name: String,
        description: String,
        language: String,
    ) -> Self {
        // Iterate in map order (IndexMap preserves insertion order)
        let mut books = Vec::with_capacity(map.len());

        for (abbrev, entry) in map.into_iter() {
            let chapters = entry
                .chapters
                .into_iter()
                .enumerate()
                .map(|(chapter_idx, verses)| {
                    let verses = verses
                        .into_iter()
                        .enumerate()
                        .map(|(verse_idx, verse_text)| Verse::new(verse_text, verse_idx + 1))
                        .collect::<Vec<_>>();
                    Chapter::new(verses, chapter_idx + 1)
                })
                .collect::<Vec<_>>();

            books.push(Book::new(abbrev, entry.name, chapters));
        }

        // Build abbrev index
        let mut index_by_abbrev = HashMap::with_capacity(books.len());
        for (i, b) in books.iter().enumerate() {
            index_by_abbrev.insert(b.abbrev.clone(), i);
        }

        Bible {
            books,
            index_by_abbrev,
            id,
            name,
            description,
            language,
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
        let mut file_content = fs::read_to_string(json_path)
            .expect("Failed to read the file. Make sure the path is correct.");

        let root: BibleFileRoot = unsafe {
            simd_from_str(&mut file_content)
                .expect("Failed to parse JSON with simd-json. Check structure & CPU features.")
        };

        Bible::new_from_map_with_meta(
            root.books,
            root.id,
            root.name,
            root.description,
            root.language,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bible_books_enum::BibleBook;

    #[test]
    fn test_bible_parse_new_wrapped_format() {
        let mut json = String::from(
            r#"
        {
            "id": "en-kjv",
            "name": "King James Version",
            "description": "King James Version of the Holy Bible",
            "language": "English",
            "books": {
                "gn": {
                    "name": "Genesis",
                    "chapters": [
                        ["Verse 1", "Verse 2"],
                        ["Verse 1"]
                    ]
                }
            }
        }
        "#,
        );

        let root: BibleFileRoot = unsafe { simd_from_str(&mut json).unwrap() };

        // Build Bible with metadata populated
        let bible = Bible::new_from_map_with_meta(
            root.books,
            root.id,
            root.name,
            root.description,
            root.language,
        );

        // Metadata assertions (optional)
        assert_eq!(bible.id(), "en-kjv");
        assert_eq!(bible.name(), "King James Version");
        assert_eq!(bible.description(), "King James Version of the Holy Bible");
        assert_eq!(bible.language(), "English");

        // Content assertions
        let gn = bible.get_book_by_abbrev("gn").unwrap();
        assert_eq!(gn.title(), "Genesis");
        assert_eq!(gn.get_chapter(1).unwrap().get_verses().len(), 2);
        assert_eq!(gn.get_chapter(2).unwrap().get_verses().len(), 1);
    }

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
            id: "en-kjv".to_string(),
            name: "King James Version".to_string(),
            description: "Test Bible".to_string(),
            language: "English".to_string(),
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
        let chapter = book.get_chapter(1).unwrap();
        assert_eq!(chapter.chapter_number, 1);

        // Test invalid chapter
        let err = book.get_chapter(99).unwrap_err();
        assert!(matches!(err, BibleError::ChapterOutOfBounds { .. }));
    }

    #[test]
    fn test_book_get_verses() {
        let book = create_test_book();

        // Test valid chapter
        let verses = book.get_verses(1).unwrap();
        assert_eq!(verses.len(), 3);

        // Test invalid chapter
        let err = book.get_verses(99).unwrap_err();
        assert!(matches!(err, BibleError::ChapterOutOfBounds { .. }));
    }

    #[test]
    fn test_book_get_verse() {
        let book = create_test_book();

        // Test valid verse
        let verse = book.get_verse(1, 2).unwrap();
        assert_eq!(verse.verse_number, 2);

        // Test invalid chapter
        let err = book.get_verse(99, 1).unwrap_err();
        assert!(matches!(err, BibleError::ChapterOutOfBounds { .. }));

        // Test invalid verse
        let err = book.get_verse(1, 99).unwrap_err();
        assert!(matches!(err, BibleError::VerseOutOfBounds { .. }));
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
        let book = bible.get_book(BibleBook::Genesis).unwrap();
        assert_eq!(book.abbrev(), "gn");

        // Test invalid book
        let err = bible.get_book(BibleBook::Exodus).unwrap_err();
        if let BibleError::BookNotFound { translation, .. } = err {
            assert_eq!(translation, "King James Version");
        } else {
            panic!("Expected BookNotFound error");
        }
    }

    #[test]
    fn test_bible_get_book_by_abbrev() {
        let bible = create_test_bible();

        // Test valid abbreviation
        let book = bible.get_book_by_abbrev("gn").unwrap();
        assert_eq!(book.title(), "Genesis");

        // Test case-insensitive lookup
        let book = bible.get_book_by_abbrev("GN").unwrap();
        assert_eq!(book.title(), "Genesis");

        // Test invalid abbreviation
        let err = bible.get_book_by_abbrev("ex").unwrap_err();
        assert!(matches!(err, BibleError::BookNotFound { .. }));
    }

    #[test]
    fn test_bible_get_verses() {
        let bible = create_test_bible();

        // Test valid book and chapter
        let verses = bible.get_verses(BibleBook::Genesis, 1).unwrap();
        assert_eq!(verses.len(), 3);

        // Test invalid book
        let err = bible.get_verses(BibleBook::Exodus, 1).unwrap_err();
        assert!(matches!(err, BibleError::BookNotFound { .. }));

        // Test invalid chapter
        let err = bible.get_verses(BibleBook::Genesis, 99).unwrap_err();
        assert!(matches!(err, BibleError::ChapterOutOfBounds { .. }));
    }

    #[test]
    fn test_bible_get_verse() {
        let bible = create_test_bible();

        // Test valid verse
        let verse = bible.get_verse(BibleBook::Genesis, 1, 2).unwrap();
        assert_eq!(verse.verse_number, 2);

        // Test invalid book
        let err = bible.get_verse(BibleBook::Exodus, 1, 1).unwrap_err();
        assert!(matches!(err, BibleError::BookNotFound { .. }));

        // Test invalid chapter
        let err = bible.get_verse(BibleBook::Genesis, 99, 1).unwrap_err();
        assert!(matches!(err, BibleError::ChapterOutOfBounds { .. }));

        // Test invalid verse
        let err = bible.get_verse(BibleBook::Genesis, 1, 99).unwrap_err();
        assert!(matches!(err, BibleError::VerseOutOfBounds { .. }));
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

        // Build via the helper and keep metadata empty (matches prior semantics)
        let bible = Bible::new_from_map_with_meta(
            map,
            String::new(),
            String::new(),
            String::new(),
            String::new(),
        );

        // If you have getters; otherwise assert fields directly.
        assert_eq!(bible.id(), "");
        assert_eq!(bible.name(), "");
        assert_eq!(bible.description(), "");
        assert_eq!(bible.language(), "");

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
