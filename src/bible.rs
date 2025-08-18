use std::{collections::HashMap, error::Error, fmt, fs, str::FromStr};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use simd_json::serde::from_slice as simd_from_slice;

use crate::{bible_books_enum::BibleBook, book::Book, chapter::Chapter, verse::Verse};

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

    /// Returns a slice of all books in this Bible.
    pub fn books(&self) -> &[Book] {
        &self.books
    }

    /// Returns a book by its BibleBook enum value.
    pub fn get_book(&self, book: BibleBook) -> Result<&Book, BibleError> {
        self.get_book_by_abbrev(book.as_str())
    }

    /// Returns a book by its abbreviation string.
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
    pub fn get_verses(
        &self,
        book: BibleBook,
        chapter_number: usize,
    ) -> Result<&[Verse], BibleError> {
        self.get_book(book)?.get_verses(chapter_number)
    }

    /// Returns a specific verse by book, chapter, and verse number.
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
            index_by_abbrev.insert(b.abbrev().to_ascii_lowercase(), i);
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
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or if the JSON cannot be
    /// parsed. The JSON should have the structure where each book is a key
    /// with an object containing "name" and "chapters" fields.
    pub fn new_from_json(json_path: &str) -> Result<Self, Box<dyn Error>> {
        let mut file_content = fs::read(json_path)?;
        let root: BibleFileRoot = simd_from_slice(&mut file_content)?;

        Ok(Bible::new_from_map_with_meta(
            root.books,
            root.id,
            root.name,
            root.description,
            root.language,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bible_books_enum::BibleBook;
    use std::collections::HashMap;

    fn create_test_bible() -> Bible {
        let verse = Verse::new("In the beginning".to_string(), 1);
        let chapter = Chapter::new(vec![verse], 1);
        let book = Book::new("GN".to_string(), "Genesis".to_string(), vec![chapter]);
        let mut index_by_abbrev = HashMap::new();
        index_by_abbrev.insert("gn".to_string(), 0);

        Bible {
            books: vec![book],
            index_by_abbrev,
            id: String::new(),
            name: String::new(),
            description: String::new(),
            language: String::new(),
        }
    }

    #[test]
    fn test_get_book_and_verse() {
        let bible = create_test_bible();
        let book = bible.get_book(BibleBook::Genesis).unwrap();
        assert_eq!(book.title(), "Genesis");
        let verse = bible.get_verse(BibleBook::Genesis, 1, 1).unwrap();
        assert_eq!(verse.number(), 1);
    }
}
