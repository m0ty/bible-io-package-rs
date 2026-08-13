use std::{
    collections::hash_map::DefaultHasher,
    error::Error,
    hash::{Hash, Hasher},
    io,
};

use bible_io::{
    BibleCatalog, BibleDataFormatErrorCode, BibleMetadata, BibleSource, TextDirectionHint,
};
use serde_json::{json, Value};

fn value_hash(value: &impl Hash) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

fn valid_source(id: &str) -> BibleSource {
    BibleSource::from_value(&json!({
        "id": id,
        "assetPath": format!("bibles/English/{id}.json"),
        "languageName": "English",
        "languageCode": "en",
        "translationName": "Example",
        "abbreviation": "EX",
        "extension": {"nested": [1, {"value": true}]}
    }))
    .unwrap()
}

#[test]
fn format_error_codes_have_stable_serialized_names() {
    let cases = [
        (BibleDataFormatErrorCode::InvalidJson, "invalid_json"),
        (BibleDataFormatErrorCode::InvalidType, "invalid_type"),
        (BibleDataFormatErrorCode::MissingField, "missing_field"),
        (BibleDataFormatErrorCode::InvalidValue, "invalid_value"),
        (BibleDataFormatErrorCode::DuplicateId, "duplicate_id"),
        (BibleDataFormatErrorCode::ReservedField, "reserved_field"),
        (BibleDataFormatErrorCode::NonJsonValue, "non_json_value"),
    ];
    for (code, name) in cases {
        assert_eq!(code.as_str(), name);
        assert_eq!(serde_json::to_value(code).unwrap(), name);
        assert_eq!(
            serde_json::from_value::<BibleDataFormatErrorCode>(json!(name)).unwrap(),
            code
        );
    }

    let error = bible_io::BibleDataFormatError::new(
        BibleDataFormatErrorCode::InvalidValue,
        "$.value",
        "custom failure",
    )
    .with_value(json!(7))
    .with_cause(io::Error::new(io::ErrorKind::InvalidData, "typed cause"));
    assert_eq!(error.cause(), Some("typed cause"));
    assert!(error.to_string().contains("typed cause"));
    assert_eq!(
        error
            .source()
            .and_then(|source| source.downcast_ref::<io::Error>())
            .map(io::Error::kind),
        Some(io::ErrorKind::InvalidData)
    );
}

#[test]
fn source_and_metadata_clones_deeply_separate_extension_values() {
    let source = valid_source("example");
    let mut cloned_source = source.clone();
    cloned_source.additional["extension"]["nested"][1]["value"] = json!(false);
    assert_eq!(source.additional["extension"]["nested"][1]["value"], true);
    assert_ne!(source, cloned_source);

    let metadata = BibleMetadata::from_value(&json!({
        "id": "example",
        "custom": {"nested": ["original"]}
    }))
    .unwrap();
    let equal = metadata.clone();
    assert_eq!(metadata, equal);
    assert_eq!(value_hash(&metadata), value_hash(&equal));

    let mut changed = equal;
    changed.additional["custom"]["nested"][0] = json!("changed");
    assert_eq!(metadata.additional["custom"]["nested"][0], "original");
    assert_ne!(metadata, changed);
}

#[test]
fn source_typed_copy_can_clear_nullable_fields_without_mutating_the_original() {
    let original = BibleSource {
        description: Some("Original description".to_string()),
        ..valid_source("edition")
    };
    let mut changed = original.clone();
    changed.description = None;
    changed.translation_name = "Changed translation".to_string();
    changed.validate("$.source").unwrap();

    assert_eq!(changed.id, "edition");
    assert_eq!(changed.description, None);
    assert_eq!(changed.translation_name, "Changed translation");
    assert_eq!(
        original.description.as_deref(),
        Some("Original description")
    );
    assert_eq!(original.translation_name, "Example");
}

