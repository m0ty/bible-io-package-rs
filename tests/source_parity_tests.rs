use bible_io::{
    source::merge_bible_metadata, BibleCatalog, BibleDataFormatErrorCode, BibleMetadata,
    BibleSource, TextDirectionHint,
};
use serde_json::{json, Map, Value};
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

fn value_hash(value: &impl Hash) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

fn source_fixture(id: &str) -> BibleSource {
    BibleSource {
        id: id.to_string(),
        asset_path: "bibles/English/kjv.json".to_string(),
        language_name: "English".to_string(),
        language_code: "en".to_string(),
        translation_name: "King James Version".to_string(),
        abbreviation: "KJV".to_string(),
        description: None,
        year: None,
        direction: TextDirectionHint::Ltr,
        source_name: None,
        copyright: None,
        license: None,
        canon: None,
        version_date: None,
        additional: Map::new(),
    }
}

#[test]
fn asset_path_inference_matches_dart_edge_cases() {
    let bare = BibleSource::from_asset_path("kjv.json");
    assert_eq!(bare.id, "kjv");
    assert_eq!(bare.language_name, "");
    assert_eq!(bare.language_code, "");
    assert_eq!(bare.abbreviation, "KJV");
    assert_eq!(bare.translation_name, "KJV");
    let error = bare.validate("$").unwrap_err();
    assert_eq!(error.code(), BibleDataFormatErrorCode::MissingField);
    assert_eq!(error.path(), "$.languageName");

    let italian = BibleSource::from_asset_path("bibles/Italian/cei.json");
    assert_eq!(italian.id, "italian_cei");
    assert_eq!(italian.language_name, "Italian");
    assert_eq!(italian.language_code, "it");

    let hebrew = BibleSource::from_asset_path(r"bibles\Hebrew\wlc.json");
    assert_eq!(hebrew.language_code, "he");
    assert_eq!(hebrew.direction, TextDirectionHint::Rtl);

    for (identifier, expected_code) in [
        ("Hindi", "hi"),
        ("hin", "hi"),
        ("Indonesian", "id"),
        ("id", "id"),
        ("Korean", "ko"),
        ("Tagalog", "tl"),
        ("fil", "tl"),
        ("Vietnamese", "vi"),
    ] {
        let source = BibleSource::from_asset_path(format!("bibles/{identifier}/translation.json"));
        assert_eq!(source.language_code, expected_code, "{identifier}");
    }
}

#[test]
fn source_aliases_are_trimmed_validated_and_round_trip() {
    let source = BibleSource::from_value(&json!({
        "key": "wlc-1917",
        "file": " bibles/Hebrew/wlc.json ",
        "language": " Hebrew ",
        "lang": " he ",
        "title": " Westminster Leningrad Codex ",
        "short_name": " WLC ",
        "summary": " Hebrew Bible ",
        "year": " 1917 ",
        "text_direction": "right-to-left",
        "source": " Open Scriptures ",
        "version_date": "1917-01-01",
        "provider": {"revision": 2}
    }))
    .unwrap();

    assert_eq!(source.id, "wlc-1917");
    assert_eq!(source.asset_path, "bibles/Hebrew/wlc.json");
    assert_eq!(source.language_name, "Hebrew");
    assert_eq!(source.language_code, "he");
    assert_eq!(source.translation_name, "Westminster Leningrad Codex");
    assert_eq!(source.abbreviation, "WLC");
    assert_eq!(source.description.as_deref(), Some("Hebrew Bible"));
    assert_eq!(source.year, Some(1917));
    assert_eq!(source.direction, TextDirectionHint::Rtl);
    assert_eq!(source.source_name.as_deref(), Some("Open Scriptures"));
    assert_eq!(source.additional["provider"]["revision"], 2);

    let restored: BibleSource = serde_json::from_value(source.to_json_value()).unwrap();
    assert_eq!(restored, source);
    assert_eq!(value_hash(&restored), value_hash(&source));
    let serialized = source.to_json_value();
    assert!(serialized.get("copyright").is_none());
}

