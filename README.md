# Bible-IO

A Rust library for working with Bible text data structures, providing efficient parsing and access to Bible books, chapters, and verses.

## Features

- Parse Bible data from JSON files
- Access books, chapters, and verses by various identifiers
- Parse multilingual human-readable references with
  [`bible-io-references`](https://crates.io/crates/bible-io-references)
- Use the shared 83-book `BibleBook` model, including deuterocanonical books

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
bible-io = "1.0.2"
```

Basic usage:

```rust
use bible_io::{Bible, BibleBook};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bible = Bible::new("path/to/bible.json")?;

    // Get a specific verse
    if let Ok(verse) = bible.get_verse(BibleBook::Genesis, 1, 1) {
        println!("{}", verse);
    }

    // Parse and retrieve a human-readable reference
    if let Ok(verse) = bible.get_verse_by_reference("John 3:16") {
        println!("{}", verse);
    }

    // Get all verses from a chapter
    if let Ok(verses) = bible.get_verses(BibleBook::Psalms, 23) {
        for verse in verses {
            println!("{}", verse);
        }
    }

    Ok(())
}
```

## JSON Structure

The library expects Bible data in the following JSON format:

```json
{
    "id": "kjv",
    "name": "King James Version",
    "description": "The King James Version (Oxford 1769) is a standardized revision of the classic 1611 English Bible.",
    "language": "English",
    "books": {
        "gn": {
            "name": "Genesis",
            "chapters": {
                "1": {
                    "1": "In the beginning God created the heaven and the earth.",
                    "2": "And the earth was without form, and void; and darkness {was} upon the face of the deep. And the Spirit of God moved upon the face of the waters.",
                    "3": "And God said, Let there be light: and there was light."
                },
                "2": {
                    "1": "Thus the heavens and the earth were finished, and all the host of them.",
                    "2": "And on the seventh day God ended his work which he had made; and he rested on the seventh day from all his work which he had made.",
                    "3": "And God blessed the seventh day, and sanctified it: because that in it he had rested from all his work which God created and made. "
                }
            }
        }
    }
}
```

Top-level fields identify the translation. Each entry in `books` uses a book abbreviation (e.g., `"gn"`) and contains a `name` and a `chapters` array, where each chapter is a list of verse strings.

Check https://github.com/m0ty/bible-io-json repository for ready to use bible .json files.

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

GNU Affero General Public License v3.0 only. See LICENSE for details.
