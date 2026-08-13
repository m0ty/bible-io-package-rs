//! A reusable inverted index for default all-term Bible search.

use std::collections::{HashMap, HashSet};

use crate::{
    bible_books_enum::BibleBook,
    text_search::{search_index_lookup_key, tokenize_search_text},
};

/// One indexed verse coordinate.
pub type VerseLocationTuple = (BibleBook, usize, usize);

/// Search index mapping normalized terms to verse locations.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SearchIndex {
    index: HashMap<String, Vec<VerseLocationTuple>>,
}

impl SearchIndex {
    /// Create a new search index from a posting map.
    #[must_use]
    pub fn new(index: HashMap<String, Vec<VerseLocationTuple>>) -> Self {
        Self { index }
    }

    /// Return the number of distinct normalized terms.
    #[must_use]
    pub fn len(&self) -> usize {
        self.index.len()
    }

    /// Return whether this index contains no terms.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }

    /// Return the total number of verse postings.
    #[must_use]
    pub fn posting_count(&self) -> usize {
        self.index.values().map(Vec::len).sum()
    }

    /// Break text into normalized Unicode terms.
    pub(crate) fn tokenize(text: &str) -> Vec<String> {
        tokenize_search_text(text, false, true, false)
    }

    /// Search for locations containing all distinct query terms.
    #[must_use]
    pub fn search(&self, query: &str) -> Vec<VerseLocationTuple> {
        let terms: HashSet<_> = Self::tokenize(query)
            .into_iter()
            .map(|term| search_index_lookup_key(&term, 3))
            .collect();
        if terms.is_empty() {
            return Vec::new();
        }
        let mut postings: Vec<_> = terms.iter().map(|term| self.index.get(term)).collect();
        if postings.iter().any(|posting| posting.is_none()) {
            return Vec::new();
        }
        postings.sort_by_key(|posting| posting.map_or(usize::MAX, Vec::len));
        let first = postings[0].expect("missing postings returned above");
        let other: Vec<HashSet<_>> = postings[1..]
            .iter()
            .map(|posting| {
                posting
                    .expect("missing postings returned above")
                    .iter()
                    .copied()
                    .collect()
            })
            .collect();
        first
            .iter()
            .copied()
            .filter(|location| other.iter().all(|posting| posting.contains(location)))
            .collect()
    }
}
