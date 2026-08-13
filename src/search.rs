//! Advanced, paginated Bible text search values.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use unicode_segmentation::UnicodeSegmentation;

use crate::{
    bible_books_enum::BibleBook,
    book::Book,
    errors::ModelError,
    text_search::{
        contains_normalized_text, find_normalized_substring_ranges, is_within_levenshtein_distance,
        normalized_range_to_source, tokenize_search_text, tokenize_search_text_with_ranges,
        uses_unspaced_word_boundaries,
    },
    verse::Verse,
};

/// Search matching policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchMode {
    /// Match any distinct query term.
    Any,
    /// Match every distinct query term.
    All,
    /// Match the exact phrase (or exact token sequence for whole-word mode).
    Exact,
}

/// Search-index construction and retention policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchIndexMode {
    /// Build the index during Bible construction.
    #[default]
    Eager,
    /// Build the index at first compatible search.
    Lazy,
    /// Never retain an index; scan the selected scope.
    Disabled,
}

/// Options for advanced and paginated searches.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SearchOptions {
    /// Matching policy.
    pub mode: SearchMode,
    /// Preserve case distinctions.
    pub case_sensitive: bool,
    /// Require complete Unicode tokens.
    pub whole_words: bool,
    /// Maximum page size, or no limit.
    pub max_results: Option<usize>,
    /// Number of matching verses to skip.
    pub offset: usize,
    /// Optional book scope.
    pub book: Option<BibleBook>,
    /// Optional chapter-number scope.
    pub chapter: Option<usize>,
    /// Optional verse-number scope.
    pub verse: Option<usize>,
    /// Canonically normalize Unicode.
    pub normalize_unicode: bool,
    /// Remove combining marks while matching.
    pub ignore_diacritics: bool,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            mode: SearchMode::Exact,
            case_sensitive: false,
            whole_words: false,
            max_results: None,
            offset: 0,
            book: None,
            chapter: None,
            verse: None,
            normalize_unicode: true,
            ignore_diacritics: false,
        }
    }
}

impl SearchOptions {
    /// Validate positive optional location filters.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.chapter == Some(0) {
            return Err("chapter must be positive");
        }
        if self.verse == Some(0) {
            return Err("verse must be positive");
        }
        Ok(())
    }
}

/// Source byte range with an inclusive start and exclusive end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TextRange {
    /// Inclusive byte offset.
    start: usize,
    /// Exclusive byte offset.
    end: usize,
}

impl TextRange {
    /// Construct a validated range.
    pub fn new(start: usize, end: usize) -> Result<Self, &'static str> {
        if end < start {
            return Err("range end must not precede start");
        }
        Ok(Self { start, end })
    }

    /// Return the byte length.
    #[must_use]
    pub const fn len(self) -> usize {
        self.end - self.start
    }

    /// Return the inclusive source byte offset.
    #[must_use]
    pub const fn start(self) -> usize {
        self.start
    }

    /// Return the exclusive source byte offset.
    #[must_use]
    pub const fn end(self) -> usize {
        self.end
    }

    /// Return whether the range is empty.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.start == self.end
    }

    /// Return whether the half-open range contains this byte offset.
    #[must_use]
    pub const fn contains(self, offset: usize) -> bool {
        offset >= self.start && offset < self.end
    }
}

/// Display-ready metadata for one matched verse.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SearchHit {
    /// Matched verse.
    verse: Verse,
    /// Loaded book display name.
    book_name: String,
    /// Display-ready reference.
    reference: String,
    /// Match ranges within the full verse text.
    match_ranges: Vec<TextRange>,
    /// Exact grapheme-safe slice of the verse text.
    snippet: String,
    /// Snippet start in the full verse text.
    snippet_start: usize,
    /// Snippet end in the full verse text.
    snippet_end: usize,
    /// Match ranges relative to the snippet.
    snippet_match_ranges: Vec<TextRange>,
}

