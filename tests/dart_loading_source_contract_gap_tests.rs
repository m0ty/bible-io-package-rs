use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use bible_io::{
    Bible, BibleBook, BibleCatalog, BibleDataFormatError, BibleDataFormatErrorCode,
    BibleLoadOptions, BibleMetadata, BibleSource, Book, Chapter, JsonMap, Language,
    MachineIdentifiers, SearchIndexMode, TextDirectionHint, Verse, VerseRef,
};
use serde_json::{json, Value};

static NEXT_TEMP_FILE_ID: AtomicU64 = AtomicU64::new(0);

struct TempJsonFile {
    path: PathBuf,
}

impl TempJsonFile {
    fn new(contents: &str) -> Self {
        let sequence = NEXT_TEMP_FILE_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "bible-io-dart-gap-{}-{sequence}.json",
            std::process::id()
        ));
        fs::write(&path, contents).expect("temporary Bible fixture should be writable");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempJsonFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn minimal_bible_value() -> Value {
    json!({
        "language": "English",
        "books": {
            "gn": {
                "name": "Genesis",
                "chapters": {
                    "1": {"1": "Alpha beta.", "2": "Gamma delta."}
                }
            }
        }
    })
}

fn annotated_bible_value() -> Value {
    json!({
        "schemaVersion": 1,
        "language": "English",
        "provider": {"slug": "example"},
        "metadata": {
            "id": "example-2026",
            "description": "Schema fixture",
            "translationName": "Example Translation",
            "customMetadata": {"revision": 2}
        },
        "bookOrder": ["ex", "gn"],
        "books": {
            "gn": {
                "name": "Genesis Local",
                "section": "Torah",
                "chapters": {
                    "1": {
                        "heading": "Creation",
                        "verses": {
                            "1": {
                                "text": "In the beginning God created.",
                                "paragraphStart": true
                            }
                        }
                    }
                }
            },
            "ex": {
                "name": "Exodus Local",
                "chapters": {"1": {"1": "These are the names."}}
            }
        }
    })
}

