# Rust Bible Struct

A Rust library for working with Bible text data structures, providing efficient parsing and access to Bible books, chapters, and verses.

## Features

- Parse Bible data from JSON files
- Access books, chapters, and verses by various identifiers
- Type-safe Bible book enumeration

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
    if let Ok(verse) = bible.get_verse(BibleBook::Genesis, 1, 1) {
        println!("{}", verse);
    }

    // Get all verses from a chapter
    if let Ok(verses) = bible.get_verses(BibleBook::Psalms, 23) {
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

## Running Tests

```bash
# Run all tests (requires en_kjv.json)
cargo test

# Run only unit tests (no external data required)
cargo test --lib

# Run only integration tests
cargo test --test integration_tests
```

## License

MIT License - see LICENSE file for details.
