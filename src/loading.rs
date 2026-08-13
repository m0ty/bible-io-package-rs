//! Bible loading, validation, and search-index options.

use serde::{Deserialize, Serialize};

use crate::errors::ModelError;

/// Current version of the serialized Bible content contract.
pub const CURRENT_BIBLE_SCHEMA_VERSION: u32 = 1;

/// Stable phases reported while loading a Bible from external storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BibleLoadPhase {
    /// Reading bytes from storage.
    Reading,
    /// Decoding and validating the content model.
    Processing,
    /// The Bible is ready for use.
    Complete,
}

/// Snapshot delivered to a load-progress callback.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BibleLoadProgress {
    /// Current phase.
    pub phase: BibleLoadPhase,
    /// Overall completion fraction in the inclusive range `0.0..=1.0`.
    pub fraction: f32,
    /// Completion fraction within the current phase.
    pub phase_fraction: f32,
}

impl BibleLoadProgress {
    /// Construct a validated progress snapshot.
    pub fn new(
        phase: BibleLoadPhase,
        fraction: f32,
        phase_fraction: f32,
    ) -> Result<Self, ModelError> {
        if !fraction.is_finite() || !(0.0..=1.0).contains(&fraction) {
            return Err(ModelError::new(
                "fraction",
                "must be finite and between 0 and 1",
            ));
        }
        if !phase_fraction.is_finite() || !(0.0..=1.0).contains(&phase_fraction) {
            return Err(ModelError::new(
                "phase_fraction",
                "must be finite and between 0 and 1",
            ));
        }
        Ok(Self {
            phase,
            fraction,
            phase_fraction,
        })
    }
}

/// Strictness controls for decoded Bible content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BibleDataValidationOptions {
    /// Require at least one book.
    pub require_books: bool,
    /// Require at least one chapter per book.
    pub require_chapters: bool,
    /// Require at least one verse per chapter.
    pub require_verses: bool,
    /// Require every verse text to contain a non-whitespace character.
    pub require_verse_text: bool,
}

impl BibleDataValidationOptions {
    /// Compatibility policy for intentionally skeletal data.
    pub const PERMISSIVE: Self = Self {
        require_books: false,
        require_chapters: false,
        require_verses: false,
        require_verse_text: false,
    };
}

impl Default for BibleDataValidationOptions {
    fn default() -> Self {
        Self {
            require_books: true,
            require_chapters: true,
            require_verses: true,
            require_verse_text: true,
        }
    }
}

/// Options shared by all Bible construction methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BibleLoadOptions {
    /// Content presence requirements.
    pub validation: BibleDataValidationOptions,
    /// Search index construction policy.
    pub search_index_mode: crate::search::SearchIndexMode,
}
