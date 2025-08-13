use std::path::Path;

/// Common test utilities for integration tests
pub mod test_utils {
    use super::*;

    pub fn get_kjv_json() -> Option<String> {
        let test_fixtures_path = "tests/fixtures/en_kjv.json";
        if Path::new(test_fixtures_path).exists() {
            return Some(test_fixtures_path.to_string());
        }
        None
    }
}
