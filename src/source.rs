//! Translation metadata and reusable source catalogs.

use std::{
    collections::HashMap,
    fmt,
    hash::{Hash, Hasher},
    path::Path,
    str::FromStr,
};

use bible_io_references::Language;
use indexmap::IndexMap;
use serde::{de::Error as _, Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};

use crate::{
    errors::{BibleDataFormatError, BibleDataFormatErrorCode},
    json_value::hash_json_map,
};

/// Scripture text direction hint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TextDirectionHint {
    /// Let the consumer infer direction.
    #[default]
    Auto,
    /// Left-to-right text.
    Ltr,
    /// Right-to-left text.
    Rtl,
}

impl TextDirectionHint {
    /// Parse common direction spellings, defaulting unknown values to auto.
    #[must_use]
    pub fn from_name(value: Option<&str>) -> Self {
        match value.map(str::trim).map(str::to_ascii_lowercase).as_deref() {
            Some("ltr" | "left-to-right" | "left_to_right") => Self::Ltr,
            Some("rtl" | "right-to-left" | "right_to_left") => Self::Rtl,
            _ => Self::Auto,
        }
    }

    fn parse(value: &str, path: &str) -> Result<Self, BibleDataFormatError> {
        match value.trim().to_ascii_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "ltr" | "left-to-right" | "left_to_right" => Ok(Self::Ltr),
            "rtl" | "right-to-left" | "right_to_left" => Ok(Self::Rtl),
            _ => Err(BibleDataFormatError::new(
                BibleDataFormatErrorCode::InvalidValue,
                path,
                "text direction must be auto, ltr, or rtl",
            )
            .with_value(Value::String(value.to_string()))),
        }
    }
}

impl<'de> Deserialize<'de> for TextDirectionHint {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value, "$direction").map_err(D::Error::custom)
    }
}

/// Metadata for one loadable Bible source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BibleSource {
    /// Stable source/edition identifier.
    pub id: String,
    /// File, asset, or URL path.
    pub asset_path: String,
    /// Human-readable language name.
    pub language_name: String,
    /// ISO language code.
    pub language_code: String,
    /// Human-readable translation name.
    pub translation_name: String,
    /// Short translation label.
    pub abbreviation: String,
    /// Optional description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Optional publication year.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub year: Option<i32>,
    /// Text direction.
    pub direction: TextDirectionHint,
    /// Optional upstream source name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_name: Option<String>,
    /// Optional content copyright statement.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub copyright: Option<String>,
    /// Optional content license identifier or URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    /// Optional canon label.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canon: Option<String>,
    /// Optional ISO-8601 version date, preserved as text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version_date: Option<String>,
    /// Unknown source fields preserved across serialization.
    #[serde(flatten)]
    pub additional: Map<String, Value>,
}

impl BibleSource {
    /// Derive conventional metadata from an asset path.
    #[must_use]
    pub fn from_asset_path(asset_path: impl Into<String>) -> Self {
        Self::from_asset_path_with(asset_path.into(), None, None)
    }

    fn from_asset_path_with(
        asset_path: String,
        id: Option<&str>,
        language_name: Option<&str>,
    ) -> Self {
        let normalized = asset_path.replace('\\', "/");
        let segments: Vec<_> = normalized
            .split('/')
            .filter(|part| !part.is_empty())
            .collect();
        let file = segments.last().copied().unwrap_or(&normalized);
        let stem = Path::new(file)
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or(file);
        let language_name = language_name.map_or_else(
            || {
                if segments.len() > 1 {
                    label_from_segment(segments[segments.len() - 2])
                } else {
                    String::new()
                }
            },
            str::to_string,
        );
        let language_code = language_code_for_name(&language_name).unwrap_or_default();
        let abbreviation = stem.to_ascii_uppercase();
        let id = id.map_or_else(
            || sanitize_id(&format!("{language_name}_{abbreviation}")),
            str::to_string,
        );
        Self {
            id,
            asset_path,
            language_name,
            language_code: language_code.clone(),
            translation_name: label_from_segment(stem).to_ascii_uppercase(),
            abbreviation,
            description: None,
            year: None,
            direction: direction_for_language(&language_code),
            source_name: None,
            copyright: None,
            license: None,
            canon: None,
            version_date: None,
            additional: Map::new(),
        }
    }

    /// Parse a source object with aliases, inference, and path-aware errors.
    pub fn from_value(value: &Value) -> Result<Self, BibleDataFormatError> {
        Self::from_value_at(value, "$")
    }

    /// Parse a source from JSON text.
    pub fn from_json(input: &str) -> Result<Self, BibleDataFormatError> {
        let value = serde_json::from_str(input).map_err(|error| {
            BibleDataFormatError::new(
                BibleDataFormatErrorCode::InvalidJson,
                "$",
                "Bible source is not valid JSON",
            )
            .with_cause(error)
        })?;
        Self::from_value(&value)
    }

