use std::collections::HashMap;

use crate::bible_books_enum::BibleBook;

/// Search index mapping normalized terms to verse locations.
#[derive(Debug, Default, Clone)]
pub struct SearchIndex {
    index: HashMap<String, Vec<(BibleBook, usize, usize)>>,
}

impl SearchIndex {
    /// Create a new search index from a map.
    pub fn new(index: HashMap<String, Vec<(BibleBook, usize, usize)>>) -> Self {
        SearchIndex { index }
    }

    /// Breaks a text into normalized lowercase terms.
    pub(crate) fn tokenize(text: &str) -> Vec<String> {
        text.split(|c: char| !c.is_ascii_alphanumeric())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_ascii_lowercase())
            .collect()
    }

    /// Searches for verses containing all terms in the query.
    pub fn search(&self, query: &str) -> Vec<(BibleBook, usize, usize)> {
        let terms = Self::tokenize(query);
        if terms.is_empty() {
            return Vec::new();
        }

        let mut iter = terms.into_iter();
        let first = iter.next().unwrap();
        let mut results = match self.index.get(&first) {
            Some(v) => v.clone(),
            None => return Vec::new(),
        };

        for term in iter {
            if let Some(list) = self.index.get(&term) {
                results.retain(|item| list.contains(item));
            } else {
                return Vec::new();
            }
        }

        results.sort_by_key(|&(b, c, v)| (b as usize, c, v));
        results.dedup();
        results
    }
}
