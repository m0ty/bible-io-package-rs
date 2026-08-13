# Bible content schema

This document defines the JSON content contract consumed and emitted by
`bible-io`. The current schema version is `1`, exposed as
`CURRENT_BIBLE_SCHEMA_VERSION`.

The loader accepts both the preferred versioned form and the package's legacy
top-level metadata form. `Bible::to_json` and `Bible::to_json_value` always emit
the canonical versioned form.

## Complete example

```json
{
  "schemaVersion": 1,
  "language": "English",
  "metadata": {
    "id": "eng-kjv",
    "translationName": "King James Version",
    "abbreviation": "KJV",
    "description": "Oxford 1769 edition",
    "languageName": "English",
    "languageCode": "en",
    "year": 1769,
    "direction": "ltr",
    "license": "Public domain"
  },
  "bookOrder": ["gn"],
  "books": {
    "gn": {
      "name": "Genesis",
      "testament": "old",
      "chapters": {
        "1": {
          "heading": "Creation",
          "verses": {
            "1": {
              "text": "In the beginning God created the heaven and the earth.",
              "paragraph": 1
            },
            "2": "And the earth was without form, and void."
          }
        }
      }
    }
  },
  "datasetRevision": "2026-08-13"
}
```

`testament`, `heading`, `paragraph`, and `datasetRevision` are examples of
extension annotations. They are not interpreted by the crate, but they are
retained at their original structural level and emitted again.

## Root object

The JSON root must be an object.

| Field | Type | Required | Meaning |
| --- | --- | --- | --- |
| `schemaVersion` | positive integer | No | Defaults to `1`; values other than the current version are rejected. |
| `language` | string | No | Preferred parsing/formatting language name, code, or locale tag. Unknown values fall back to automatic detection. |
| `metadata` | object | No | Canonical edition metadata. |
| `bookOrder` | array of strings | No | Exact loaded-edition order. It must name every loaded book once. |
| `books` | object | Yes in strict mode | Bible books keyed by a supported identifier. |

For legacy files, metadata fields may appear at the root. Nested `metadata`
values take precedence over their root equivalents. Recognized legacy fields
are canonicalized into `metadata` during serialization; unrecognized root
fields remain root annotations.

When `bookOrder` is absent, books use the canonical order of the shared
83-book `BibleBook` model. When present, its order governs iteration,
navigation, edition ranges, range resolution, and search result order. This is
important for editions whose canon order differs from the shared model.

### Book identifiers

A key in `books` and an item in `bookOrder` may use any identifier understood
by `BibleBook`:

- the compact package abbreviation, such as `gn`, `jo`, or `1co`;
- a canonical English name;
- an OSIS identifier;
- a USFM identifier.

Serialization always uses the compact package abbreviation. Two different
input identifiers that resolve to the same book are rejected.

## Metadata

All metadata fields are optional unless they occur inside a catalog source.
The canonical serialized names and accepted compatibility aliases are:

| Canonical field | Accepted aliases | Type |
| --- | --- | --- |
| `source` | — | `BibleSource` object |
| `id` | `editionId`, `edition_id` | string |
| `description` | `summary` | string |
| `languageName` | `language_name`, `language` | string |
| `languageCode` | `language_code`, `lang` | string |
| `translationName` | `translation_name`, `name`, `title`, `version` | string |
| `abbreviation` | `abbr`, `shortName`, `short_name` | string |
| `year` | — | integer or integer string |
| `direction` | `textDirection`, `text_direction` | `auto`, `ltr`, or `rtl` |
| `sourceName` | `source_name` | string |
| `copyright` | — | string |
| `license` | — | string |
| `canon` | — | string |
| `versionDate` | `version_date`, `date` | string |

An edition `id`, when supplied, must be trimmed and nonblank. Unknown metadata
fields are retained through the `BibleMetadata::additional` map. Unknown
fields inside `source` are likewise retained through `BibleSource::additional`.
Direction compatibility spellings such as `left-to-right`, `left_to_right`,
`right-to-left`, and `right_to_left` are accepted and serialized canonically.
`versionDate` must be an ISO-8601 date or date-time with a valid calendar date.

The language used by the reference parser is selected from the first
recognized value among root `language`, metadata `languageCode`, and metadata
`languageName`. The original metadata strings remain accessible even when no
bundled language matches them.

## Books

Each book value is an object:

```json
{
  "name": "Genesis",
  "chapters": {
    "1": { "1": "Verse text" }
  }
}
```

- `name` is optional and defaults to the canonical English book name. If
  supplied, it must be a nonblank string and becomes a reference-parser alias
  for that edition.
- `chapters` is required in strict mode and must be an object.
- Other fields are book annotations and are retained.

Chapter keys must parse as positive integers. Chapters are sorted numerically,
may be sparse, and must not contain numerically duplicate keys such as `"1"`
and `"01"`.

For pre-versioned compatibility, `chapters` may instead be an array. Its first
item is chapter 1, its second is chapter 2, and so on. Serialization always
normalizes this legacy form to the numeric-keyed object above.

## Chapters