#[test]
fn source_identity_and_catalog_identity_are_always_validated() {
    let mut root_blank = valid_source("valid");
    root_blank.id.clear();
    let error = root_blank.validate("$").unwrap_err();
    assert_eq!(error.code(), BibleDataFormatErrorCode::MissingField);
    assert_eq!(error.path(), "$.id");
    assert_eq!(error.value(), Some(&json!("")));

    for (id, expected_code) in [
        ("", BibleDataFormatErrorCode::MissingField),
        (" ", BibleDataFormatErrorCode::MissingField),
        (" padded ", BibleDataFormatErrorCode::InvalidValue),
    ] {
        let mut source = valid_source("valid");
        source.id = id.to_string();
        let error = source.validate("$.source").unwrap_err();
        assert_eq!(error.code(), expected_code);
        assert_eq!(error.path(), "$.source.id");
        assert_eq!(error.value(), Some(&json!(id)));
    }

    let duplicate =
        BibleCatalog::new(vec![valid_source("same"), valid_source("same")]).unwrap_err();
    assert_eq!(duplicate.code(), BibleDataFormatErrorCode::DuplicateId);
    assert_eq!(duplicate.path(), "$.sources[1].id");

    let catalog = BibleCatalog::new(vec![valid_source("Alpha"), valid_source("beta")]).unwrap();
    assert!(catalog.find_by_id("Alpha").is_some());
    assert!(catalog.find_by_id("alpha").is_none());
    assert_eq!(catalog.for_language("EN").len(), 2);
    assert_eq!(catalog.for_language("english").len(), 2);
}

#[test]
fn source_and_catalog_json_failures_retain_typed_causes() {
    let source_error = BibleSource::from_json("{").unwrap_err();
    assert_eq!(source_error.code(), BibleDataFormatErrorCode::InvalidJson);
    assert_eq!(source_error.path(), "$");
    assert!(source_error
        .source()
        .and_then(|cause| cause.downcast_ref::<serde_json::Error>())
        .is_some());

    let catalog_error = BibleCatalog::from_json("[").unwrap_err();
    assert_eq!(catalog_error.code(), BibleDataFormatErrorCode::InvalidJson);
    assert!(catalog_error
        .source()
        .and_then(|cause| cause.downcast_ref::<serde_json::Error>())
        .is_some());

    let utf8_error = BibleCatalog::from_json_slice(&[0xff]).unwrap_err();
    assert_eq!(utf8_error.code(), BibleDataFormatErrorCode::InvalidJson);
    assert!(utf8_error
        .source()
        .and_then(|cause| cause.downcast_ref::<std::str::Utf8Error>())
        .is_some());
}

#[test]
fn source_alias_types_and_reserved_extensions_fail_at_their_exact_paths() {
    let error = BibleSource::from_value(&json!({
        "id": "example",
        "assetPath": "example.json",
        "languageName": "English",
        "languageCode": "en",
        "translationName": "Example",
        "abbreviation": "EX",
        "year": true
    }))
    .unwrap_err();
    assert_eq!(error.code(), BibleDataFormatErrorCode::InvalidType);
    assert_eq!(error.path(), "$.year");
    assert_eq!(error.value(), Some(&Value::Bool(true)));

    let mut source = valid_source("example");
    source.additional.insert("id".to_string(), json!("hidden"));
    let error = source.validate("$.source").unwrap_err();
    assert_eq!(error.code(), BibleDataFormatErrorCode::ReservedField);
    assert_eq!(error.path(), "$.source.id");

    let metadata = BibleMetadata {
        direction: TextDirectionHint::Auto,
        id: Some(" example ".to_string()),
        ..BibleMetadata::default()
    };
    let error = metadata.validate("$.metadata").unwrap_err();
    assert_eq!(error.code(), BibleDataFormatErrorCode::InvalidValue);
    assert_eq!(error.path(), "$.metadata.id");
    assert_eq!(error.value(), Some(&json!(" example ")));
}

#[test]
fn catalog_accepts_all_documented_container_aliases_and_rejects_ambiguity() {
    for container in ["sources", "bibles", "translations"] {
        let catalog = BibleCatalog::from_value(&json!({
            container: [{
                "id": "example",
                "assetPath": "bibles/English/example.json",
                "languageName": "English",
                "languageCode": "en",
                "translationName": "Example",
                "abbreviation": "EX"
            }]
        }))
        .unwrap();
        assert_eq!(catalog.sources().len(), 1, "{container}");
    }

    let error = BibleCatalog::from_value(&json!({"sources": [], "bibles": []})).unwrap_err();
    assert_eq!(error.code(), BibleDataFormatErrorCode::InvalidValue);
    assert_eq!(error.path(), "$");

    let error = BibleCatalog::from_value(&json!({
        "sources": [{
            "id": "   ",
            "assetPath": "bibles/English/example.json",
            "languageName": "English",
            "languageCode": "en",
            "translationName": "Example",
            "abbreviation": "EX"
        }]
    }))
    .unwrap_err();
    assert_eq!(error.code(), BibleDataFormatErrorCode::InvalidValue);
    assert_eq!(error.path(), "$.sources[0].id");
    assert_eq!(error.value(), Some(&json!("   ")));
}
