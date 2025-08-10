//! A Rust library for working with Bible text data structures
//! 
//! This library provides structures and functionality for parsing and working with Bible text data,
//! including books, chapters, and verses.

pub mod bible_class;
pub mod bible_books_enum;

// Re-export main types for easier access
pub use bible_class::{Bible, Book, Chapter, Verse};
pub use bible_books_enum::BibleBook;
