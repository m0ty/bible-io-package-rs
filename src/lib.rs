//! A Rust library for working with Bible text data structures
//!
//! This library provides structures and functionality for parsing and working with Bible text data,
//! including books, chapters, and verses.

pub mod bible_books_enum;
pub mod bible_class;

// Re-export main types for easier access
pub use bible_books_enum::BibleBook;
pub use bible_class::{Bible, Book, Chapter, Verse};
