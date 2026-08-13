# Changelog

All notable changes to this project are documented here.

## 1.1.0

- Updated `bible-io-references` to 1.1.1 and re-exported its core reference,
  passage, parser, formatter, and identifier APIs.
- Added the versioned v1 content schema with lossless annotations, metadata,
  explicit edition ordering, sparse declared coordinates, strict/permissive
  validation, canonical serialization, and path-aware data errors.
- Added edition-aware reference and passage resolution, locations, persisted
  verse keys, formatting, chapter/verse navigation, a shape-preserving
  `get_by_reference` dispatcher, and immutable resolved verse selections.
- Added Unicode exact/all/any and fuzzy search, lazy/eager/disabled index
  policies, paginated search values, UTF-8 match ranges, safe snippets, and
  statistics/performance metrics.
- Added source metadata and catalog parsing with aliases, inference, merging,
  validation, and value semantics.
- Fixed lossy brace removal, silent sparse-coordinate renumbering, malformed
  content panics, duplicate semantic book handling, conflicting legacy aliases,
  ASCII-only search, index inconsistencies, Dart-compatible numeric-key bounds,
  exact `bookOrder` diagnostics, and the declared Rust 1.85 MSRV.
- Added a tag-driven, OIDC-authenticated crates.io publishing workflow.
- Ported every applicable behavioral contract from the 145-test Dart suite and
  documented the platform/type-system translations used by the Rust tests.

## 1.0.2

- Added shared reference parsing through `bible-io-references`.