impl SearchHit {
    /// Construct a validated hit from explicit snippet bounds. The snippet is
    /// always derived as an exact source slice, so all offsets remain usable
    /// for direct UTF-8 string slicing.
    pub fn new(
        verse: Verse,
        book: &Book,
        reference: Option<String>,
        match_ranges: Vec<TextRange>,
        snippet_start: usize,
        snippet_end: usize,
    ) -> Result<Self, ModelError> {
        validate_hit_inputs(&verse, book, &match_ranges)?;
        if snippet_start > snippet_end
            || snippet_end > verse.text().len()
            || !verse.text().is_char_boundary(snippet_start)
            || !verse.text().is_char_boundary(snippet_end)
        {
            return Err(ModelError::new(
                "snippet_bounds",
                "must be ordered UTF-8 boundaries inside the verse",
            ));
        }
        let snippet = verse.text()[snippet_start..snippet_end].to_string();
        let snippet_match_ranges = relative_match_ranges(&match_ranges, snippet_start, snippet_end);
        Ok(Self {
            reference: reference.unwrap_or_else(|| {
                format!("{} {}:{}", book.title(), verse.chapter(), verse.number())
            }),
            book_name: book.title().to_string(),
            verse,
            match_ranges,
            snippet,
            snippet_start,
            snippet_end,
            snippet_match_ranges,
        })
    }

    /// Create a validated hit with a grapheme-safe context window.
    pub fn with_context(
        verse: Verse,
        book: &Book,
        match_ranges: Vec<TextRange>,
        max_snippet_length: usize,
    ) -> Result<Self, ModelError> {
        if max_snippet_length == 0 {
            return Err(ModelError::new("max_snippet_length", "must be positive"));
        }
        validate_hit_inputs(&verse, book, &match_ranges)?;
        let (snippet_start, snippet_end) = context_bounds(
            verse.text(),
            match_ranges.first().copied(),
            max_snippet_length,
        );
        Self::new(verse, book, None, match_ranges, snippet_start, snippet_end)
    }

    /// Return the matched verse.
    #[must_use]
    pub fn verse(&self) -> &Verse {
        &self.verse
    }

    /// Return the loaded book display name.
    #[must_use]
    pub fn book_name(&self) -> &str {
        &self.book_name
    }

    /// Return the display-ready reference.
    #[must_use]
    pub fn reference(&self) -> &str {
        &self.reference
    }

    /// Return full-text match ranges.
    #[must_use]
    pub fn match_ranges(&self) -> &[TextRange] {
        &self.match_ranges
    }

    /// Return the exact snippet slice.
    #[must_use]
    pub fn snippet(&self) -> &str {
        &self.snippet
    }

    /// Return the snippet's full-text byte bounds.
    #[must_use]
    pub const fn snippet_bounds(&self) -> TextRange {
        TextRange {
            start: self.snippet_start,
            end: self.snippet_end,
        }
    }

    /// Return match ranges relative to the snippet.
    #[must_use]
    pub fn snippet_match_ranges(&self) -> &[TextRange] {
        &self.snippet_match_ranges
    }

    /// Return whether content was omitted before this snippet.
    #[must_use]
    pub const fn has_leading_omission(&self) -> bool {
        self.snippet_start > 0
    }

    /// Return whether content was omitted after this snippet.
    #[must_use]
    pub fn has_trailing_omission(&self) -> bool {
        self.snippet_end < self.verse.text().len()
    }
}

/// A validated page of matched verses and display metadata.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SearchResults {
    /// Original query.
    query: String,
    /// Returned verse values in edition order.
    verses: Vec<Verse>,
    /// UI-ready metadata in the same order as `verses`.
    hits: Vec<SearchHit>,
    /// Requested result offset.
    offset: usize,
    /// Requested result limit.
    limit: Option<usize>,
    /// Total matches when fully counted.
    total_count: Option<usize>,
    /// Whether at least one later result is known.
    has_more: bool,
}

impl SearchResults {
    /// Construct a validated compatibility page from verse values when no
    /// display-ready hit metadata is available.
    pub fn from_verses(
        query: impl Into<String>,
        verses: Vec<Verse>,
        offset: usize,
        limit: Option<usize>,
        total_count: Option<usize>,
        has_more: bool,
    ) -> Result<Self, ModelError> {
        validate_result_page(&verses, offset, limit, total_count, has_more)?;
        Ok(Self {
            query: query.into(),
            verses,
            hits: Vec::new(),
            offset,
            limit,
            total_count,
            has_more,
        })
    }

