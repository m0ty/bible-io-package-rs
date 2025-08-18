use std::fmt;

use crate::verse::Verse;

/// Represents a chapter from a Bible book.
///
/// A chapter contains multiple verses and has a chapter number.
#[derive(Debug, Clone)]
pub struct Chapter {
    verses: Vec<Verse>,
    chapter_number: usize,
}

impl Chapter {
    /// Creates a new chapter with the given verses and chapter number.
    ///
    /// # Arguments
    ///
    /// * `verses` - A vector of verses in this chapter
    /// * `chapter_number` - The chapter number within the book
    pub fn new(verses: Vec<Verse>, chapter_number: usize) -> Self {
        Chapter {
            verses,
            chapter_number,
        }
    }

    /// Returns this chapter's number within its book.
    pub fn number(&self) -> usize {
        self.chapter_number
    }

    /// Returns a slice of all verses in this chapter.
    ///
    /// # Returns
    ///
    /// A slice containing the verses in this chapter.
    pub fn get_verses(&self) -> &[Verse] {
        &self.verses
    }

    /// Returns a specific verse by its verse number.
    ///
    /// # Arguments
    ///
    /// * `verse_number` - The verse number to retrieve
    ///
    /// # Returns
    ///
    /// An optional reference to the verse if found, None otherwise.
    pub fn get_verse(&self, verse_number: usize) -> Option<&Verse> {
        if verse_number == 0 {
            return None;
        }
        self.verses.get(verse_number - 1)
    }
}

impl fmt::Display for Chapter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let verses_str = self
            .verses
            .iter()
            .map(|v| format!("{}", v))
            .collect::<Vec<String>>()
            .join("\n");
        write!(f, "Chapter {}:\n{}", self.chapter_number, verses_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_and_accessors() {
        let verses = vec![Verse::new("Test".into(), 1)];
        let chapter = Chapter::new(verses, 1);
        assert_eq!(chapter.number(), 1);
        assert_eq!(chapter.get_verses().len(), 1);
        assert_eq!(chapter.get_verse(1).unwrap().number(), 1);
        assert!(chapter.get_verse(0).is_none());
    }

    #[test]
    fn test_clone_independence() {
        let verses = vec![Verse::new("Clone".into(), 1)];
        let original = Chapter::new(verses, 1);
        let cloned = original.clone();

        assert_eq!(original.number(), cloned.number());
        assert_eq!(original.get_verses().len(), cloned.get_verses().len());
        assert_eq!(
            original.get_verses()[0].text(),
            cloned.get_verses()[0].text()
        );

        // Ensure the cloned chapter owns its data
        assert_ne!(original.get_verses().as_ptr(), cloned.get_verses().as_ptr());
        assert_ne!(
            original.get_verses()[0].text().as_ptr(),
            cloned.get_verses()[0].text().as_ptr()
        );
    }
}