#[test]
fn path_loading_wraps_json_and_schema_failures_with_dart_diagnostics() {
    let invalid_json = TempJsonFile::new("invalid json");
    let error = Bible::from_path(invalid_json.path(), BibleLoadOptions::default()).unwrap_err();
    assert_eq!(error.code(), BibleDataFormatErrorCode::InvalidJson);
    assert_eq!(error.path(), "$");
    assert!(error
        .source()
        .and_then(|cause| cause.downcast_ref::<serde_json::Error>())
        .is_some());

    let malformed = TempJsonFile::new(r#"{"invalid":"structure"}"#);
    let error = Bible::from_path(malformed.path(), BibleLoadOptions::default()).unwrap_err();
    assert_eq!(error.code(), BibleDataFormatErrorCode::MissingField);
    assert_eq!(error.path(), "$.books");
}

#[test]
fn path_loading_preserves_utf8_scripts_exactly() {
    let document = json!({
        "books": {
            "gn": {"chapters": {"1": {
                "1": "في البدء",
                "2": "В начале",
                "3": "起初"
            }}}
        }
    });
    let file = TempJsonFile::new(&serde_json::to_string(&document).unwrap());
    let bible = Bible::from_path(file.path(), BibleLoadOptions::default()).unwrap();

    assert_eq!(
        bible.get_verse(BibleBook::Genesis, 1, 1).unwrap().text(),
        "في البدء"
    );
    assert_eq!(
        bible.get_verse(BibleBook::Genesis, 1, 2).unwrap().text(),
        "В начале"
    );
    assert_eq!(
        bible.get_verse(BibleBook::Genesis, 1, 3).unwrap().text(),
        "起初"
    );
    assert_eq!(bible.search("البدء").len(), 1);
    assert_eq!(bible.search("начале").len(), 1);
    assert_eq!(bible.search("起初").len(), 1);
}

#[test]
fn schema_identity_annotations_encoding_and_edition_order_match_dart_exactly() {
    let bible = Bible::from_json_value(annotated_bible_value()).unwrap();
    assert_eq!(bible.schema_version(), 1);
    assert_eq!(bible.id(), "example-2026");
    assert_eq!(bible.description(), "Schema fixture");
    assert_eq!(
        bible.books().iter().map(Book::book).collect::<Vec<_>>(),
        [BibleBook::Exodus, BibleBook::Genesis]
    );
    assert_eq!(bible.annotations()["provider"], json!({"slug": "example"}));
    assert_eq!(
        bible.metadata().additional["customMetadata"],
        json!({"revision": 2})
    );

    let genesis = bible.get_book(BibleBook::Genesis).unwrap();
    let chapter = genesis.get_chapter(1).unwrap();
    let verse = chapter.get_verse(1).unwrap();
    assert_eq!(genesis.annotations()["section"], "Torah");
    assert_eq!(chapter.annotations()["heading"], "Creation");
    assert_eq!(verse.annotations()["paragraphStart"], true);

    let encoded = bible.to_json_value();
    assert_eq!(encoded["schemaVersion"], 1);
    assert_eq!(encoded["bookOrder"], json!(["ex", "gn"]));
    assert_eq!(encoded["provider"], json!({"slug": "example"}));
    assert_eq!(
        encoded["metadata"]["customMetadata"],
        json!({"revision": 2})
    );
    assert_eq!(Bible::from_json_value(encoded).unwrap(), bible);

    let range = bible
        .get_verse_range_by_reference("Exodus Local 1:1-Genesis Local 1:1")
        .unwrap();
    assert_eq!(
        range.iter().map(|verse| verse.text()).collect::<Vec<_>>(),
        ["These are the names.", "In the beginning God created."]
    );
    let passage = bible
        .get_passage("Exodus Local 1:1-Genesis Local 1:1")
        .unwrap();
    assert_eq!(passage, range);
}

#[test]
fn source_path_inference_and_mixed_catalog_entries_match_dart_exactly() {
    let source = BibleSource::from_asset_path("bible_io_json/English/kjv.json");
    assert_eq!(source.id, "english_kjv");
    assert_eq!(source.asset_path, "bible_io_json/English/kjv.json");
    assert_eq!(source.language_name, "English");
    assert_eq!(source.language_code, "en");
    assert_eq!(source.abbreviation, "KJV");
    assert_eq!(source.translation_name, "KJV");

    let catalog = BibleCatalog::from_value(&json!({
        "sources": [
            {
                "id": "kjv",
                "assetPath": "bible_io_json/English/kjv.json",
                "translationName": "King James Version",
                "abbreviation": "KJV",
                "languageCode": "en"
            },
            "bible_io_json/Hebrew/wlc.json"
        ]
    }))
    .unwrap();
    assert_eq!(catalog.sources().len(), 2);
    assert_eq!(
        catalog.find_by_id("kjv").unwrap().translation_name,
        "King James Version"
    );
    assert_eq!(catalog.for_language("en")[0].abbreviation, "KJV");
    assert_eq!(
        catalog.for_language("he")[0].direction,
        TextDirectionHint::Rtl
    );

    let nested = BibleCatalog::from_value(&json!({
        "English": {
            "kjv": "translations/kjv.json",
            "web": "translations/web.json"
        }
    }))
    .unwrap();
    assert_eq!(
        nested
            .sources()
            .iter()
            .map(|source| source.id.as_str())
            .collect::<Vec<_>>(),
        ["kjv", "web"]
    );
    assert_eq!(
        nested
            .sources()
            .iter()
            .map(|source| source.asset_path.as_str())
            .collect::<Vec<_>>(),
        ["translations/kjv.json", "translations/web.json"]
    );
    assert!(nested
        .sources()
        .iter()
        .all(|source| source.language_name == "English"));
    assert_eq!(nested.find_by_id("web").unwrap().abbreviation, "WEB");

    let blank_id = BibleCatalog::from_value(&json!({
        "sources": [{
            "id": "   ",
            "assetPath": "bibles/English/a.json",
            "languageName": "English",
            "languageCode": "en",
            "translationName": "A",
            "abbreviation": "A"
        }]
    }))
    .unwrap_err();
    // Parsed IDs are rejected by Dart's `_readIdentifier` before the
    // constructor's required-field validation runs.
    assert_eq!(blank_id.code(), BibleDataFormatErrorCode::InvalidValue);
    assert_eq!(blank_id.path(), "$.sources[0].id");
    assert_eq!(blank_id.value(), Some(&json!("   ")));
}

#[test]
fn rich_metadata_is_exposed_on_bible_values() {
    let mut value = minimal_bible_value();
    value["metadata"] = json!({
        "translationName": "King James Version",
        "abbreviation": "KJV",
        "languageCode": "en",
        "license": "Public Domain",
        "canon": "protestant",
        "versionDate": "1769-01-01"
    });
    let bible = Bible::from_json_value(value).unwrap();

    assert_eq!(bible.name(), "King James Version");
    assert_eq!(bible.abbreviation(), Some("KJV"));
    assert_eq!(bible.language_code(), Some("en"));
    assert_eq!(bible.license(), Some("Public Domain"));
    assert_eq!(bible.canon(), Some("protestant"));
    assert_eq!(bible.version_date(), Some("1769-01-01"));
}

#[test]
fn source_round_trips_through_bible_json_and_supplies_an_omitted_language() {
    let source = BibleSource::from_value(&json!({
        "id": "wlc",
        "assetPath": "bible_io_json/Hebrew/wlc.json",
        "languageName": "Hebrew",
        "languageCode": "he",
        "translationName": "Westminster Leningrad Codex",
        "abbreviation": "WLC",
        "year": 1769,
        "direction": "rtl",
        "sourceName": "Public Domain Text",
        "license": "Public Domain",
        "canon": "protestant",
        "versionDate": "1769-01-01"
    }))
    .unwrap();
    let mut value = minimal_bible_value();
    value.as_object_mut().unwrap().remove("language");
    let bible =
        Bible::from_json_value_with_source(value, &source, BibleLoadOptions::default()).unwrap();

    assert_eq!(bible.source(), Some(&source));
    assert_eq!(bible.language_id(), Language::Hebrew);
    let restored = Bible::from_json_value(bible.to_json_value()).unwrap();
    assert_eq!(restored.source(), Some(&source));
    assert_eq!(
        restored.source().unwrap().to_json_value(),
        source.to_json_value()
    );
}

#[test]
fn format_error_diagnostics_and_parsed_extension_ownership_are_stable() {
    let error = BibleDataFormatError::new(
        BibleDataFormatErrorCode::InvalidType,
        "$.books.gn",
        "Expected an object.",
    )
    .with_value(json!(42));
    assert_eq!(error.code().as_str(), "invalid_type");
    assert_eq!(error.path(), "$.books.gn");
    assert_eq!(error.value(), Some(&json!(42)));
    assert!(error.to_string().contains("$.books.gn"));

    let mut source_input = json!({
        "id": "kjv",
        "assetPath": "bibles/English/kjv.json",
        "languageName": "English",
        "languageCode": "en",
        "translationName": "King James Version",
        "abbreviation": "KJV",
        "provider": {"revision": 2, "links": ["https://example.test"]}
    });
    let source = BibleSource::from_value(&source_input).unwrap();
    source_input["provider"]["revision"] = json!(3);
    source_input["provider"]["links"][0] = json!("mutated");
    assert_eq!(source.additional["provider"]["revision"], 2);
    assert_eq!(
        source.additional["provider"]["links"][0],
        "https://example.test"
    );
    assert_eq!(
        BibleSource::from_value(&source.to_json_value()).unwrap(),
        source
    );

    let mut metadata_input = json!({
        "id": "eng-kjv-1769",
        "description": "A stable edition",
        "schemaVersion": 1,
        "metadata": {
            "provider": {
                "name": "Bible Provider",
                "links": ["https://example.test"]
            }
        }
    });
    let metadata = BibleMetadata::from_document_value(&metadata_input, None).unwrap();
    metadata_input["metadata"]["provider"]["name"] = json!("mutated input");
    metadata_input["metadata"]["provider"]["links"][0] = json!("mutated");
    assert_eq!(metadata.additional["schemaVersion"], 1);
    assert_eq!(metadata.id.as_deref(), Some("eng-kjv-1769"));
    assert_eq!(metadata.description.as_deref(), Some("A stable edition"));
    assert_eq!(metadata.additional["provider"]["name"], "Bible Provider");
    assert_eq!(
        metadata.additional["provider"]["links"][0],
        "https://example.test"
    );
    assert_eq!(
        metadata.to_json_value()["provider"],
        json!({
            "name": "Bible Provider",
            "links": ["https://example.test"]
        })
    );

    for id in ["", " edition "] {
        assert!(BibleMetadata::from_value(&json!({"id": id})).is_err());
    }
}

#[test]
fn public_entrypoint_combines_bible_and_reference_identifier_apis() {
    let reference = VerseRef::new(BibleBook::John, 3, 16).unwrap();
    let verse = Verse::checked(
        BibleBook::John,
        3,
        16,
        "For God so loved the world.",
        JsonMap::new(),
    )
    .unwrap();
    let chapter = Chapter::checked(BibleBook::John, 3, vec![verse], JsonMap::new()).unwrap();
    let book = Book::checked(BibleBook::John, "John", vec![chapter], JsonMap::new()).unwrap();
    let bible = Bible::from_books(
        vec![book],
        Language::English,
        BibleMetadata::default(),
        JsonMap::new(),
        SearchIndexMode::Eager,
    )
    .unwrap();

    assert_eq!(reference.osis_identifier(), "John.3.16");
    let resolved = bible.resolve_reference(reference.into()).unwrap();
    assert_eq!(resolved.len(), 1);
    assert!(resolved[0].text().contains("loved"));
}
