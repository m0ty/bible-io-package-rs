//! JSON-compatible extension values shared by content models.

use std::hash::{Hash, Hasher};

use serde_json::{Map, Value};

use crate::errors::ModelError;

/// Owned JSON object used for lossless annotations and metadata extensions.
pub type JsonMap = Map<String, Value>;

/// Validate that extension data does not shadow structural fields.
pub(crate) fn validate_annotations(
    annotations: &JsonMap,
    reserved: &[&str],
) -> Result<(), ModelError> {
    if let Some(key) = reserved.iter().find(|key| annotations.contains_key(**key)) {
        return Err(ModelError::new(
            "annotations",
            format!("must not contain the structural key \"{key}\""),
        ));
    }
    Ok(())
}

/// Hash JSON values structurally, sorting object keys so the result agrees
/// with JSON-object equality regardless of insertion order.
pub(crate) fn hash_json_map<H: Hasher>(values: &JsonMap, state: &mut H) {
    let mut entries = values.iter().collect::<Vec<_>>();
    entries.sort_unstable_by_key(|(key, _)| *key);
    entries.len().hash(state);
    for (key, value) in entries {
        key.hash(state);
        hash_json_value(value, state);
    }
}

fn hash_json_value<H: Hasher>(value: &Value, state: &mut H) {
    match value {
        Value::Null => 0_u8.hash(state),
        Value::Bool(value) => {
            1_u8.hash(state);
            value.hash(state);
        }
        Value::Number(value) => {
            2_u8.hash(state);
            value.to_string().hash(state);
        }
        Value::String(value) => {
            3_u8.hash(state);
            value.hash(state);
        }
        Value::Array(values) => {
            4_u8.hash(state);
            values.len().hash(state);
            for value in values {
                hash_json_value(value, state);
            }
        }
        Value::Object(values) => {
            5_u8.hash(state);
            hash_json_map(values, state);
        }
    }
}
