//! Unicode-aware normalization, tokenization, and bounded fuzzy matching.

use std::{collections::HashSet, ops::Range};

use unicode_normalization::{char::is_combining_mark, UnicodeNormalization};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug)]
struct MappedSearchText {
    text: String,
    byte_starts: Vec<usize>,
    byte_ends: Vec<usize>,
}

/// A normalized Unicode token with byte offsets into the original text.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SearchTextToken {
    /// Original token text.
    pub raw: String,
    /// Normalized token used for matching.
    pub normalized: String,
    /// Inclusive source byte offset.
    pub start: usize,
    /// Exclusive source byte offset.
    pub end: usize,
}

/// Normalize text for searching while preserving meaningful marks by default.
#[must_use]
pub fn normalize_search_text(
    text: &str,
    case_sensitive: bool,
    normalize_unicode: bool,
    ignore_diacritics: bool,
) -> String {
    let mut normalized = if normalize_unicode || ignore_diacritics {
        text.nfc().collect::<String>()
    } else {
        text.to_string()
    };
    if !case_sensitive {
        normalized = normalized.to_lowercase();
        normalized = normalized
            .replace('ß', "ss")
            .replace('ς', "σ")
            .replace('ſ', "s");
    }
    if ignore_diacritics {
        normalized = normalized
            .nfd()
            .filter(|character| !is_combining_mark(*character))
            .collect::<String>()
            .nfc()
            .collect();
    } else if normalize_unicode {
        normalized = normalized.nfc().collect();
    }
    normalized
}

fn is_search_character(character: char) -> bool {
    character.is_alphanumeric() || is_combining_mark(character)
}

/// Tokenize Unicode letters, marks, and numbers.
#[must_use]
pub fn tokenize_search_text(
    text: &str,
    case_sensitive: bool,
    normalize_unicode: bool,
    ignore_diacritics: bool,
) -> Vec<String> {
    tokenize_search_text_with_ranges(text, case_sensitive, normalize_unicode, ignore_diacritics)
        .into_iter()
        .map(|token| token.normalized)
        .collect()
}

/// Tokenize Unicode text and retain byte offsets into the original string.
#[must_use]
pub fn tokenize_search_text_with_ranges(
    text: &str,
    case_sensitive: bool,
    normalize_unicode: bool,
    ignore_diacritics: bool,
) -> Vec<SearchTextToken> {
    let mut tokens = Vec::new();
    let mut start = None;
    for (offset, character) in text.char_indices() {
        if is_search_character(character) {
            start.get_or_insert(offset);
        } else if let Some(token_start) = start.take() {
            let raw = &text[token_start..offset];
            let normalized =
                normalize_search_text(raw, case_sensitive, normalize_unicode, ignore_diacritics);
            if !normalized.is_empty() {
                tokens.push(SearchTextToken {
                    raw: raw.to_string(),
                    normalized,
                    start: token_start,
                    end: offset,
                });
            }
        }
    }
    if let Some(token_start) = start {
        let raw = &text[token_start..];
        let normalized =
            normalize_search_text(raw, case_sensitive, normalize_unicode, ignore_diacritics);
        if !normalized.is_empty() {
            tokens.push(SearchTextToken {
                raw: raw.to_string(),
                normalized,
                start: token_start,
                end: text.len(),
            });
        }
    }
    tokens
}

/// Extract original Unicode words for display and statistics.
#[must_use]
pub fn extract_unicode_words(text: &str) -> Vec<&str> {
    let mut words = Vec::new();
    let mut start = None;
    for (offset, character) in text.char_indices() {
        if is_search_character(character) {
            start.get_or_insert(offset);
        } else if let Some(token_start) = start.take() {
            words.push(&text[token_start..offset]);
        }
    }
    if let Some(token_start) = start {
        words.push(&text[token_start..]);
    }
    words
}

/// Return whether normalized `text` contains normalized non-empty `query`.
#[must_use]
pub fn contains_normalized_text(
    text: &str,
    query: &str,
    case_sensitive: bool,
    normalize_unicode: bool,
    ignore_diacritics: bool,
) -> bool {
    let query = normalize_search_text(query, case_sensitive, normalize_unicode, ignore_diacritics);
    !query.is_empty()
        && normalize_search_text(text, case_sensitive, normalize_unicode, ignore_diacritics)
            .contains(&query)
}

/// Return whether a token uses a script that commonly omits spaces.
#[must_use]
pub fn uses_unspaced_word_boundaries(token: &str) -> bool {
    token.chars().any(|character| {
        let value = character as u32;
        matches!(
            value,
            0x3400..=0x4dbf
                | 0x4e00..=0x9fff
                | 0xf900..=0xfaff
                | 0x20000..=0x323af
                | 0x3040..=0x309f
                | 0x30a0..=0x30ff
                | 0x31f0..=0x31ff
                | 0xff66..=0xff9d
                | 0x0e00..=0x0e7f
                | 0x0e80..=0x0eff
                | 0x1000..=0x109f
                | 0xa9e0..=0xa9ff
                | 0xaa60..=0xaa7f
                | 0x1780..=0x17ff
        )
    })
}