    fn from_value_at(value: &Value, path: &str) -> Result<Self, BibleDataFormatError> {
        let object = value.as_object().ok_or_else(|| {
            data_error(
                BibleDataFormatErrorCode::InvalidType,
                path,
                "Bible source must be an object",
                value,
            )
        })?;

        let asset_path = read_string(object, SOURCE_PATH_KEYS, path)?.unwrap_or_default();
        let fallback = (!asset_path.is_empty()).then(|| Self::from_asset_path(asset_path.clone()));
        let language_name =
            read_string(object, &["languageName", "language_name", "language"], path)?
                .or_else(|| fallback.as_ref().map(|source| source.language_name.clone()))
                .unwrap_or_default();
        let language_code = read_string(object, &["languageCode", "language_code", "lang"], path)?
            .or_else(|| fallback.as_ref().map(|source| source.language_code.clone()))
            .or_else(|| language_code_for_name(&language_name))
            .unwrap_or_default();
        let abbreviation = read_string(
            object,
            &["abbreviation", "abbr", "shortName", "short_name"],
            path,
        )?
        .or_else(|| fallback.as_ref().map(|source| source.abbreviation.clone()))
        .unwrap_or_default();
        let translation_name = read_string(
            object,
            &[
                "translationName",
                "translation_name",
                "name",
                "title",
                "version",
            ],
            path,
        )?
        .or_else(|| {
            fallback
                .as_ref()
                .map(|source| source.translation_name.clone())
        })
        .unwrap_or_else(|| abbreviation.clone());
        let direction = read_direction(object, path)?
            .or_else(|| fallback.as_ref().map(|source| source.direction))
            .unwrap_or_else(|| direction_for_language(&language_code));

        let source = Self {
            id: read_identifier(object, &["id", "key"], path)?
                .or_else(|| fallback.as_ref().map(|source| source.id.clone()))
                .unwrap_or_else(|| sanitize_id(&format!("{language_name}_{abbreviation}"))),
            asset_path,
            language_name,
            language_code,
            translation_name,
            abbreviation,
            description: read_string(object, &["description", "summary"], path)?,
            year: read_i32(object, &["year"], path)?,
            direction,
            source_name: read_string(object, &["sourceName", "source_name", "source"], path)?,
            copyright: read_string(object, &["copyright"], path)?,
            license: read_string(object, &["license"], path)?,
            canon: read_string(object, &["canon"], path)?,
            version_date: read_date(object, &["versionDate", "version_date", "date"], path)?,
            additional: additional_fields(object, SOURCE_RECOGNIZED_KEYS),
        };
        source.validate(path)?;
        Ok(source)
    }

    /// Validate required fields, canonical identity, dates, and extensions.
    pub fn validate(&self, path: &str) -> Result<(), BibleDataFormatError> {
        for (field, value) in [
            ("id", self.id.as_str()),
            ("assetPath", self.asset_path.as_str()),
            ("languageName", self.language_name.as_str()),
            ("languageCode", self.language_code.as_str()),
            ("translationName", self.translation_name.as_str()),
            ("abbreviation", self.abbreviation.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(BibleDataFormatError::new(
                    BibleDataFormatErrorCode::MissingField,
                    json_path(path, field),
                    format!("required source field {field} cannot be blank"),
                )
                .with_value(Value::String(value.to_string())));
            }
        }
        if self.id.trim() != self.id {
            return Err(BibleDataFormatError::new(
                BibleDataFormatErrorCode::InvalidValue,
                json_path(path, "id"),
                "source IDs cannot have surrounding whitespace",
            )
            .with_value(Value::String(self.id.clone())));
        }
        validate_optional_date(
            self.version_date.as_deref(),
            &json_path(path, "versionDate"),
        )?;
        validate_additional(&self.additional, METADATA_RECOGNIZED_KEYS, path)?;
        Ok(())
    }

    /// Return this source as a JSON object.
    #[must_use]
    pub fn to_json_value(&self) -> Value {
        serde_json::to_value(self).expect("BibleSource contains only JSON values")
    }
}

impl<'de> Deserialize<'de> for BibleSource {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        Self::from_value_at(&value, "$").map_err(D::Error::custom)
    }
}

impl Hash for BibleSource {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.asset_path.hash(state);
        self.language_name.hash(state);
        self.language_code.hash(state);
        self.translation_name.hash(state);
        self.abbreviation.hash(state);
        self.description.hash(state);
        self.year.hash(state);
        self.direction.hash(state);
        self.source_name.hash(state);
        self.copyright.hash(state);
        self.license.hash(state);
        self.canon.hash(state);
        self.version_date.hash(state);
        hash_json_map(&self.additional, state);
    }
}

/// Metadata attached to a loaded Bible instance.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BibleMetadata {
    /// Optional nested source provenance.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<BibleSource>,
    /// Stable edition ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Translation description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Human-readable language name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language_name: Option<String>,
    /// ISO language code.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language_code: Option<String>,
    /// Translation display name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub translation_name: Option<String>,
    /// Translation abbreviation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub abbreviation: Option<String>,
    /// Optional publication year.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub year: Option<i32>,
    /// Text direction.
    pub direction: TextDirectionHint,
    /// Optional upstream source name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_name: Option<String>,
    /// Optional content copyright.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub copyright: Option<String>,
    /// Optional content license.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    /// Optional canon label.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canon: Option<String>,
    /// Optional ISO-8601 version date, preserved as text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version_date: Option<String>,
    /// Unknown metadata fields preserved across serialization.
    #[serde(flatten)]
    pub additional: Map<String, Value>,
}

impl BibleMetadata {
    /// Parse a standalone metadata object.
    pub fn from_value(value: &Value) -> Result<Self, BibleDataFormatError> {
        let object = value.as_object().ok_or_else(|| {
            data_error(
                BibleDataFormatErrorCode::InvalidType,
                "$.metadata",
                "Bible metadata must be an object",
                value,
            )
        })?;
        Self::from_layers(Some(object), None, None, "$.metadata")
    }