An unannotated chapter uses a direct verse map:

```json
{
  "1": "First verse",
  "3": "A deliberately sparse third verse"
}
```

An annotated chapter uses an object with a structural `verses` field:

```json
{
  "heading": "A chapter annotation",
  "verses": {
    "1": "First verse"
  }
}
```

Verse keys follow the same positive, unique numeric-key rules as chapter keys.
Verses are sorted numerically and may be sparse. A direct verse map cannot also
carry chapter annotations; use the structured `verses` form when annotations
are needed.

A chapter, or the `verses` value in a structured chapter, may also be a legacy
array. Array positions become 1-based verse numbers and are serialized back as
a numeric-keyed object.

## Verses

A verse may be a string:

```json
"In the beginning..."
```

Or an annotated object containing a required string `text` field:

```json
{
  "text": "In the beginning...",
  "paragraph": 1,
  "speaker": "narrator"
}
```

Other object fields are verse annotations and are retained. No whitespace or
markup is removed during JSON loading or lossless serialization.

## Extensions and canonical serialization

Unknown JSON fields are preserved on:

- the root object;
- metadata and nested source objects;
- book objects;
- structured chapter objects;
- annotated verse objects.

Extension maps cannot shadow structural fields when models are constructed
through the checked Rust constructors. Reserved keys are:

- book annotations: `name`, `chapters`;
- chapter annotations: `verses`;
- verse annotations: `text`.

Canonical serialization has these properties:

1. `schemaVersion`, resolved `language`, `metadata`, `bookOrder`, and `books`
   are always emitted.
2. Book keys and `bookOrder` use compact `BibleBook` abbreviations.
3. Books, chapters, and verses follow loaded-edition/numeric order.
4. An unannotated chapter is emitted as a direct verse map; an annotated one
   is emitted with `verses`.
5. An unannotated verse is emitted as a string; an annotated one is emitted as
   an object with `text`.
6. Known compatibility aliases are normalized to canonical field names.

## Validation policies

`BibleDataValidationOptions::default()` is strict and requires:

- at least one book;
- at least one chapter in every book;
- at least one verse in every chapter;
- nonblank verse text.

`BibleDataValidationOptions::PERMISSIVE` relaxes only those four presence
requirements. It does not relax JSON types, schema versions, supported book
identifiers, positive coordinates, duplicate coordinates, metadata identity,
annotation-key, alias-collision, or `bookOrder` validation.

The four policy flags may also be configured independently and passed through
`BibleLoadOptions`.

Malformed data returns `BibleDataFormatError`. Its stable codes are:

- `invalid_json`
- `invalid_type`
- `missing_field`
- `invalid_value`
- `duplicate_id`
- `reserved_field`
- `non_json_value`

The error also provides a JSON-style `path`, a human-readable `message`, and
optional offending `value` and underlying `cause`.

## Loading and writing APIs

The same contract is available through all constructors:

| Input | Strict convenience | Configurable form |
| --- | --- | --- |
| File | `Bible::new` | `Bible::from_path` |
| UTF-8 bytes | `Bible::from_json_slice` | `Bible::from_json_slice_with_options` |
| JSON text | `Bible::from_json_str` | `Bible::from_json_str_with_options` |
| `serde_json::Value` | `Bible::from_json_value` | `Bible::from_json_value_with_options` |
| Rust models | — | `Bible::from_books` |

Use `Bible::to_json_value` for a structured value or `Bible::to_json` for
compact JSON text.

## Source catalogs

`BibleCatalog` describes available edition files; it does not fetch them.
Catalog input may be:

- a list of source objects or asset-path strings;
- an object containing exactly one of `sources`, `bibles`, or `translations`;
- an ID-keyed map of sources;
- nested language-keyed objects used by existing data repositories.

A fully specified `BibleSource` requires these nonblank fields:

| Field | Meaning |
| --- | --- |
| `id` | Stable source/edition ID |
| `assetPath` | Local asset, file, or URL-like path |
| `languageName` | Human-readable language |
| `languageCode` | Language code |
| `translationName` | Translation display name |
| `abbreviation` | Short translation label |

The metadata aliases listed above are accepted, and asset paths also accept
`asset_path`, `path`, `file`, or `url`. Source IDs also accept `key`, and a
source's `sourceName` accepts `source_name` or `source`. A path-only entry
derives conventional metadata through `BibleSource::from_asset_path`; catalog
nesting can provide the source ID or language name. Catalog IDs must be unique.

## Stable supporting JSON values

`BibleLocation` serializes a chapter or verse location with a compact book ID:

```json
{"book":"jo","chapter":3,"verse":16}
```

The `verse` field is omitted for a chapter location. `BibleVerseKey` adds an
edition ID for bookmarks, notes, or reading progress:

```json
{
  "editionId": "eng-kjv",
  "location": {"book":"jo","chapter":3,"verse":16}
}
```

Search `TextRange` values and reference-extractor spans are UTF-8 byte offsets,
so they are safe for Rust string slicing. This intentionally differs from the
Dart package's UTF-16 code-unit offsets.