/// Build full-token and short scalar n-gram terms for the default index.
#[must_use]
pub fn build_search_index_terms(text: &str, max_ngram_length: usize) -> HashSet<String> {
    assert!(max_ngram_length > 0, "max_ngram_length must be positive");
    let mut terms = HashSet::new();
    for token in tokenize_search_text(text, false, true, false) {
        terms.insert(token.clone());
        if !uses_unspaced_word_boundaries(&token) {
            continue;
        }
        let characters: Vec<_> = token.chars().collect();
        for length in 1..=max_ngram_length.min(characters.len()) {
            for start in 0..=characters.len() - length {
                terms.insert(characters[start..start + length].iter().collect());
            }
        }
    }
    terms
}

/// Select a key guaranteed to exist for an indexed unspaced-script substring.
#[must_use]
pub fn search_index_lookup_key(token: &str, max_ngram_length: usize) -> String {
    assert!(max_ngram_length > 0, "max_ngram_length must be positive");
    if !uses_unspaced_word_boundaries(token) {
        return token.to_string();
    }
    token.chars().take(max_ngram_length).collect()
}

/// Find normalized substring matches and map them to original UTF-8 byte ranges.
#[must_use]
pub fn find_normalized_substring_ranges(
    text: &str,
    query: &str,
    case_sensitive: bool,
    normalize_unicode: bool,
    ignore_diacritics: bool,
) -> Vec<Range<usize>> {
    let query = normalize_search_text(query, case_sensitive, normalize_unicode, ignore_diacritics);
    if query.is_empty() {
        return Vec::new();
    }
    let mapped =
        normalize_with_source_mapping(text, case_sensitive, normalize_unicode, ignore_diacritics);
    let mut ranges = Vec::new();
    let mut search_from = 0;
    while search_from <= mapped.text.len().saturating_sub(query.len()) {
        let Some(relative) = mapped.text[search_from..].find(&query) else {
            break;
        };
        let start = search_from + relative;
        let end = start + query.len();
        if let Some(range) = mapped.source_range(start..end) {
            if ranges.last() != Some(&range) {
                ranges.push(range);
            }
        }
        search_from = end;
    }
    ranges
}

/// Map a non-empty byte range in normalized text back to the original source.
pub(crate) fn normalized_range_to_source(
    source: &str,
    normalized_range: Range<usize>,
    case_sensitive: bool,
    normalize_unicode: bool,
    ignore_diacritics: bool,
) -> Option<Range<usize>> {
    normalize_with_source_mapping(source, case_sensitive, normalize_unicode, ignore_diacritics)
        .source_range(normalized_range)
}

fn normalize_with_source_mapping(
    source: &str,
    case_sensitive: bool,
    normalize_unicode: bool,
    ignore_diacritics: bool,
) -> MappedSearchText {
    let mut normalized = String::new();
    let mut starts = Vec::new();
    let mut ends = Vec::new();
    for (start, grapheme) in source.grapheme_indices(true) {
        let end = start + grapheme.len();
        let mapped = normalize_search_text(
            grapheme,
            case_sensitive,
            normalize_unicode,
            ignore_diacritics,
        );
        starts.extend(std::iter::repeat_n(start, mapped.len()));
        ends.extend(std::iter::repeat_n(end, mapped.len()));
        normalized.push_str(&mapped);
    }
    MappedSearchText {
        text: normalized,
        byte_starts: starts,
        byte_ends: ends,
    }
}

impl MappedSearchText {
    fn source_range(&self, normalized: Range<usize>) -> Option<Range<usize>> {
        if normalized.start >= normalized.end
            || normalized.end > self.text.len()
            || !self.text.is_char_boundary(normalized.start)
            || !self.text.is_char_boundary(normalized.end)
        {
            return None;
        }
        Some(self.byte_starts[normalized.start]..self.byte_ends[normalized.end - 1])
    }
}

/// Return whether two strings have at most `max_distance` Unicode-scalar edits.
#[must_use]
pub fn is_within_levenshtein_distance(first: &str, second: &str, max_distance: usize) -> bool {
    let first: Vec<char> = first.chars().collect();
    let second: Vec<char> = second.chars().collect();
    if first.len().abs_diff(second.len()) > max_distance {
        return false;
    }
    let (rows, columns) = if first.len() >= second.len() {
        (&first, &second)
    } else {
        (&second, &first)
    };
    let sentinel = max_distance.saturating_add(1);
    let mut previous: Vec<usize> = (0..=columns.len()).collect();
    let mut current = vec![sentinel; columns.len() + 1];
    for (row, row_character) in rows.iter().enumerate() {
        let row_number = row + 1;
        current[0] = row_number;
        let mut minimum = current[0];
        for (column, column_character) in columns.iter().enumerate() {
            let column_number = column + 1;
            current[column_number] = (previous[column_number] + 1)
                .min(current[column] + 1)
                .min(previous[column] + usize::from(row_character != column_character));
            minimum = minimum.min(current[column_number]);
        }
        if minimum > max_distance {
            return false;
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[columns.len()] <= max_distance
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalization_and_unicode_tokens_match_dart_contract() {
        assert!(contains_normalized_text(
            "Cafe\u{301}",
            "CAFÉ",
            false,
            true,
            false
        ));
        assert_eq!(
            tokenize_search_text("שלום κόσμος 123", false, true, false).len(),
            3
        );
        assert!(is_within_levenshtein_distance("κόσμος", "κόσμοσ", 1));
        assert!(uses_unspaced_word_boundaries("创造"));
    }
}