    /// Read metadata from a complete Bible document.
    ///
    /// Values nested in `metadata` take precedence over legacy root values,
    /// followed by an explicitly supplied source, an embedded source, and
    /// inferred language fallbacks.
    pub fn from_document_value(
        document: &Value,
        supplied_source: Option<&BibleSource>,
    ) -> Result<Self, BibleDataFormatError> {
        let root = document.as_object().ok_or_else(|| {
            data_error(
                BibleDataFormatErrorCode::InvalidType,
                "$",
                "Bible JSON must have an object at its root",
                document,
            )
        })?;
        let nested = match root.get("metadata") {
            None | Some(Value::Null) => None,
            Some(Value::Object(object)) => Some(object),
            Some(value) => {
                return Err(data_error(
                    BibleDataFormatErrorCode::InvalidType,
                    "$.metadata",
                    "Bible metadata must be an object",
                    value,
                ));
            }
        };
        Self::from_layers(nested, Some(root), supplied_source, "$.metadata")
    }

    fn from_layers(
        nested: Option<&Map<String, Value>>,
        root: Option<&Map<String, Value>>,
        supplied_source: Option<&BibleSource>,
        path: &str,
    ) -> Result<Self, BibleDataFormatError> {
        let embedded = read_embedded_source(nested, root)?;
        let source = supplied_source.cloned().or(embedded);

        let language_name = read_layered_string(
            nested,
            root,
            &["languageName", "language_name", "language"],
            path,
        )?
        .or_else(|| source.as_ref().map(|source| source.language_name.clone()));
        let language_code = read_layered_string(
            nested,
            root,
            &["languageCode", "language_code", "lang"],
            path,
        )?
        .or_else(|| source.as_ref().map(|source| source.language_code.clone()))
        .or_else(|| language_name.as_deref().and_then(language_code_for_name));
        let explicit_direction = read_layered_direction(nested, root, path)?;
        let direction = explicit_direction
            .or_else(|| source.as_ref().map(|source| source.direction))
            .unwrap_or_else(|| {
                direction_for_language(language_code.as_deref().unwrap_or_default())
            });

        let mut additional = Map::new();
        if let Some(root) = root {
            additional.extend(additional_fields(root, ROOT_METADATA_RECOGNIZED_KEYS));
        }
        if let Some(nested) = nested {
            additional.extend(additional_fields(nested, METADATA_RECOGNIZED_KEYS));
        }

        let metadata = Self {
            source,
            id: read_layered_identifier(nested, root, &["id", "editionId", "edition_id"], path)?,
            description: read_layered_string(nested, root, &["description", "summary"], path)?,
            language_name,
            language_code,
            translation_name: read_layered_string(
                nested,
                root,
                &[
                    "translationName",
                    "translation_name",
                    "name",
                    "title",
                    "version",
                ],
                path,
            )?,
            abbreviation: read_layered_string(
                nested,
                root,
                &["abbreviation", "abbr", "shortName", "short_name"],
                path,
            )?,
            year: read_layered_i32(nested, root, &["year"], path)?,
            direction,
            source_name: read_layered_string(nested, root, &["sourceName", "source_name"], path)?,
            copyright: read_layered_string(nested, root, &["copyright"], path)?,
            license: read_layered_string(nested, root, &["license"], path)?,
            canon: read_layered_string(nested, root, &["canon"], path)?,
            version_date: read_layered_date(
                nested,
                root,
                &["versionDate", "version_date", "date"],
                path,
            )?,
            additional,
        };

        let mut metadata = fill_from_source(metadata);
        if metadata.language_code.is_none() {
            metadata.language_code = metadata
                .language_name
                .as_deref()
                .and_then(language_code_for_name);
        }
        metadata.validate(path)?;
        Ok(metadata)
    }

    /// Validate optional identity, nested source, dates, and extensions.
    pub fn validate(&self, path: &str) -> Result<(), BibleDataFormatError> {
        if let Some(id) = &self.id {
            if id.trim().is_empty() || id.trim() != id {
                return Err(BibleDataFormatError::new(
                    BibleDataFormatErrorCode::InvalidValue,
                    json_path(path, "id"),
                    "edition IDs must be non-blank and trimmed",
                )
                .with_value(Value::String(id.clone())));
            }
        }
        if let Some(source) = &self.source {
            source.validate(&json_path(path, "source"))?;
        }
        validate_optional_date(
            self.version_date.as_deref(),
            &json_path(path, "versionDate"),
        )?;
        validate_additional(&self.additional, METADATA_RECOGNIZED_KEYS, path)?;
        Ok(())
    }

    /// Return this metadata as a JSON object.
    #[must_use]
    pub fn to_json_value(&self) -> Value {
        serde_json::to_value(self).expect("BibleMetadata contains only JSON values")
    }
}

impl<'de> Deserialize<'de> for BibleMetadata {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        Self::from_value(&value).map_err(D::Error::custom)
    }
}

impl Hash for BibleMetadata {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.source.hash(state);
        self.id.hash(state);
        self.description.hash(state);
        self.language_name.hash(state);
        self.language_code.hash(state);
        self.translation_name.hash(state);
        self.abbreviation.hash(state);
        self.year.hash(state);
        self.direction.hash(state);
        self.source_name.hash(state);
        self.copyright.hash(state);
        self.license.hash(state);
        self.canon.hash(state);
        self.version_date.hash(state);
        hash_json_map(&self.additional, state);
    }
}

