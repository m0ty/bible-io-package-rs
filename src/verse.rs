use std::fmt;

/// Represents a single verse from the Bible.
///
/// A verse contains the text content and its verse number within a chapter.
#[derive(Debug, Clone)]
pub struct Verse {
    verse_text: String,
    verse_number: usize,
}

impl Verse {
    /// Creates a new verse with the given text and verse number.
    ///
    /// # Arguments
    ///
    /// * `verse_text` - The text content of the verse
    /// * `verse_number` - The verse number within its chapter
    pub fn new(verse_text: String, verse_number: usize) -> Self {
        Verse {
            verse_text,
            verse_number,
        }
    }

    /// Returns the text content of the verse.
    pub fn text(&self) -> &str {
        &self.verse_text
    }

    /// Returns the verse number within its chapter.
    pub fn number(&self) -> usize {
        self.verse_number
    }
}

impl fmt::Display for Verse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.verse_number, self.verse_text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_and_accessors() {
        let verse = Verse::new("Test".to_string(), 1);
        assert_eq!(verse.text(), "Test");
        assert_eq!(verse.number(), 1);
        assert_eq!(format!("{}", verse), "1: Test");
    }

    #[test]
    fn test_clone_independence() {
        let original = Verse::new("Clone me".to_string(), 42);
        let cloned = original.clone();

        assert_eq!(original.text(), cloned.text());
        assert_eq!(original.number(), cloned.number());

        // Ensure the cloned verse has its own allocation
        assert_ne!(original.text().as_ptr(), cloned.text().as_ptr());
    }
}