    /// Construct a validated result page from display-ready hits.
    pub fn from_hits(
        query: impl Into<String>,
        hits: Vec<SearchHit>,
        offset: usize,
        limit: Option<usize>,
        total_count: Option<usize>,
        has_more: bool,
    ) -> Result<Self, ModelError> {
        let verses: Vec<_> = hits.iter().map(|hit| hit.verse.clone()).collect();
        validate_result_page(&verses, offset, limit, total_count, has_more)?;
        Ok(Self {
            query: query.into(),
            verses,
            hits,
            offset,
            limit,
            total_count,
            has_more,
        })
    }

    /// Return the original query.
    #[must_use]
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Return the verses on this page.
    #[must_use]
    pub fn verses(&self) -> &[Verse] {
        &self.verses
    }

    /// Consume this page and return its verses.
    #[must_use]
    pub fn into_verses(self) -> Vec<Verse> {
        self.verses
    }

    /// Return display-ready hits.
    #[must_use]
    pub fn hits(&self) -> &[SearchHit] {
        &self.hits
    }

    /// Return this page's zero-based offset.
    #[must_use]
    pub const fn offset(&self) -> usize {
        self.offset
    }

    /// Return the requested page size.
    #[must_use]
    pub const fn limit(&self) -> Option<usize> {
        self.limit
    }

    /// Return the known total result count.
    #[must_use]
    pub const fn total_count(&self) -> Option<usize> {
        self.total_count
    }

    /// Return whether a later result is known.
    #[must_use]
    pub const fn has_more(&self) -> bool {
        self.has_more
    }

    /// Return the number of results on this page.
    #[must_use]
    pub fn count(&self) -> usize {
        self.verses.len()
    }

    /// Return whether this page is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.verses.is_empty()
    }

    /// Return whether this page contains at least one verse.
    #[must_use]
    pub fn is_not_empty(&self) -> bool {
        !self.is_empty()
    }

    /// Return whether this is not the first possible page.
    #[must_use]
    pub fn has_previous(&self) -> bool {
        self.offset > 0 && self.total_count != Some(0)
    }

    /// Return the next page offset when a non-empty later page is known.
    #[must_use]
    pub fn next_offset(&self) -> Option<usize> {
        (self.has_more && !self.verses.is_empty())
            .then(|| self.offset.checked_add(self.verses.len()))
            .flatten()
    }

    /// Group returned verses by book.
    #[must_use]
    pub fn by_book(&self) -> HashMap<BibleBook, Vec<&Verse>> {
        let mut grouped = HashMap::new();
        for verse in &self.verses {
            grouped
                .entry(verse.book())
                .or_insert_with(Vec::new)
                .push(verse);
        }
        grouped
    }

    /// Group returned verses by canonical English chapter label.
    #[must_use]
    pub fn by_chapter(&self) -> HashMap<String, Vec<&Verse>> {
        let mut grouped = HashMap::new();
        for verse in &self.verses {
            let key = format!("{} {}", verse.book().full_name(), verse.chapter());
            grouped.entry(key).or_insert_with(Vec::new).push(verse);
        }
        grouped
    }

    /// Group returned verses by stable, language-neutral chapter identity.
    #[must_use]
    pub fn by_chapter_location(&self) -> HashMap<(BibleBook, usize), Vec<&Verse>> {
        let mut grouped = HashMap::new();
        for verse in &self.verses {
            grouped
                .entry((verse.book(), verse.chapter()))
                .or_insert_with(Vec::new)
                .push(verse);
        }
        grouped
    }

    /// Group returned verses by their loaded or localized chapter label.
    #[must_use]
    pub fn by_display_chapter(&self) -> HashMap<String, Vec<&Verse>> {
        let names: HashMap<_, _> = self
            .hits
            .iter()
            .map(|hit| (hit.verse.book(), hit.book_name.as_str()))
            .collect();
        let mut grouped = HashMap::new();
        for verse in &self.verses {
            let name = names
                .get(&verse.book())
                .copied()
                .unwrap_or_else(|| verse.book().full_name());
            let key = format!("{name} {}", verse.chapter());
            grouped.entry(key).or_insert_with(Vec::new).push(verse);
        }
        grouped
    }
}