/// Merge explicit metadata, source data, and language fallbacks.
pub fn merge_bible_metadata(
    metadata: Option<&BibleMetadata>,
    source: Option<&BibleSource>,
    fallback_language_name: Option<&str>,
    fallback_language_code: Option<&str>,
) -> Result<BibleMetadata, BibleDataFormatError> {
    let effective_source = source
        .cloned()
        .or_else(|| metadata.and_then(|value| value.source.clone()));
    let metadata_direction = metadata.map(|value| value.direction);
    let direction = match metadata_direction {
        Some(direction) if direction != TextDirectionHint::Auto => direction,
        _ => effective_source
            .as_ref()
            .map(|source| source.direction)
            .or(metadata_direction)
            .unwrap_or_else(|| {
                direction_for_language(
                    metadata
                        .and_then(|value| value.language_code.as_deref())
                        .or_else(|| {
                            effective_source
                                .as_ref()
                                .map(|value| value.language_code.as_str())
                        })
                        .or(fallback_language_code)
                        .unwrap_or_default(),
                )
            }),
    };

    let merged = BibleMetadata {
        source: effective_source.clone(),
        id: metadata
            .and_then(|value| value.id.clone())
            .or_else(|| effective_source.as_ref().map(|value| value.id.clone())),
        description: metadata
            .and_then(|value| value.description.clone())
            .or_else(|| {
                effective_source
                    .as_ref()
                    .and_then(|value| value.description.clone())
            }),
        language_name: metadata
            .and_then(|value| value.language_name.clone())
            .or_else(|| {
                effective_source
                    .as_ref()
                    .map(|value| value.language_name.clone())
            })
            .or_else(|| fallback_language_name.map(str::to_string)),
        language_code: metadata
            .and_then(|value| value.language_code.clone())
            .or_else(|| {
                effective_source
                    .as_ref()
                    .map(|value| value.language_code.clone())
            })
            .or_else(|| fallback_language_code.map(str::to_string)),
        translation_name: metadata
            .and_then(|value| value.translation_name.clone())
            .or_else(|| {
                effective_source
                    .as_ref()
                    .map(|value| value.translation_name.clone())
            }),
        abbreviation: metadata
            .and_then(|value| value.abbreviation.clone())
            .or_else(|| {
                effective_source
                    .as_ref()
                    .map(|value| value.abbreviation.clone())
            }),
        year: metadata
            .and_then(|value| value.year)
            .or_else(|| effective_source.as_ref().and_then(|value| value.year)),
        direction,
        source_name: metadata
            .and_then(|value| value.source_name.clone())
            .or_else(|| {
                effective_source
                    .as_ref()
                    .and_then(|value| value.source_name.clone())
            }),
        copyright: metadata
            .and_then(|value| value.copyright.clone())
            .or_else(|| {
                effective_source
                    .as_ref()
                    .and_then(|value| value.copyright.clone())
            }),
        license: metadata
            .and_then(|value| value.license.clone())
            .or_else(|| {
                effective_source
                    .as_ref()
                    .and_then(|value| value.license.clone())
            }),
        canon: metadata.and_then(|value| value.canon.clone()).or_else(|| {
            effective_source
                .as_ref()
                .and_then(|value| value.canon.clone())
        }),
        version_date: metadata
            .and_then(|value| value.version_date.clone())
            .or_else(|| {
                effective_source
                    .as_ref()
                    .and_then(|value| value.version_date.clone())
            }),
        additional: metadata.map_or_else(Map::new, |value| value.additional.clone()),
    };
    merged.validate("$.metadata")?;
    Ok(merged)
}

/// Collection of available Bible sources indexed by stable ID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BibleCatalog {
    sources: Vec<BibleSource>,
    by_id: HashMap<String, usize>,
}

impl Hash for BibleCatalog {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.sources.hash(state);
    }
}

impl BibleCatalog {
    /// Construct a catalog and reject duplicate or invalid sources.
    pub fn new(sources: Vec<BibleSource>) -> Result<Self, BibleDataFormatError> {
        let mut by_id = HashMap::new();
        for (index, source) in sources.iter().enumerate() {
            let path = format!("$.sources[{index}]");
            source.validate(&path)?;
            if by_id.insert(source.id.clone(), index).is_some() {
                return Err(BibleDataFormatError::new(
                    BibleDataFormatErrorCode::DuplicateId,
                    format!("{path}.id"),
                    "Bible source IDs must be unique",
                ));
            }
        }
        Ok(Self { sources, by_id })
    }

    /// Parse a catalog from JSON text.
    pub fn from_json(input: &str) -> Result<Self, BibleDataFormatError> {
        let value: Value = serde_json::from_str(input).map_err(|error| {
            BibleDataFormatError::new(
                BibleDataFormatErrorCode::InvalidJson,
                "$",
                "catalog is not valid JSON",
            )
            .with_cause(error)
        })?;
        Self::from_value(&value)
    }

    /// Parse a catalog from UTF-8 JSON bytes.
    pub fn from_json_slice(input: &[u8]) -> Result<Self, BibleDataFormatError> {
        let text = std::str::from_utf8(input).map_err(|error| {
            BibleDataFormatError::new(
                BibleDataFormatErrorCode::InvalidJson,
                "$",
                "catalog is not valid UTF-8",
            )
            .with_cause(error)
        })?;
        Self::from_json(text)
    }