#[test]
fn malformed_source_values_have_stable_codes_and_paths() {
    let invalid_direction = BibleSource::from_value(&json!({
        "id": "x",
        "assetPath": "bibles/English/x.json",
        "languageName": "English",
        "languageCode": "en",
        "translationName": "X",
        "abbreviation": "X",
        "direction": "sideways"
    }))
    .unwrap_err();
    assert_eq!(
        invalid_direction.code(),
        BibleDataFormatErrorCode::InvalidValue
    );
    assert_eq!(invalid_direction.path(), "$.direction");

    let invalid_date = BibleSource::from_value(&json!({
        "id": "x",
        "assetPath": "bibles/English/x.json",
        "languageName": "English",
        "languageCode": "en",
        "translationName": "X",
        "abbreviation": "X",
        "versionDate": "2026-02-30"
    }))
    .unwrap_err();
    assert_eq!(invalid_date.code(), BibleDataFormatErrorCode::InvalidValue);
    assert_eq!(invalid_date.path(), "$.versionDate");

    let invalid_timestamp = BibleSource::from_value(&json!({
        "id": "x",
        "assetPath": "bibles/English/x.json",
        "languageName": "English",
        "languageCode": "en",
        "translationName": "X",
        "abbreviation": "X",
        "versionDate": "2026-01-01T12:34garbage"
    }))
    .unwrap_err();
    assert_eq!(
        invalid_timestamp.code(),
        BibleDataFormatErrorCode::InvalidValue
    );
    assert_eq!(invalid_timestamp.path(), "$.versionDate");

    let invalid_year_type = BibleSource::from_value(&json!({
        "id": "x",
        "assetPath": "bibles/English/x.json",
        "languageName": "English",
        "languageCode": "en",
        "translationName": "X",
        "abbreviation": "X",
        "year": 1917.5
    }))
    .unwrap_err();
    assert_eq!(
        invalid_year_type.code(),
        BibleDataFormatErrorCode::InvalidType
    );
    assert_eq!(invalid_year_type.path(), "$.year");

    let padded_id = BibleSource::from_value(&json!({
        "id": " x ",
        "assetPath": "bibles/English/x.json",
        "languageName": "English",
        "languageCode": "en",
        "translationName": "X",
        "abbreviation": "X"
    }))
    .unwrap_err();
    assert_eq!(padded_id.path(), "$.id");

    let mut source = source_fixture("x");
    source
        .additional
        .insert("translationName".to_string(), json!("hidden"));
    let reserved = source.validate("$").unwrap_err();
    assert_eq!(reserved.code(), BibleDataFormatErrorCode::ReservedField);
    assert_eq!(reserved.path(), "$.translationName");
}

#[test]
fn catalogs_infer_nested_ids_and_languages_without_bogus_paths() {
    let catalog = BibleCatalog::from_value(&json!({
        "English": {
            "kjv": "translations/kjv.json",
            "web": "translations/web.json"
        },
        "Hebrew": ["translations/wlc.json"]
    }))
    .unwrap();

    assert_eq!(catalog.sources().len(), 3);
    let kjv = catalog.find_by_id("kjv").unwrap();
    assert_eq!(kjv.asset_path, "translations/kjv.json");
    assert_eq!(kjv.language_name, "English");
    assert_eq!(kjv.language_code, "en");
    assert_eq!(catalog.for_language("EN").len(), 2);
    assert_eq!(catalog.for_language("he").len(), 1);
    assert_eq!(catalog.by_language_name()["English"].len(), 2);
    assert_eq!(
        catalog
            .by_language_name()
            .keys()
            .copied()
            .collect::<Vec<_>>(),
        ["English", "Hebrew"]
    );

    let equivalent = BibleCatalog::new(catalog.sources().to_vec()).unwrap();
    assert_eq!(equivalent, catalog);
    assert_eq!(value_hash(&equivalent), value_hash(&catalog));
}

#[test]
fn catalogs_reject_recognized_but_incomplete_sources_and_bad_entries() {
    let missing_path = BibleCatalog::from_value(&json!({
        "sources": [{"id": "missing-path"}]
    }))
    .unwrap_err();
    assert_eq!(missing_path.code(), BibleDataFormatErrorCode::MissingField);
    assert_eq!(missing_path.path(), "$.sources[0].assetPath");

    let malformed = BibleCatalog::from_value(&json!({
        "English": {"broken": 7}
    }))
    .unwrap_err();
    assert_eq!(malformed.code(), BibleDataFormatErrorCode::InvalidType);
    assert_eq!(malformed.path(), "$.English.broken");

    let multiple_containers = BibleCatalog::from_value(&json!({
        "sources": [],
        "translations": []
    }))
    .unwrap_err();
    assert_eq!(
        multiple_containers.code(),
        BibleDataFormatErrorCode::InvalidValue
    );
    assert_eq!(multiple_containers.path(), "$");

    let duplicate =
        BibleCatalog::new(vec![source_fixture("same"), source_fixture("same")]).unwrap_err();
    assert_eq!(duplicate.code(), BibleDataFormatErrorCode::DuplicateId);
    assert_eq!(duplicate.path(), "$.sources[1].id");
}