fn validate_hit_inputs(
    verse: &Verse,
    book: &Book,
    match_ranges: &[TextRange],
) -> Result<(), ModelError> {
    if book.book() != verse.book() || book.get_verse(verse.chapter(), verse.number()) != Ok(verse) {
        return Err(ModelError::new(
            "book",
            "must contain the matched verse value",
        ));
    }
    let mut previous_end = 0;
    for range in match_ranges {
        if range.is_empty()
            || range.end > verse.text().len()
            || !verse.text().is_char_boundary(range.start)
            || !verse.text().is_char_boundary(range.end)
            || range.start < previous_end
        {
            return Err(ModelError::new(
                "match_ranges",
                "must be sorted, non-empty, non-overlapping UTF-8 ranges inside the verse",
            ));
        }
        previous_end = range.end;
    }
    Ok(())
}

fn relative_match_ranges(
    match_ranges: &[TextRange],
    snippet_start: usize,
    snippet_end: usize,
) -> Vec<TextRange> {
    match_ranges
        .iter()
        .filter_map(|range| {
            let start = range.start.max(snippet_start);
            let end = range.end.min(snippet_end);
            (start < end).then_some(TextRange {
                start: start - snippet_start,
                end: end - snippet_start,
            })
        })
        .collect()
}

fn validate_result_page(
    verses: &[Verse],
    offset: usize,
    limit: Option<usize>,
    total_count: Option<usize>,
    has_more: bool,
) -> Result<(), ModelError> {
    if limit.is_some_and(|limit| verses.len() > limit) {
        return Err(ModelError::new("verses", "page exceeds its result limit"));
    }
    let page_end = offset
        .checked_add(verses.len())
        .ok_or_else(|| ModelError::new("offset", "offset and page length must not overflow"))?;
    if let Some(total) = total_count {
        if total < verses.len() || (!verses.is_empty() && page_end > total) {
            return Err(ModelError::new(
                "total_count",
                "must include the returned page",
            ));
        }
        if has_more != (page_end < total) {
            return Err(ModelError::new(
                "has_more",
                "must agree with offset, count, and total_count",
            ));
        }
    }
    let mut locations = HashSet::new();
    for verse in verses {
        if !locations.insert((verse.book(), verse.chapter(), verse.number())) {
            return Err(ModelError::new(
                "verses",
                "must not contain duplicate verse locations",
            ));
        }
    }
    Ok(())
}

pub(crate) fn matches_search_text(text: &str, query: &str, options: &SearchOptions) -> bool {
    match options.mode {
        SearchMode::Exact if !options.whole_words => contains_normalized_text(
            text,
            query,
            options.case_sensitive,
            options.normalize_unicode,
            options.ignore_diacritics,
        ),
        SearchMode::Exact => {
            let content = tokenize_search_text(
                text,
                options.case_sensitive,
                options.normalize_unicode,
                options.ignore_diacritics,
            );
            let query = tokenize_search_text(
                query,
                options.case_sensitive,
                options.normalize_unicode,
                options.ignore_diacritics,
            );
            !query.is_empty() && content.windows(query.len()).any(|window| window == query)
        }
        SearchMode::All | SearchMode::Any => {
            let content = tokenize_search_text(
                text,
                options.case_sensitive,
                options.normalize_unicode,
                options.ignore_diacritics,
            );
            let query: HashSet<_> = tokenize_search_text(
                query,
                options.case_sensitive,
                options.normalize_unicode,
                options.ignore_diacritics,
            )
            .into_iter()
            .collect();
            if query.is_empty() {
                return false;
            }
            let matches = |query: &String| {
                content.iter().any(|content| {
                    content == query
                        || (!options.whole_words
                            && uses_unspaced_word_boundaries(query)
                            && content.contains(query))
                })
            };
            if options.mode == SearchMode::All {
                query.iter().all(matches)
            } else {
                query.iter().any(matches)
            }
        }
    }
}