    /// Parse a list, container, path, ID map, or nested language map.
    pub fn from_value(value: &Value) -> Result<Self, BibleDataFormatError> {
        let mut sources = Vec::new();
        parse_catalog_value(value, "$", None, None, false, &mut sources)?;
        Self::new(sources)
    }

    /// Return every source in catalog order.
    #[must_use]
    pub fn sources(&self) -> &[BibleSource] {
        &self.sources
    }

    /// Find a source by exact stable ID.
    #[must_use]
    pub fn find_by_id(&self, id: &str) -> Option<&BibleSource> {
        self.by_id.get(id).map(|index| &self.sources[*index])
    }

    /// Find sources by case-insensitive language name or code.
    #[must_use]
    pub fn for_language(&self, language: &str) -> Vec<&BibleSource> {
        self.sources
            .iter()
            .filter(|source| {
                source.language_name.eq_ignore_ascii_case(language.trim())
                    || source.language_code.eq_ignore_ascii_case(language.trim())
            })
            .collect()
    }

    /// Group sources by their display language in catalog order.
    #[must_use]
    pub fn by_language_name(&self) -> IndexMap<&str, Vec<&BibleSource>> {
        let mut grouped = IndexMap::new();
        for source in &self.sources {
            grouped
                .entry(source.language_name.as_str())
                .or_insert_with(Vec::new)
                .push(source);
        }
        grouped
    }
}

fn parse_catalog_value(
    value: &Value,
    path: &str,
    language: Option<&str>,
    id: Option<&str>,
    expect_source: bool,
    output: &mut Vec<BibleSource>,
) -> Result<(), BibleDataFormatError> {
    match value {
        Value::String(asset_path) => {
            if asset_path.trim().is_empty() {
                return Err(data_error(
                    BibleDataFormatErrorCode::InvalidValue,
                    path,
                    "Bible source asset path cannot be blank",
                    value,
                ));
            }
            let source = BibleSource::from_asset_path_with(asset_path.clone(), id, language);
            push_catalog_source(source, path, output)
        }
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                parse_catalog_value(
                    item,
                    &format!("{path}[{index}]"),
                    language,
                    None,
                    true,
                    output,
                )?;
            }
            Ok(())
        }
        Value::Object(object) => {
            if expect_source || looks_like_source(object) {
                let mut source_value = object.clone();
                if let Some(id) = id {
                    source_value
                        .entry("id".to_string())
                        .or_insert_with(|| Value::String(id.to_string()));
                }
                if let Some(language) = language {
                    source_value
                        .entry("languageName".to_string())
                        .or_insert_with(|| Value::String(language.to_string()));
                }
                let source = BibleSource::from_value_at(&Value::Object(source_value), path)?;
                return push_catalog_source(source, path, output);
            }

            let containers: Vec<_> = ["sources", "bibles", "translations"]
                .into_iter()
                .filter(|key| object.contains_key(*key))
                .collect();
            if containers.len() > 1 {
                return Err(BibleDataFormatError::new(
                    BibleDataFormatErrorCode::InvalidValue,
                    path,
                    "catalog must use only one source container key",
                ));
            }
            if let Some(container) = containers.first() {
                return parse_catalog_value(
                    &object[*container],
                    &json_path(path, container),
                    language,
                    None,
                    false,
                    output,
                );
            }

            for (key, child) in object {
                let child_path = json_path(path, key);
                match child {
                    Value::String(_) => {
                        parse_catalog_value(
                            child,
                            &child_path,
                            language,
                            Some(key),
                            false,
                            output,
                        )?;
                    }
                    Value::Array(_) => {
                        parse_catalog_value(
                            child,
                            &child_path,
                            language.or(Some(key)),
                            None,
                            false,
                            output,
                        )?;
                    }
                    Value::Object(child_object) if looks_like_source(child_object) => {
                        parse_catalog_value(
                            child,
                            &child_path,
                            language,
                            Some(key),
                            false,
                            output,
                        )?;
                    }
                    Value::Object(_) => {
                        parse_catalog_value(
                            child,
                            &child_path,
                            language.or(Some(key)),
                            None,
                            false,
                            output,
                        )?;
                    }
                    _ => {
                        return Err(data_error(
                            BibleDataFormatErrorCode::InvalidType,
                            &child_path,
                            "catalog entry must be a source object, list, or path",
                            child,
                        ));
                    }
                }
            }
            Ok(())
        }
        _ => Err(data_error(
            BibleDataFormatErrorCode::InvalidType,
            path,
            "catalog entries must be source objects, lists, or paths",
            value,
        )),
    }
}

fn push_catalog_source(
    source: BibleSource,
    path: &str,
    output: &mut Vec<BibleSource>,
) -> Result<(), BibleDataFormatError> {
    source.validate(path)?;
    output.push(source);
    Ok(())
}

fn read_embedded_source(
    nested: Option<&Map<String, Value>>,
    root: Option<&Map<String, Value>>,
) -> Result<Option<BibleSource>, BibleDataFormatError> {
    if let Some(nested) = nested {
        if let Some(value) = nested.get("source") {
            if !value.is_null() {
                return BibleSource::from_value_at(value, "$.metadata.source").map(Some);
            }
        }
    }
    if let Some(root) = root {
        if let Some(value) = root.get("source") {
            if !value.is_null() {
                return BibleSource::from_value_at(value, "$.source").map(Some);
            }
        }
    }
    Ok(None)
}

