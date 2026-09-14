//! Envelope sukses `{data, meta}` (Task 0.4).
use axum::Json;
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Serialize)]
pub struct Meta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pagination: Option<Value>,
}

impl Default for Meta {
    fn default() -> Self {
        Self { pagination: None }
    }
}

/// Response sukses standar: `{ "data": ..., "meta": ... }`
pub fn ok<T: Serialize>(data: T) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "data": serde_json::to_value(data).unwrap_or(Value::Null),
        "meta": { },
    }))
}

/// Response sukses + meta pagination.
pub fn ok_with_meta<T: Serialize>(data: T, meta: Meta) -> Json<serde_json::Value> {
    let pagination = meta.pagination.unwrap_or(Value::Null);
    Json(serde_json::json!({
        "data": serde_json::to_value(data).unwrap_or(Value::Null),
        "meta": { "pagination": pagination },
    }))
}