pub(crate) fn find_match_ranges(
    text: &str,
    query: &str,
    options: &SearchOptions,
) -> Vec<TextRange> {
    if options.mode == SearchMode::Exact && !options.whole_words {
        return find_normalized_substring_ranges(
            text,
            query,
            options.case_sensitive,
            options.normalize_unicode,
            options.ignore_diacritics,
        )
        .into_iter()
        .map(|range| TextRange {
            start: range.start,
            end: range.end,
        })
        .collect();
    }
    let query_tokens = tokenize_search_text(
        query,
        options.case_sensitive,
        options.normalize_unicode,
        options.ignore_diacritics,
    );
    if query_tokens.is_empty() {
        return Vec::new();
    }
    let content_tokens = tokenize_search_text_with_ranges(
        text,
        options.case_sensitive,
        options.normalize_unicode,
        options.ignore_diacritics,
    );
    if options.mode == SearchMode::Exact {
        return content_tokens
            .windows(query_tokens.len())
            .filter(|window| {
                window
                    .iter()
                    .zip(&query_tokens)
                    .all(|(content, query)| content.normalized == *query)
            })
            .map(|window| TextRange {
                start: window[0].start,
                end: window[window.len() - 1].end,
            })
            .collect();
    }
    let query_tokens: HashSet<_> = query_tokens.into_iter().collect();
    let mut ranges = Vec::new();
    for content in content_tokens {
        for query in &query_tokens {
            if content.normalized == *query {
                ranges.push(TextRange {
                    start: content.start,
                    end: content.end,
                });
            } else if !options.whole_words
                && uses_unspaced_word_boundaries(query)
                && content.normalized.contains(query)
            {
                ranges.extend(
                    find_normalized_substring_ranges(
                        &content.raw,
                        query,
                        options.case_sensitive,
                        options.normalize_unicode,
                        options.ignore_diacritics,
                    )
                    .into_iter()
                    .map(|range| TextRange {
                        start: content.start + range.start,
                        end: content.start + range.end,
                    }),
                );
            }
        }
    }
    merge_ranges(ranges)
}

pub(crate) fn fuzzy_matches(
    text: &str,
    query: &str,
    options: &SearchOptions,
    max_distance: usize,
) -> bool {
    fuzzy_match_ranges(text, query, options, max_distance).is_some()
}

pub(crate) fn fuzzy_match_ranges(
    text: &str,
    query: &str,
    options: &SearchOptions,
    max_distance: usize,
) -> Option<Vec<TextRange>> {
    let content = tokenize_search_text_with_ranges(
        text,
        options.case_sensitive,
        options.normalize_unicode,
        options.ignore_diacritics,
    );
    let query = tokenize_search_text(
        query,
        options.case_sensitive,
        options.normalize_unicode,
        options.ignore_diacritics,
    );
    if content.is_empty() || query.is_empty() {
        return None;
    }
    match options.mode {
        SearchMode::Any => {
            let mut ranges = Vec::new();
            for token in &content {
                for query in &query {
                    if let Some(range) =
                        fuzzy_token_match_range(token, query, max_distance, options)
                    {
                        ranges.push(range);
                    }
                }
            }
            (!ranges.is_empty()).then(|| merge_ranges(ranges))
        }
        SearchMode::All => {
            let mut ranges = Vec::new();
            for query in query.iter().collect::<HashSet<_>>() {
                let range = content.iter().find_map(|token| {
                    fuzzy_token_match_range(token, query, max_distance, options)
                })?;
                ranges.push(range);
            }
            Some(merge_ranges(ranges))
        }
        SearchMode::Exact => {
            let mut ranges = Vec::new();
            for window in content.windows(query.len()) {
                let matched = window
                    .iter()
                    .zip(&query)
                    .map(|(content, query)| {
                        fuzzy_token_match_range(content, query, max_distance, options)
                    })
                    .collect::<Option<Vec<_>>>();
                if let Some(matched) = matched {
                    ranges.push(TextRange {
                        start: matched[0].start,
                        end: matched[matched.len() - 1].end,
                    });
                }
            }
            (!ranges.is_empty()).then(|| merge_ranges(ranges))
        }
    }
}

fn fuzzy_token_match_range(
    content: &crate::text_search::SearchTextToken,
    query: &str,
    max_distance: usize,
    options: &SearchOptions,
) -> Option<TextRange> {
    if !options.whole_words && uses_unspaced_word_boundaries(query) {
        return fuzzy_unspaced_substring_range(content, query, max_distance, options);
    }
    is_within_levenshtein_distance(&content.normalized, query, max_distance).then_some(TextRange {
        start: content.start,
        end: content.end,
    })
}