fn fill_from_source(mut metadata: BibleMetadata) -> BibleMetadata {
    let Some(source) = metadata.source.as_ref() else {
        return metadata;
    };
    metadata.id.get_or_insert_with(|| source.id.clone());
    if metadata.description.is_none() {
        metadata.description = source.description.clone();
    }
    metadata
        .language_name
        .get_or_insert_with(|| source.language_name.clone());
    metadata
        .language_code
        .get_or_insert_with(|| source.language_code.clone());
    metadata
        .translation_name
        .get_or_insert_with(|| source.translation_name.clone());
    metadata
        .abbreviation
        .get_or_insert_with(|| source.abbreviation.clone());
    if metadata.year.is_none() {
        metadata.year = source.year;
    }
    if metadata.source_name.is_none() {
        metadata.source_name = source.source_name.clone();
    }
    if metadata.copyright.is_none() {
        metadata.copyright = source.copyright.clone();
    }
    if metadata.license.is_none() {
        metadata.license = source.license.clone();
    }
    if metadata.canon.is_none() {
        metadata.canon = source.canon.clone();
    }
    if metadata.version_date.is_none() {
        metadata.version_date = source.version_date.clone();
    }
    metadata
}

fn read_layered_string(
    nested: Option<&Map<String, Value>>,
    root: Option<&Map<String, Value>>,
    keys: &[&str],
    path: &str,
) -> Result<Option<String>, BibleDataFormatError> {
    if let Some(nested) = nested {
        if let Some(value) = read_string(nested, keys, path)? {
            return Ok(Some(value));
        }
    }
    root.map_or(Ok(None), |root| read_string(root, keys, "$"))
}

fn read_layered_identifier(
    nested: Option<&Map<String, Value>>,
    root: Option<&Map<String, Value>>,
    keys: &[&str],
    path: &str,
) -> Result<Option<String>, BibleDataFormatError> {
    if let Some(nested) = nested {
        if let Some(value) = read_identifier(nested, keys, path)? {
            return Ok(Some(value));
        }
    }
    root.map_or(Ok(None), |root| read_identifier(root, keys, "$"))
}

fn read_layered_i32(
    nested: Option<&Map<String, Value>>,
    root: Option<&Map<String, Value>>,
    keys: &[&str],
    path: &str,
) -> Result<Option<i32>, BibleDataFormatError> {
    if let Some(nested) = nested {
        if let Some(value) = read_i32(nested, keys, path)? {
            return Ok(Some(value));
        }
    }
    root.map_or(Ok(None), |root| read_i32(root, keys, "$"))
}

fn read_layered_date(
    nested: Option<&Map<String, Value>>,
    root: Option<&Map<String, Value>>,
    keys: &[&str],
    path: &str,
) -> Result<Option<String>, BibleDataFormatError> {
    if let Some(nested) = nested {
        if let Some(value) = read_date(nested, keys, path)? {
            return Ok(Some(value));
        }
    }
    root.map_or(Ok(None), |root| read_date(root, keys, "$"))
}

fn read_layered_direction(
    nested: Option<&Map<String, Value>>,
    root: Option<&Map<String, Value>>,
    path: &str,
) -> Result<Option<TextDirectionHint>, BibleDataFormatError> {
    if let Some(nested) = nested {
        if let Some(value) = read_direction(nested, path)? {
            return Ok(Some(value));
        }
    }
    root.map_or(Ok(None), |root| read_direction(root, "$"))
}

fn read_string(
    object: &Map<String, Value>,
    keys: &[&str],
    path: &str,
) -> Result<Option<String>, BibleDataFormatError> {
    for key in keys {
        let Some(value) = object.get(*key) else {
            continue;
        };
        if value.is_null() {
            continue;
        }
        let Some(value) = value.as_str() else {
            return Err(data_error(
                BibleDataFormatErrorCode::InvalidType,
                &json_path(path, key),
                "expected a string",
                value,
            ));
        };
        if value.trim().is_empty() {
            return Err(data_error(
                BibleDataFormatErrorCode::InvalidValue,
                &json_path(path, key),
                "string value cannot be blank",
                &Value::String(value.to_string()),
            ));
        }
        return Ok(Some(value.trim().to_string()));
    }
    Ok(None)
}

fn read_identifier(
    object: &Map<String, Value>,
    keys: &[&str],
    path: &str,
) -> Result<Option<String>, BibleDataFormatError> {
    for key in keys {
        let Some(value) = object.get(*key) else {
            continue;
        };
        if value.is_null() {
            continue;
        }
        let Some(value) = value.as_str() else {
            return Err(data_error(
                BibleDataFormatErrorCode::InvalidType,
                &json_path(path, key),
                "expected an identifier string",
                value,
            ));
        };
        if value.trim().is_empty() || value.trim() != value {
            return Err(data_error(
                BibleDataFormatErrorCode::InvalidValue,
                &json_path(path, key),
                "identifiers must be non-blank and have no surrounding whitespace",
                &Value::String(value.to_string()),
            ));
        }
        return Ok(Some(value.to_string()));
    }
    Ok(None)
}

