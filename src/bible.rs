use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fmt, fs,
};

use bible_io_references::{Language, ParseErrorKind, ReferenceParser};
use indexmap::IndexMap;
use serde::{de, Deserialize, Deserializer, Serialize};
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
    /// The provided reference string could not be parsed.
    InvalidReference { input: String },
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
            BibleError::InvalidReference { input } => {
                write!(f, "Invalid reference: '{}'", input)
            }
        }
    }
}

impl Error for BibleError {}

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
    #[serde(deserialize_with = "deserialize_chapters")]
    chapters: Vec<Vec<String>>,
    name: String,
}

#[derive(Debug)]
struct BibleInitializationData {
    books: Vec<Book>,
    search_index: Option<SearchIndex>,
    id: String,
    name: String,
    description: String,
    language: String,
}

// These aliases preserve the handful of historical inputs that are not
// equivalent in bible-io-references. The shared parser handles everything else.
const LEGACY_REFERENCE_ALIASES: &[(&str, BibleBook)] = &[
    ("ge", BibleBook::Genesis),
    ("le", BibleBook::Leviticus),
    ("nu", BibleBook::Numbers),
    ("sos", BibleBook::SongOfSolomon),
    ("songofsongs", BibleBook::SongOfSolomon),
    ("da", BibleBook::Daniel),
    ("joe", BibleBook::Joel),
    ("1thes", BibleBook::FirstThessalonians),
    ("2thes", BibleBook::SecondThessalonians),
    ("jam", BibleBook::James),
    ("estg", BibleBook::EstherAdditions),
    ("dan3", BibleBook::DanielSongOfThree),
    ("jn", BibleBook::John),
    ("jud", BibleBook::Jude),
];

fn build_reference_parser(books: &[Book], language: &str) -> ReferenceParser {
    let mut builder = ReferenceParser::builder();

    if let Ok(language) = language.parse::<Language>() {
        if !language.is_auto() && language.is_parsing_supported() {
            builder = builder.preferred_languages([language]);
        }
    }

    for &(alias, book) in LEGACY_REFERENCE_ALIASES {
        builder = builder.alias(alias, book);
    }

    // Preserve support for translation-specific titles from the loaded JSON.
    for book in books {
        if let Some(book_id) = BibleBook::from_abbreviation(book.abbrev()) {
            builder = builder.alias(book.title(), book_id);
        }
    }

    builder
        .build()
        .expect("the compatibility reference parser configuration is valid")
}

fn reference_book_token(reference: &str) -> &str {
    let before_verse = reference
        .rfind([':', '.'])
        .map_or(reference, |separator| &reference[..separator]);
    let book = before_verse
        .trim_end_matches(|character: char| character.is_whitespace() || character.is_numeric());

    if book.is_empty() {
        reference.trim()
    } else {
        book.trim()
    }
}

fn deserialize_chapters<'de, D>(deserializer: D) -> Result<Vec<Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum ChaptersHelper {
        Array(Vec<Vec<String>>),
        Map(IndexMap<String, IndexMap<String, String>>),
    }

    let helper = ChaptersHelper::deserialize(deserializer)?;

    match helper {
        ChaptersHelper::Array(chapters) => Ok(chapters),
        ChaptersHelper::Map(map) => map
            .into_iter()
            .map(|(chapter_key, verses)| {
                let chapter_num = chapter_key.parse::<usize>().map_err(|_| {
                    de::Error::custom(format!(
                        "Invalid chapter key '{}': expected positive integer",
                        chapter_key
                    ))
                })?;

                let mut verses_vec = verses
                    .into_iter()
                    .map(|(verse_key, text)| {
                        let verse_num = verse_key.parse::<usize>().map_err(|_| {
                            de::Error::custom(format!(
                                "Invalid verse key '{}': expected positive integer",
                                verse_key
                            ))
                        })?;

                        Ok((verse_num, text))
                    })
                    .collect::<Result<Vec<_>, D::Error>>()?;

                verses_vec.sort_by_key(|(verse_num, _)| *verse_num);

                let verses = verses_vec
                    .into_iter()
                    .map(|(_, text)| text)
                    .collect::<Vec<_>>();

                Ok((chapter_num, verses))
            })
            .collect::<Result<Vec<_>, D::Error>>()
            .map(|mut chapters| {
                chapters.sort_by_key(|(chapter_num, _)| *chapter_num);
                chapters
                    .into_iter()
                    .map(|(_, verses)| verses)
                    .collect::<Vec<_>>()
            }),
    }
}

