//! A Rust library for working with Bible text data structures
//!
//! This library provides structures and functionality for parsing and working with Bible text data,
//! including books, chapters, and verses.

pub mod bible;
pub mod bible_books_enum;
pub mod book;
pub mod chapter;
pub mod search_index;
pub mod verse;

// Expose the shared reference package and retain the established BibleBook name.
pub use bible_io_references;

// Re-export main types for easier access
pub use bible::{Bible, BibleError};
pub use bible_books_enum::BibleBook;
pub use book::Book;
pub use chapter::Chapter;
pub use search_index::SearchIndex;
pub use verse::Verse;