fn read_i32(
    object: &Map<String, Value>,
    keys: &[&str],
    path: &str,
) -> Result<Option<i32>, BibleDataFormatError> {
    for key in keys {
        let Some(value) = object.get(*key) else {
            continue;
        };
        if value.is_null() {
            continue;
        }
        let result = match value {
            Value::Number(number) if number.is_i64() => {
                number.as_i64().and_then(|value| i32::try_from(value).ok())
            }
            Value::Number(number) if number.is_u64() => {
                number.as_u64().and_then(|value| i32::try_from(value).ok())
            }
            Value::Number(_) => {
                return Err(data_error(
                    BibleDataFormatErrorCode::InvalidType,
                    &json_path(path, key),
                    "expected an integer",
                    value,
                ));
            }
            Value::String(value) => value.trim().parse::<i32>().ok(),
            _ => {
                return Err(data_error(
                    BibleDataFormatErrorCode::InvalidType,
                    &json_path(path, key),
                    "expected an integer",
                    value,
                ));
            }
        };
        return result.map(Some).ok_or_else(|| {
            data_error(
                BibleDataFormatErrorCode::InvalidValue,
                &json_path(path, key),
                "expected an integer value",
                value,
            )
        });
    }
    Ok(None)
}

fn read_direction(
    object: &Map<String, Value>,
    path: &str,
) -> Result<Option<TextDirectionHint>, BibleDataFormatError> {
    let keys = ["direction", "textDirection", "text_direction"];
    for key in keys {
        let Some(value) = object.get(key) else {
            continue;
        };
        if value.is_null() {
            continue;
        }
        let Some(value) = value.as_str() else {
            return Err(data_error(
                BibleDataFormatErrorCode::InvalidType,
                &json_path(path, key),
                "expected a string",
                value,
            ));
        };
        return TextDirectionHint::parse(value, &json_path(path, key)).map(Some);
    }
    Ok(None)
}

fn read_date(
    object: &Map<String, Value>,
    keys: &[&str],
    path: &str,
) -> Result<Option<String>, BibleDataFormatError> {
    let value = read_string(object, keys, path)?;
    if let Some(value) = &value {
        let key = keys
            .iter()
            .find(|key| object.get(**key).is_some_and(|value| !value.is_null()))
            .copied()
            .unwrap_or(keys[0]);
        validate_optional_date(Some(value), &json_path(path, key))?;
    }
    Ok(value)
}

fn validate_optional_date(value: Option<&str>, path: &str) -> Result<(), BibleDataFormatError> {
    if let Some(value) = value {
        if !is_iso_8601(value) {
            return Err(BibleDataFormatError::new(
                BibleDataFormatErrorCode::InvalidValue,
                path,
                "expected an ISO-8601 date",
            )
            .with_value(Value::String(value.to_string())));
        }
    }
    Ok(())
}

fn is_iso_8601(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() < 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || !bytes[..4].iter().all(u8::is_ascii_digit)
        || !bytes[5..7].iter().all(u8::is_ascii_digit)
        || !bytes[8..10].iter().all(u8::is_ascii_digit)
    {
        return false;
    }
    let year = value[..4].parse::<i32>().ok();
    let month = value[5..7].parse::<u8>().ok();
    let day = value[8..10].parse::<u8>().ok();
    let (Some(year), Some(month), Some(day)) = (year, month, day) else {
        return false;
    };
    let maximum_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if year % 400 == 0 || (year % 4 == 0 && year % 100 != 0) => 29,
        2 => 28,
        _ => return false,
    };
    if day == 0 || day > maximum_day {
        return false;
    }
    if bytes.len() == 10 {
        return true;
    }
    if !matches!(bytes[10], b'T' | b' ') || bytes.len() < 16 {
        return false;
    }
    let time = &value[11..];
    let time_bytes = time.as_bytes();
    let hour = time.get(..2).and_then(|part| part.parse::<u8>().ok());
    let minute = time.get(3..5).and_then(|part| part.parse::<u8>().ok());
    if !matches!(
        (hour, time_bytes.get(2), minute),
        (Some(0..=23), Some(b':'), Some(0..=59))
    ) {
        return false;
    }

    let mut remainder = &time[5..];
    if let Some(after_colon) = remainder.strip_prefix(':') {
        let Some(seconds) = after_colon.get(..2) else {
            return false;
        };
        if !seconds.bytes().all(|byte| byte.is_ascii_digit())
            || seconds
                .parse::<u8>()
                .ok()
                .is_none_or(|seconds| seconds > 59)
        {
            return false;
        }
        remainder = &after_colon[2..];
        if let Some(after_dot) = remainder.strip_prefix('.') {
            let digit_count = after_dot
                .bytes()
                .take_while(|byte| byte.is_ascii_digit())
                .count();
            if digit_count == 0 {
                return false;
            }
            remainder = &after_dot[digit_count..];
        }
    }

    if remainder.is_empty() || matches!(remainder, "Z" | "z") {
        return true;
    }
    let Some(offset) = remainder
        .strip_prefix('+')
        .or_else(|| remainder.strip_prefix('-'))
    else {
        return false;
    };
    if offset.len() != 5 || offset.as_bytes().get(2) != Some(&b':') {
        return false;
    }
    let offset_hour = offset[..2].parse::<u8>().ok();
    let offset_minute = offset[3..].parse::<u8>().ok();
    matches!((offset_hour, offset_minute), (Some(0..=23), Some(0..=59)))
}