fn fuzzy_unspaced_substring_range(
    content: &crate::text_search::SearchTextToken,
    query: &str,
    max_distance: usize,
    options: &SearchOptions,
) -> Option<TextRange> {
    let normalized: Vec<_> = content.normalized.chars().collect();
    let query_length = query.chars().count();
    let minimum = query_length.saturating_sub(max_distance).max(1);
    let maximum = query_length
        .saturating_add(max_distance)
        .min(normalized.len());
    if minimum > maximum {
        return None;
    }

    let mut character_boundaries = content
        .normalized
        .char_indices()
        .map(|(offset, _)| offset)
        .collect::<Vec<_>>();
    character_boundaries.push(content.normalized.len());

    let mut best: Option<(usize, usize, usize, usize)> = None;
    for length in minimum..=maximum {
        for start in 0..=normalized.len() - length {
            let end = start + length;
            let candidate: String = normalized[start..end].iter().collect();
            if !is_within_levenshtein_distance(&candidate, query, max_distance) {
                continue;
            }
            let distance = levenshtein_distance_with_limit(&candidate, query, max_distance);
            let length_difference = length.abs_diff(query_length);
            let should_replace = best.is_none_or(
                |(best_distance, best_length_difference, best_start, best_end)| {
                    (distance, length_difference, start, end)
                        < (best_distance, best_length_difference, best_start, best_end)
                },
            );
            if should_replace {
                best = Some((distance, length_difference, start, end));
            }
        }
    }
    let (_, _, start, end) = best?;
    let normalized_start = character_boundaries[start];
    let normalized_end = character_boundaries[end];
    let range = normalized_range_to_source(
        &content.raw,
        normalized_start..normalized_end,
        options.case_sensitive,
        options.normalize_unicode,
        options.ignore_diacritics,
    )?;
    Some(TextRange {
        start: content.start + range.start,
        end: content.start + range.end,
    })
}

fn levenshtein_distance_with_limit(first: &str, second: &str, limit: usize) -> usize {
    let first: Vec<_> = first.chars().collect();
    let second: Vec<_> = second.chars().collect();
    let mut previous: Vec<usize> = (0..=second.len()).collect();
    let mut current = vec![0; second.len() + 1];
    for (row, first_character) in first.iter().enumerate() {
        current[0] = row + 1;
        for (column, second_character) in second.iter().enumerate() {
            current[column + 1] = (previous[column + 1] + 1)
                .min(current[column] + 1)
                .min(previous[column] + usize::from(first_character != second_character));
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[second.len()].min(limit.saturating_add(1))
}

fn merge_ranges(mut ranges: Vec<TextRange>) -> Vec<TextRange> {
    if ranges.is_empty() {
        return ranges;
    }
    ranges.sort_by_key(|range| (range.start, range.end));
    let mut merged = Vec::new();
    let mut current = ranges[0];
    for next in ranges.into_iter().skip(1) {
        if next.start <= current.end {
            current.end = current.end.max(next.end);
        } else {
            merged.push(current);
            current = next;
        }
    }
    merged.push(current);
    merged
}

fn context_bounds(text: &str, first: Option<TextRange>, maximum: usize) -> (usize, usize) {
    if text.len() <= maximum {
        return (0, text.len());
    }
    let match_length = first.map_or(0, TextRange::len);
    let desired_length = maximum.max(match_length).min(text.len());
    let center = first.map_or(0, |range| (range.start + range.end) / 2);
    let desired_start = center
        .saturating_sub(desired_length / 2)
        .min(text.len() - desired_length);
    let desired_end = desired_start + desired_length;
    let boundaries: Vec<_> = UnicodeSegmentation::grapheme_indices(text, true)
        .map(|(offset, _)| offset)
        .chain(std::iter::once(text.len()))
        .collect();
    let start = boundaries
        .iter()
        .copied()
        .take_while(|offset| *offset <= desired_start)
        .last()
        .unwrap_or(0);
    let end = boundaries
        .iter()
        .copied()
        .find(|offset| *offset >= desired_end)
        .unwrap_or(text.len());
    (start, end)
}
