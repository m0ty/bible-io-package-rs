use std::{collections::HashMap, error::Error, fmt, fs, str::FromStr};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use simd_json::serde::from_slice as simd_from_slice;

use crate::{
    bible_books_enum::BibleBook, book::Book, chapter::Chapter, search_index::SearchIndex,
    verse::Verse,
};

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
#[derive(Debug, Clone)]
pub struct Bible {
    books: Vec<Book>,
    index_by_abbrev: HashMap<String, usize>,

    /// Lazily constructed search index for verse lookups.
    search_index: Option<SearchIndex>,

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

    /// Returns a specific verse using a human-readable reference string.
    ///
    /// The reference should be in the form "Book Chapter:Verse", for example
    /// `"Genesis 1:1"` or `"Jn 3:16"`. Common book abbreviations are
    /// supported.
    pub fn get_verse_by_reference(&self, reference: &str) -> Result<&Verse, BibleError> {
        let reference = reference.trim();

        // Split verse part
        let (book_and_chapter, verse_str) = reference
            .rsplit_once(':')
            .ok_or_else(|| self.parse_error(reference))?;
        let verse_number: usize = verse_str
            .trim()
            .parse()
            .map_err(|_| self.parse_error(reference))?;

        // Split chapter part
        let (book_str, chapter_str) = book_and_chapter
            .rsplit_once(' ')
            .ok_or_else(|| self.parse_error(book_and_chapter))?;
        let chapter_number: usize = chapter_str
            .trim()
            .parse()
            .map_err(|_| self.parse_error(book_and_chapter))?;

        // Resolve the book reference
        let book = self
            .resolve_book(book_str.trim())
            .ok_or_else(|| self.parse_error(book_str))?;

        self.get_verse(book, chapter_number, verse_number)
    }

    /// Searches the Bible for verses containing all terms in the query.
    ///
    /// A tokenized search index is built on first use and reused on subsequent
    /// queries, providing fast lookups while keeping the public API unchanged.
    pub fn search(&mut self, query: &str) -> Vec<(BibleBook, usize, usize)> {
        if query.is_empty() {
            return Vec::new();
        }

        if self.search_index.is_none() {
            let index = self.build_search_index();
            self.search_index = Some(index);
        }

        // Safe to unwrap: ensured Some above
        self.search_index.as_ref().unwrap().search(query)
    }

    /// Builds a search index for faster repeated searches.
    pub fn build_search_index(&self) -> SearchIndex {
        let mut map: HashMap<String, Vec<(BibleBook, usize, usize)>> = HashMap::new();

        for book in &self.books {
            if let Ok(book_enum) = BibleBook::from_str(book.abbrev()) {
                for (chapter_idx, chapter) in book.chapters().iter().enumerate() {
                    for verse in chapter.get_verses() {
                        for term in SearchIndex::tokenize(verse.text()) {
                            let entry = map.entry(term).or_insert_with(Vec::new);
                            let tuple = (book_enum, chapter_idx + 1, verse.number());
                            if !entry.contains(&tuple) {
                                entry.push(tuple);
                            }
                        }
                    }
                }
            }
        }

        for values in map.values_mut() {
            values.sort_by_key(|&(b, c, v)| (b as usize, c, v));
        }

        SearchIndex::new(map)
    }

    fn parse_error(&self, part: &str) -> BibleError {
        BibleError::BookNotFound {
            book_abbrev: part.to_ascii_lowercase(),
            book_name: part.to_string(),
            translation: self.name.clone(),
        }
    }