fn validate_additional(
    additional: &Map<String, Value>,
    reserved: &[&str],
    path: &str,
) -> Result<(), BibleDataFormatError> {
    if let Some(key) = reserved.iter().find(|key| additional.contains_key(**key)) {
        return Err(BibleDataFormatError::new(
            BibleDataFormatErrorCode::ReservedField,
            json_path(path, key),
            "recognized metadata fields cannot be stored as extensions",
        )
        .with_value(additional[*key].clone()));
    }
    Ok(())
}

fn additional_fields(object: &Map<String, Value>, recognized: &[&str]) -> Map<String, Value> {
    object
        .iter()
        .filter(|(key, _)| !recognized.contains(&key.as_str()))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

fn looks_like_source(object: &Map<String, Value>) -> bool {
    object
        .keys()
        .any(|key| SOURCE_RECOGNIZED_KEYS.contains(&key.as_str()))
}

fn language_code_for_name(language_name: &str) -> Option<String> {
    if let Ok(language) = Language::from_str(language_name) {
        if !language.is_auto() {
            return Some(language.code().to_string());
        }
    }
    language_name
        .trim()
        .eq_ignore_ascii_case("italian")
        .then(|| "it".to_string())
}

fn label_from_segment(value: &str) -> String {
    value
        .split(['_', '-'])
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut characters = word.chars();
            characters.next().map_or_else(String::new, |first| {
                first.to_uppercase().collect::<String>() + &characters.as_str().to_lowercase()
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn sanitize_id(value: &str) -> String {
    let mut result = String::new();
    let mut separator = false;
    for character in value.to_ascii_lowercase().chars() {
        if character.is_ascii_alphanumeric() {
            if separator && !result.is_empty() {
                result.push('_');
            }
            result.push(character);
            separator = false;
        } else {
            separator = true;
        }
    }
    if result.is_empty() {
        "bible_source".to_string()
    } else {
        result
    }
}

fn direction_for_language(code: &str) -> TextDirectionHint {
    match code.trim().to_ascii_lowercase().as_str() {
        "ar" | "fa" | "he" | "ur" => TextDirectionHint::Rtl,
        _ => TextDirectionHint::Auto,
    }
}

fn data_error(
    code: BibleDataFormatErrorCode,
    path: &str,
    message: &str,
    value: &Value,
) -> BibleDataFormatError {
    BibleDataFormatError::new(code, path, message).with_value(value.clone())
}

fn json_path(base: &str, key: &str) -> String {
    if is_simple_key(key) {
        format!("{base}.{key}")
    } else {
        format!(
            "{base}[{}]",
            serde_json::to_string(key).expect("a string always serializes")
        )
    }
}

fn is_simple_key(key: &str) -> bool {
    let mut characters = key.chars();
    match characters.next() {
        Some(first) if first == '_' || first.is_ascii_alphabetic() => {}
        _ => return false,
    }
    characters.all(|character| character == '_' || character.is_ascii_alphanumeric())
}

const SOURCE_PATH_KEYS: &[&str] = &["assetPath", "asset_path", "path", "file", "url"];

const SOURCE_RECOGNIZED_KEYS: &[&str] = &[
    "assetPath",
    "asset_path",
    "path",
    "file",
    "url",
    "id",
    "key",
    "languageName",
    "language_name",
    "language",
    "languageCode",
    "language_code",
    "lang",
    "translationName",
    "translation_name",
    "name",
    "title",
    "version",
    "abbreviation",
    "abbr",
    "shortName",
    "short_name",
    "description",
    "summary",
    "year",
    "direction",
    "textDirection",
    "text_direction",
    "sourceName",
    "source_name",
    "source",
    "copyright",
    "license",
    "canon",
    "versionDate",
    "version_date",
    "date",
];

const METADATA_RECOGNIZED_KEYS: &[&str] = &[
    "assetPath",
    "asset_path",
    "path",
    "file",
    "url",
    "id",
    "key",
    "editionId",
    "edition_id",
    "languageName",
    "language_name",
    "language",
    "languageCode",
    "language_code",
    "lang",
    "translationName",
    "translation_name",
    "name",
    "title",
    "version",
    "abbreviation",
    "abbr",
    "shortName",
    "short_name",
    "description",
    "summary",
    "year",
    "direction",
    "textDirection",
    "text_direction",
    "sourceName",
    "source_name",
    "source",
    "copyright",
    "license",
    "canon",
    "versionDate",
    "version_date",
    "date",
];

const ROOT_METADATA_RECOGNIZED_KEYS: &[&str] = &[
    "assetPath",
    "asset_path",
    "path",
    "file",
    "url",
    "id",
    "key",
    "editionId",
    "edition_id",
    "languageName",
    "language_name",
    "language",
    "languageCode",
    "language_code",
    "lang",
    "translationName",
    "translation_name",
    "name",
    "title",
    "version",
    "abbreviation",
    "abbr",
    "shortName",
    "short_name",
    "description",
    "summary",
    "year",
    "direction",
    "textDirection",
    "text_direction",
    "sourceName",
    "source_name",
    "source",
    "copyright",
    "license",
    "canon",
    "versionDate",
    "version_date",
    "date",
    "books",
    "metadata",
];

impl fmt::Display for BibleSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} ({})",
            self.translation_name, self.abbreviation
        )
    }
}
