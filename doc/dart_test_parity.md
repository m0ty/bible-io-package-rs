# Dart test parity

This document maps the public behavior tested by
[`bible-io-package-dart`](https://github.com/m0ty/bible-io-package-dart) to
this crate's Rust test suite. The comparison baseline is Dart commit
`8f056b6734c5f80e656c4b12e8e3de0786c0837b`, which contains 145 test cases in
19 test files.

The Dart tests and documented data contracts are the oracle. Tests in this
crate must not derive expected values from the implementation under test. When
a fixture-dependent value is needed, it is counted or decoded independently
from the raw fixture. A behavior may be marked not applicable only when Rust's
type system or platform replaces the Dart runtime concept.

Rust tests consolidate related assertion matrices, so Rust and Dart test counts
are intentionally not expected to match one for one.

The completed assertion-by-assertion audit classifies the 145 Dart tests as:

- 140 directly or equivalently covered by executable Rust tests;
- 1 intentional host error-model translation (missing-file outer error type);
- 4 wholly inapplicable tests (one vacuous placeholder, one negative value for
  a Rust `usize`, and two tests of Dart's package-specific `Result` wrapper).

| Dart test file | Rust coverage |
| --- | --- |
| `bible_loading_test.dart` | `loading_index_performance_parity_tests.rs`, `dart_loading_source_contract_gap_tests.rs` |
| `bible_test.dart` | `dart_loading_source_contract_gap_tests.rs`, `references_dependency_tests.rs` |
| `book_navigation_test.dart` | `model_state_navigation_parity_tests.rs` |
| `boundary_performance_test.dart` | `loading_index_performance_parity_tests.rs` |
| `chapter_navigation_test.dart` | `model_state_navigation_parity_tests.rs` |
| `content_schema_test.dart` | `parity_tests.rs`, `dart_loading_source_contract_gap_tests.rs`, `loading_index_performance_parity_tests.rs` |
| `data_validation_test.dart` | `model_state_navigation_parity_tests.rs` |
| `extensions_fuzzy_stats_test.dart` | `passage_search_remaining_parity_tests.rs`, `search_value_edge_parity_tests.rs` |
| `loading_catalog_metadata_test.dart` | `source_parity_tests.rs`, `dart_loading_source_contract_gap_tests.rs` |
| `model_correctness_test.dart` | `model_state_navigation_parity_tests.rs`, `parity_tests.rs` |
| `package_test.dart` | `integration_tests.rs`, `model_state_navigation_parity_tests.rs`, `reference_result_parity_tests.rs`, `reference_tests.rs`, `search_tests.rs` |
| `passage_resolution_test.dart` | `passage_search_remaining_parity_tests.rs`, `parity_tests.rs`, `reference_result_parity_tests.rs` |
| `reference_parsing_test.dart` | `passage_search_remaining_parity_tests.rs`, `reference_result_parity_tests.rs`, `reference_tests.rs` |
| `search_advanced_unicode_test.dart` | `search_parity_tests.rs`, `search_value_edge_parity_tests.rs`, `passage_search_remaining_parity_tests.rs` |
| `search_functionality_test.dart` | `search_tests.rs`, `search_value_edge_parity_tests.rs`, `passage_search_remaining_parity_tests.rs` |
| `search_values_test.dart` | `search_parity_tests.rs`, `search_value_edge_parity_tests.rs`, `passage_search_remaining_parity_tests.rs` |
| `source_contract_test.dart` | `source_parity_tests.rs`, `source_error_value_parity_tests.rs`, `dart_loading_source_contract_gap_tests.rs` |
| `state_value_test.dart` | `model_state_navigation_parity_tests.rs`, `parity_tests.rs` |
| `verse_navigation_test.dart` | `model_state_navigation_parity_tests.rs` |

`adversarial_contract_property_tests.rs` supplements the one-case mapping with
all six permutations of a three-book edition order, both range endpoints,
eager/lazy/scanned search equivalence against independent expected locations,
42 pagination boundaries, Dart-compatible signed-integer key parsing, and exact
`bookOrder` diagnostics.

## Rust-specific equivalents

- Flutter `AssetBundle` loading is represented by path, JSON string, UTF-8
  bytes, decoded-value, source-aware, and progress-reporting loaders.
- Dart isolate/background construction is represented by synchronous Rust
  construction plus `Send + Sync` concurrent-read tests. Search-index modes
  and their observable lifecycle are tested independently of scheduling.
- Dart's package-specific `Result<T>` and `ResultException` wrappers map to
  Rust's native `Result<T, E>`. Tests verify typed errors, stable codes and
  paths, concrete `Error::source()` downcasts, and compatibility string output.
- Dart's unmodifiable reference/passage results map to `VerseSelection`, whose
  compile-fail documentation proves that callers cannot reorder it. Existing
  legacy methods retain their `Vec` results for compatibility; the corresponding
  `*_selection` APIs provide the immutable contract. Model collections and maps
  are exposed through slices and immutable borrows, and tests verify deep
  defensive ownership by mutating original inputs after construction.
- Negative coordinates, wrong-typed `copyWith` values, and non-JSON extension
  objects are unrepresentable in the corresponding typed Rust APIs. Malformed
  external JSON is nevertheless tested at every public deserialization boundary.
- Dart reports string offsets in UTF-16 code units. Rust search values document
  and test UTF-8 byte offsets on character boundaries, including exact slicing
  of NFC/NFD and multi-byte-script source text.
- Dart stack-trace objects have no stable Rust value equivalent. Rust instead
  preserves the concrete error chain and tests downcasting through
  `std::error::Error::source`.
- A missing file is a top-level `FileSystemException` in Dart. Rust wraps the
  concrete `std::io::Error` in the crate's uniform `BibleDataFormatError` and
  preserves it for typed downcasting through `Error::source`.
- Dart exposes `versionDate` as a `DateTime`; Rust validates and preserves the
  same ISO-8601 date as a string, avoiding a mandatory date/time dependency.

## Release gate

Parity changes are accepted only after the Rust 1.85 all-target/all-feature and
minimal-feature test suites, formatting, Clippy with warnings denied, docs with
warnings denied, and package verification all pass. The publishing workflow
runs the same checks before authenticating to crates.io.