    fn resolve_book(&self, input: &str) -> Option<BibleBook> {
        let lower = input.to_ascii_lowercase();

        const ALT_ABBREVS: &[(&str, BibleBook)] = &[
            // --- Protestant (66) ---
            ("gen", BibleBook::Genesis),
            ("ge", BibleBook::Genesis),
            ("exo", BibleBook::Exodus),
            ("exod", BibleBook::Exodus),
            ("lev", BibleBook::Leviticus),
            ("le", BibleBook::Leviticus),
            ("num", BibleBook::Numbers),
            ("nu", BibleBook::Numbers),
            ("deut", BibleBook::Deuteronomy),
            ("deu", BibleBook::Deuteronomy),
            ("jos", BibleBook::Joshua),
            ("josh", BibleBook::Joshua),
            ("jdg", BibleBook::Judges),
            ("judg", BibleBook::Judges),
            ("rut", BibleBook::Ruth),
            ("ru", BibleBook::Ruth),
            ("1sa", BibleBook::FirstSamuel),
            ("1sam", BibleBook::FirstSamuel),
            ("2sa", BibleBook::SecondSamuel),
            ("2sam", BibleBook::SecondSamuel),
            ("1ki", BibleBook::FirstKings),
            ("1kings", BibleBook::FirstKings),
            ("2ki", BibleBook::SecondKings),
            ("2kings", BibleBook::SecondKings),
            ("1ch", BibleBook::FirstChronicles),
            ("1chr", BibleBook::FirstChronicles),
            ("2ch", BibleBook::SecondChronicles),
            ("2chr", BibleBook::SecondChronicles),
            ("ezr", BibleBook::Ezra),
            ("ezra", BibleBook::Ezra),
            ("neh", BibleBook::Nehemiah),
            ("ne", BibleBook::Nehemiah),
            ("est", BibleBook::Esther),
            ("esth", BibleBook::Esther),
            ("job", BibleBook::Job),
            ("jb", BibleBook::Job),
            ("psa", BibleBook::Psalms),
            ("psalm", BibleBook::Psalms),
            ("psalms", BibleBook::Psalms),
            ("pro", BibleBook::Proverbs),
            ("prov", BibleBook::Proverbs),
            ("ecc", BibleBook::Ecclesiastes),
            ("eccl", BibleBook::Ecclesiastes),
            ("sos", BibleBook::SongOfSolomon),
            ("song", BibleBook::SongOfSolomon),
            ("songofsongs", BibleBook::SongOfSolomon),
            ("isa", BibleBook::Isaiah),
            ("jer", BibleBook::Jeremiah),
            ("lam", BibleBook::Lamentations),
            ("ezek", BibleBook::Ezekiel),
            ("eze", BibleBook::Ezekiel),
            ("dan", BibleBook::Daniel),
            ("da", BibleBook::Daniel),
            ("hos", BibleBook::Hosea),
            ("joe", BibleBook::Joel),
            ("amo", BibleBook::Amos),
            ("oba", BibleBook::Obadiah),
            ("obad", BibleBook::Obadiah),
            ("jon", BibleBook::Jonah),
            ("jnh", BibleBook::Jonah),
            ("mic", BibleBook::Micah),
            ("nah", BibleBook::Nahum),
            ("hab", BibleBook::Habakkuk),
            ("zep", BibleBook::Zephaniah),
            ("zeph", BibleBook::Zephaniah),
            ("hag", BibleBook::Haggai),
            ("zec", BibleBook::Zechariah),
            ("zech", BibleBook::Zechariah),
            ("mal", BibleBook::Malachi),
            ("mat", BibleBook::Matthew),
            ("matt", BibleBook::Matthew),
            ("mar", BibleBook::Mark),
            ("mrk", BibleBook::Mark),
            ("luk", BibleBook::Luke),
            ("luke", BibleBook::Luke),
            ("john", BibleBook::John),
            ("jhn", BibleBook::John),
            ("jn", BibleBook::John),
            ("acts", BibleBook::Acts),
            ("ac", BibleBook::Acts),
            ("rom", BibleBook::Romans),
            ("1co", BibleBook::FirstCorinthians),
            ("1cor", BibleBook::FirstCorinthians),
            ("2co", BibleBook::SecondCorinthians),
            ("2cor", BibleBook::SecondCorinthians),
            ("gal", BibleBook::Galatians),
            ("eph", BibleBook::Ephesians),
            ("phil", BibleBook::Philippians),
            ("php", BibleBook::Philippians),
            ("col", BibleBook::Colossians),
            ("1th", BibleBook::FirstThessalonians),
            ("1thes", BibleBook::FirstThessalonians),
            ("2th", BibleBook::SecondThessalonians),
            ("2thes", BibleBook::SecondThessalonians),
            ("1ti", BibleBook::FirstTimothy),
            ("1tim", BibleBook::FirstTimothy),
            ("2ti", BibleBook::SecondTimothy),
            ("2tim", BibleBook::SecondTimothy),
            ("tit", BibleBook::Titus),
            ("phm", BibleBook::Philemon),
            ("phlm", BibleBook::Philemon),
            ("philemon", BibleBook::Philemon),
            ("heb", BibleBook::Hebrews),
            ("jas", BibleBook::James),
            ("jam", BibleBook::James),
            ("1pe", BibleBook::FirstPeter),
            ("1pet", BibleBook::FirstPeter),
            ("2pe", BibleBook::SecondPeter),
            ("2pet", BibleBook::SecondPeter),
            ("1jn", BibleBook::FirstJohn),
            ("1joh", BibleBook::FirstJohn),
            ("2jn", BibleBook::SecondJohn),
            ("2joh", BibleBook::SecondJohn),
            ("3jn", BibleBook::ThirdJohn),
            ("3joh", BibleBook::ThirdJohn),
            ("jud", BibleBook::Jude),
            ("jude", BibleBook::Jude),
            ("rev", BibleBook::Revelation),
            ("revelation", BibleBook::Revelation),
            // --- Catholic Deuterocanon ---
            ("tob", BibleBook::Tobit),
            ("jdt", BibleBook::Judith),
            ("wis", BibleBook::Wisdom),
            ("sir", BibleBook::Sirach),
            ("bar", BibleBook::Baruch),
            ("1mac", BibleBook::FirstMaccabees),
            ("2mac", BibleBook::SecondMaccabees),
            ("estg", BibleBook::EstherAdditions),
            ("addesth", BibleBook::EstherAdditions),
            ("dan3", BibleBook::DanielSongOfThree),
            ("sus", BibleBook::DanielSusanna),
            ("bel", BibleBook::DanielBelAndTheDragon),
            // --- Eastern Orthodox Additions ---
            ("1esd", BibleBook::FirstEsdras),
            ("2esd", BibleBook::SecondEsdras),
            ("man", BibleBook::PrayerOfManasseh),
            ("prman", BibleBook::PrayerOfManasseh),
            ("ps151", BibleBook::Psalm151),
            ("3mac", BibleBook::ThirdMaccabees),
            ("4mac", BibleBook::FourthMaccabees),
        ];

        ALT_ABBREVS
            .iter()
            .find(|(abbr, _)| *abbr == lower)
            .map(|(_, book)| *book)
            .or_else(|| {
                // Try official abbreviations
                BibleBook::from_str(&lower).ok()
            })
            .or_else(|| {
                // Try full book titles from loaded data
                self.books
                    .iter()
                    .find(|b| b.title().eq_ignore_ascii_case(input))
                    .and_then(|b| BibleBook::from_str(&b.abbrev().to_ascii_lowercase()).ok())
            })
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
            search_index: None,
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
            search_index: None,
            id: "id".to_string(),
            name: "name".to_string(),
            description: "desc".to_string(),
            language: "lang".to_string(),
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

    #[test]
    fn test_clone_independence() {
        let original = create_test_bible();
        let cloned = original.clone();

        assert_eq!(original.id(), cloned.id());
        assert_eq!(original.name(), cloned.name());
        assert_eq!(original.description(), cloned.description());
        assert_eq!(original.language(), cloned.language());
        assert_eq!(original.books().len(), cloned.books().len());
        assert_eq!(original.books()[0].title(), cloned.books()[0].title());

        // Ensure cloned Bible owns its data
        assert_ne!(original.books().as_ptr(), cloned.books().as_ptr());
        assert_ne!(original.name().as_ptr(), cloned.name().as_ptr());
    }
}