/// Represents the complete Bible with all books, chapters, and verses.
///
/// The Bible struct provides efficient access to any verse, chapter, or book
#[derive(Debug, Clone)]
pub struct Bible {
    books: Vec<Book>,
    index_by_abbrev: HashMap<String, usize>,
    reference_parser: ReferenceParser,

    /// Lazily constructed search index for verse lookups.
    search_index: Option<SearchIndex>,

    id: String,
    name: String,
    description: String,
    language: String,
}

impl Bible {
    fn from_initialization_data(data: BibleInitializationData) -> Self {
        let BibleInitializationData {
            books,
            search_index,
            id,
            name,
            description,
            language,
        } = data;

        let mut index_by_abbrev = HashMap::with_capacity(books.len());
        for (i, book) in books.iter().enumerate() {
            index_by_abbrev.insert(book.abbrev().to_ascii_lowercase(), i);
        }

        let reference_parser = build_reference_parser(&books, &language);

        Bible {
            books,
            index_by_abbrev,
            reference_parser,
            search_index,
            id,
            name,
            description,
            language,
        }
    }

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
        self.get_book_by_abbrev(book.abbreviation())
    }

    /// Returns a book by its abbreviation string.
    pub fn get_book_by_abbrev(&self, abbrev: &str) -> Result<&Book, BibleError> {
        let key = abbrev.to_ascii_lowercase();
        self.index_by_abbrev
            .get(key.as_str())
            .and_then(|&i| self.books.get(i))
            .ok_or_else(|| {
                let book_name = BibleBook::from_abbreviation(&key)
                    .map(|book| book.full_name().to_string())
                    .unwrap_or_else(|| key.clone());
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
    /// Parsing is provided by `bible-io-references`, including compact and
    /// common book abbreviations, localized names, Unicode syntax, and either
    /// spaced or adjacent coordinates. Ranges are rejected by this single-verse
    /// lookup API.
    pub fn get_verse_by_reference(&self, reference: &str) -> Result<&Verse, BibleError> {
        let reference = reference.trim();
        let parsed = self
            .reference_parser
            .parse_verse(reference)
            .map_err(|error| {
                if error.kind() == ParseErrorKind::UnknownBook {
                    let book_name = reference_book_token(reference);
                    BibleError::BookNotFound {
                        book_abbrev: book_name.to_ascii_lowercase(),
                        book_name: book_name.to_string(),
                        translation: self.name.clone(),
                    }
                } else {
                    BibleError::InvalidReference {
                        input: reference.to_string(),
                    }
                }
            })?;

        self.get_verse(
            parsed.book(),
            usize::from(parsed.chapter()),
            usize::from(parsed.verse()),
        )
    }

    /// Searches the Bible for verses containing all terms in the query.
    ///
    /// A tokenized search index is built on first use and reused on subsequent
    /// queries, providing fast lookups while returning cloned verse data for each match.
    pub fn search(&mut self, query: &str) -> Vec<Verse> {
        if query.is_empty() {
            return Vec::new();
        }

        if self.search_index.is_none() {
            let index = self.build_search_index();
            self.search_index = Some(index);
        }

        // Safe to unwrap: ensured Some above
        let matches = self.search_index.as_ref().unwrap().search(query);

        matches
            .into_iter()
            .filter_map(|(book, chapter, verse)| self.get_verse(book, chapter, verse).ok().cloned())
            .collect()
    }

    /// Builds a search index for faster repeated searches.
    pub fn build_search_index(&self) -> SearchIndex {
        let mut map: HashMap<String, Vec<(BibleBook, usize, usize)>> = HashMap::new();

        for book in &self.books {
            for chapter in book.chapters() {
                for verse in chapter.get_verses() {
                    for term in SearchIndex::tokenize(verse.text()) {
                        let entry = map.entry(term).or_default();
                        let tuple = (verse.book(), verse.chapter(), verse.number());
                        if !entry.contains(&tuple) {
                            entry.push(tuple);
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

    fn new_from_map_with_meta(
        map: IndexMap<String, FileDataEntry>,
        id: String,
        name: String,
        description: String,
        language: String,
    ) -> BibleInitializationData {
        // Iterate in map order (IndexMap preserves insertion order)
        let mut books = Vec::with_capacity(map.len());
        let mut search_index_map: HashMap<String, Vec<(BibleBook, usize, usize)>> = HashMap::new();

        for (abbrev, entry) in map.into_iter() {
            let book_enum = BibleBook::from_abbreviation(&abbrev).unwrap_or_else(|| {
                panic!(
                    "Unknown book abbreviation '{}' encountered while building Bible data",
                    abbrev
                )
            });

            let mut chapters = Vec::with_capacity(entry.chapters.len());

            for (chapter_idx, verses) in entry.chapters.into_iter().enumerate() {
                let chapter_number = chapter_idx + 1;
                let mut verses_vec = Vec::with_capacity(verses.len());

                for (verse_idx, verse_text) in verses.into_iter().enumerate() {
                    let verse_number = verse_idx + 1;
                    let tokens = SearchIndex::tokenize(&verse_text);
                    let mut seen_terms: HashSet<String> = HashSet::new();

                    for term in tokens {
                        if seen_terms.insert(term.clone()) {
                            let location = (book_enum, chapter_number, verse_number);
                            search_index_map.entry(term).or_default().push(location);
                        }
                    }

                    verses_vec.push(Verse::new(
                        book_enum,
                        chapter_number,
                        verse_number,
                        verse_text,
                    ));
                }

                chapters.push(Chapter::new(verses_vec, chapter_number));
            }

            books.push(Book::new(abbrev, entry.name, chapters));
        }

        for values in search_index_map.values_mut() {
            values.sort_by_key(|&(book, chapter, verse)| (book as usize, chapter, verse));
            values.dedup();
        }

        let search_index = if search_index_map.is_empty() {
            None
        } else {
            Some(SearchIndex::new(search_index_map))
        };

        BibleInitializationData {
            books,
            search_index,
            id,
            name,
            description,
            language,
        }
    }

    fn load_from_json(json_path: &str) -> Result<BibleInitializationData, Box<dyn Error>> {
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
    pub fn new(json_path: &str) -> Result<Self, Box<dyn Error>> {
        let initialization_data = Bible::load_from_json(json_path)?;
        Ok(Bible::from_initialization_data(initialization_data))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bible_books_enum::BibleBook;
    use std::collections::HashMap;

    fn create_test_bible() -> Bible {
        let verse = Verse::new(BibleBook::Genesis, 1, 1, "In the beginning".to_string());
        let chapter = Chapter::new(vec![verse], 1);
        let book = Book::new("GN".to_string(), "Genesis".to_string(), vec![chapter]);
        let mut index_by_abbrev = HashMap::new();
        index_by_abbrev.insert("gn".to_string(), 0);
        let books = vec![book];
        let reference_parser = build_reference_parser(&books, "English");

        Bible {
            books,
            index_by_abbrev,
            reference_parser,
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

    #[test]
    fn test_reference_parser_aliases() {
        let bible = create_test_bible();

        for &(alias, expected) in LEGACY_REFERENCE_ALIASES {
            let reference = format!("{alias} 1:1");
            let parsed = bible.reference_parser.parse_verse(&reference).unwrap();
            assert_eq!(parsed.book(), expected, "failed to parse {reference}");
        }

        for (reference, expected) in [
            ("Gen 1:1", BibleBook::Genesis),
            ("Rev 1:1", BibleBook::Revelation),
        ] {
            let parsed = bible.reference_parser.parse_verse(reference).unwrap();
            assert_eq!(parsed.book(), expected, "failed to parse {reference}");
        }
    }
}
