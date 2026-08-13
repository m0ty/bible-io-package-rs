# Bible IO

`bible-io` loads, validates, queries, searches, and serializes Bible editions.
It combines an edition's actual content and ordering with the multilingual
reference model from
[`bible-io-references`](https://crates.io/crates/bible-io-references).

## Features

- Load Bible JSON from a path, UTF-8 bytes, text, or a `serde_json::Value`.
- Validate a versioned, extensible content schema with path-aware errors.
- Preserve unknown JSON annotations on the root, books, chapters, verses, and
  metadata when serializing again.
- Access sparse books, chapters, and verses by their declared identifiers and
  numbers, and navigate across edition boundaries.
- Parse multilingual verses, ranges, whole chapters, verse lists, and passage
  sequences through `bible-io-references` 1.1.1.
- Resolve references according to the books and verses actually present in the
  loaded edition, including a custom `bookOrder`.
- Search Unicode text with exact, all-term, any-term, scoped, paginated, and
  typo-tolerant modes, with display-ready snippets and match ranges.
- Work with stable locations, edition-aware verse keys, source catalogs,
  statistics, and search-index diagnostics.

The complete `bible-io-references` crate is available as
`bible_io::bible_io_references`; its commonly used types and functions are also
re-exported from the `bible_io` root.

## Installation

```toml
[dependencies]
bible-io = "1.1.0"
```

The minimum supported Rust version is 1.85.

## Quick start

```rust
use bible_io::{Bible, BibleBook, SearchMode, SearchOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bible = Bible::new("path/to/bible.json")?;

    let verse = bible.get_verse(BibleBook::John, 3, 16)?;
    println!("{}", verse);

    let verse = bible.get_verse_by_reference("John 3:16")?;
    println!("{}", verse.text());

    let passage = bible.parse_passage("John 3:16,18-20; Acts 2")?;
    let verses = bible.resolve_edition_passage(&passage)?;
    println!("resolved {} verses", verses.len());

    let options = SearchOptions {
        mode: SearchMode::All,
        max_results: Some(10),
        ..SearchOptions::default()
    };
    for hit in bible.search_with_options("faith hope", &options)?.hits() {
        println!("{} — {}", hit.reference(), hit.snippet());
    }

    Ok(())
}
```

Run the included file-loading example with:

```bash
cargo run --example basic_usage
```

## Content JSON

Legacy Bible JSON remains accepted:

```json
{
  "id": "kjv",
  "name": "King James Version",
  "description": "Oxford 1769 edition",
  "language": "English",
  "books": {
    "gn": {
      "name": "Genesis",
      "chapters": {
        "1": {
          "1": "In the beginning God created the heaven and the earth.",
          "2": "And the earth was without form, and void."
        }
      }
    }
  }
}
```

The preferred versioned form adds `schemaVersion`, nested `metadata`, an
optional explicit `bookOrder`, and lossless annotations. Chapter and verse
numbers are positive numeric object keys; they may be sparse and are sorted by
their numeric value when loaded.

See [the content schema](doc/content_schema.md) for the canonical shape,
validation rules, compatibility aliases, annotations, catalogs, and
serialization behavior. Ready-to-use data files are maintained in the
[`bible-io-json` repository](https://github.com/m0ty/bible-io-json).

## Loading and validation

Strict validation is the default for `Bible::new`, `Bible::from_path`, and the
`from_json_*` constructors. It requires at least one book, chapter, and verse,
and nonblank verse text. Use `BibleLoadOptions` to select eager, lazy, or
disabled search indexing, or `BibleDataValidationOptions::PERMISSIVE` for
intentionally skeletal data. Structural and identity checks still apply in
permissive mode.

Malformed content returns a `BibleDataFormatError` containing a stable code,
JSON-style path, message, and optional offending value/cause.

## References and passages

`Bible` adds loaded-edition awareness to `bible-io-references`:

- `parse_reference` and `parse_passage` recognize bundled languages plus
  titles found in the loaded file, and their `EditionReference` /
  `EditionPassage` results support ranges ordered by a custom canon.
- `resolve_edition_reference` and `resolve_edition_passage` return only verses
  present in the edition. The `parse_canonical_*` and `resolve_reference`
  methods remain available for the dependency's canonical types.
- `get_verse_range_by_reference` supports edition-ordered ranges even when a
  custom canon differs from the reference crate's canonical 83-book order.
- `get_by_reference` preserves the single-verse versus range result shape via
  `BibleReferenceResult`. The `*_selection` resolution methods return an
  immutable `VerseSelection` while retaining edition order and intentional
  passage duplicates.
- `BibleLocation` and `BibleVerseKey` provide stable values for navigation,
  bookmarks, notes, and reading progress.

## Testing

```bash
cargo fmt --all -- --check
cargo test --all-features --locked
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo doc --all-features --no-deps --locked
cargo package --locked
```

The integration suite uses the checked-in `tests/fixtures/en_kjv.json` fixture.
The [Dart parity matrix](doc/dart_test_parity.md) records the assertion-level
mapping for all 145 upstream Dart tests and the small set of host-language
translations.

## Publishing

Tags in the form `Release-x.y.z` trigger the
[`Publish` workflow](.github/workflows/publish.yml). The tag version must match
the `bible-io` version in `Cargo.toml`; the workflow runs the release checks,
verifies the package, obtains a short-lived crates.io token through GitHub
OIDC, and publishes with the lockfile.

Configure the crate's crates.io trusted publisher once with:

- GitHub owner: `m0ty`
- Repository: `bible-json-package-rs`
- Workflow: `publish.yml`
- Environment: leave blank

No long-lived `CARGO_REGISTRY_TOKEN` GitHub secret is required.

## License

GNU Affero General Public License v3.0 only. See [LICENSE](LICENSE).