#[test]
fn document_metadata_uses_nested_root_source_precedence_and_round_trips_extensions() {
    let document = json!({
        "schemaVersion": 1,
        "id": "root-id",
        "description": "root description",
        "license": "root license",
        "source": {
            "id": "source-id",
            "assetPath": "bibles/English/kjv.json",
            "languageName": "English",
            "languageCode": "en",
            "translationName": "Source name",
            "abbreviation": "SRC",
            "provider": {"level": "source"}
        },
        "metadata": {
            "editionId": "nested-id",
            "translationName": "Metadata name",
            "custom": {"revision": 2, "channel": "stable"}
        },
        "books": {}
    });
    let metadata = BibleMetadata::from_document_value(&document, None).unwrap();

    assert_eq!(metadata.id.as_deref(), Some("nested-id"));
    assert_eq!(metadata.description.as_deref(), Some("root description"));
    assert_eq!(metadata.translation_name.as_deref(), Some("Metadata name"));
    assert_eq!(metadata.license.as_deref(), Some("root license"));
    assert_eq!(metadata.language_code.as_deref(), Some("en"));
    assert_eq!(metadata.additional["schemaVersion"], 1);
    assert_eq!(metadata.additional["custom"]["revision"], 2);
    assert_eq!(
        metadata.source.as_ref().unwrap().additional["provider"]["level"],
        "source"
    );

    let restored: BibleMetadata = serde_json::from_value(metadata.to_json_value()).unwrap();
    assert_eq!(restored, metadata);
    assert_eq!(value_hash(&restored), value_hash(&metadata));

    let reordered = BibleMetadata::from_value(&json!({
        "custom": {"channel": "stable", "revision": 2},
        "schemaVersion": 1,
        "source": metadata.source.as_ref().unwrap().to_json_value(),
        "id": "nested-id",
        "description": "root description",
        "languageName": "English",
        "languageCode": "en",
        "translationName": "Metadata name",
        "abbreviation": "SRC",
        "direction": "auto",
        "license": "root license"
    }))
    .unwrap();
    assert_eq!(reordered, metadata);
    assert_eq!(value_hash(&reordered), value_hash(&metadata));
}

#[test]
fn supplied_source_overrides_embedded_source_and_merge_respects_explicit_metadata() {
    let supplied = BibleSource {
        id: "catalog-id".to_string(),
        asset_path: "catalog/Hebrew/wlc.json".to_string(),
        language_name: "Hebrew".to_string(),
        language_code: "he".to_string(),
        translation_name: "Catalog name".to_string(),
        abbreviation: "CAT".to_string(),
        direction: TextDirectionHint::Rtl,
        license: Some("source license".to_string()),
        ..source_fixture("unused")
    };
    let document = json!({
        "source": {
            "id": "embedded-id",
            "assetPath": "embedded/English/kjv.json",
            "languageName": "English",
            "languageCode": "en",
            "translationName": "Embedded name",
            "abbreviation": "EMB"
        }
    });
    let from_document = BibleMetadata::from_document_value(&document, Some(&supplied)).unwrap();
    assert_eq!(from_document.source.as_ref(), Some(&supplied));
    assert_eq!(from_document.id.as_deref(), Some("catalog-id"));
    assert_eq!(
        from_document.translation_name.as_deref(),
        Some("Catalog name")
    );

    let explicit = BibleMetadata {
        translation_name: Some("Display name".to_string()),
        license: Some("metadata license".to_string()),
        ..BibleMetadata::default()
    };
    let merged = merge_bible_metadata(Some(&explicit), Some(&supplied), None, None).unwrap();
    assert_eq!(merged.source.as_ref(), Some(&supplied));
    assert_eq!(merged.id.as_deref(), Some("catalog-id"));
    assert_eq!(merged.translation_name.as_deref(), Some("Display name"));
    assert_eq!(merged.language_name.as_deref(), Some("Hebrew"));
    assert_eq!(merged.direction, TextDirectionHint::Rtl);
    assert_eq!(merged.license.as_deref(), Some("metadata license"));
}

#[test]
fn metadata_rejects_reserved_extensions_and_invalid_dates() {
    let mut metadata = BibleMetadata::default();
    metadata.additional.insert(
        "translationName".to_string(),
        Value::String("hidden".to_string()),
    );
    let error = metadata.validate("$.metadata").unwrap_err();
    assert_eq!(error.code(), BibleDataFormatErrorCode::ReservedField);
    assert_eq!(error.path(), "$.metadata.translationName");

    let error = BibleMetadata::from_value(&json!({"date": "not-a-date"})).unwrap_err();
    assert_eq!(error.code(), BibleDataFormatErrorCode::InvalidValue);
    assert_eq!(error.path(), "$.metadata.date");
}
