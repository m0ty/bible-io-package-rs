# Rust Bible Struct

A Rust library for working with Bible text data structures, providing efficient parsing and access to Bible books, chapters, and verses.

## Features

- Parse Bible data from JSON files
- Access books, chapters, and verses by various identifiers
- Type-safe Bible book enumeration
- Efficient data structures with O(1) lookup times
- Serde integration for JSON serialization/deserialization

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
rust_bible_struct = "0.1.0"
```

Basic usage:

```rust
use rust_bible_struct::{Bible, BibleBook};

fn main() {
    let bible = Bible::new_from_json("path/to/bible.json");
    
    // Get a specific verse
    if let Some(verse) = bible.get_verse(BibleBook::Genesis, 1, 1) {
        println!("{}", verse);
    }
    
    // Get all verses from a chapter
    if let Some(verses) = bible.get_verses(BibleBook::Psalms, 23) {
        for verse in verses {
            println!("{}", verse);
        }
    }
}
```

## Examples

Run the included example:

```bash
cargo run --example basic_usage
```

## Project Structure

```
rust_bible_struct/
├── src/                    # Source code
├── tests/                  # Integration tests
│   ├── fixtures/          # Test data files (recommended location for bbe.json)
│   ├── common.rs          # Shared test utilities
│   └── integration_tests.rs # Main test suite
├── examples/               # Usage examples
└── bbe.json               # Bible data file (can be moved to tests/fixtures/)
```

## Test Data Organization

The library includes integration tests that require Bible data. For best practices:

- **Recommended**: Place `bbe.json` in `tests/fixtures/` directory
- **Alternative**: Place `bbe.json` directly in `tests/` directory
- **Fallback**: Place `bbe.json` in project root

Use the provided scripts to automatically move your data file:
- **Windows**: `.\move_bbe_json.ps1`
- **Linux/macOS**: `./move_bbe_json.sh`

## Running Tests

```bash
# Run all tests (requires bbe.json)
cargo test

# Run only unit tests (no external data required)
cargo test --lib

# Run only integration tests
cargo test --test integration_tests
```

## License

MIT License - see LICENSE file for details.
