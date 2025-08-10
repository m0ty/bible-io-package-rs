use std::path::Path;

/// Common test utilities for integration tests
pub mod test_utils {
    use super::*;

    /// Helper function to find the bbe.json file
    pub fn get_bbe_json() -> Option<String> {
        let test_fixtures_path = "tests/fixtures/bbe.json";
        if Path::new(test_fixtures_path).exists() {
            return Some(test_fixtures_path.to_string());
        }
        None
    }
}
