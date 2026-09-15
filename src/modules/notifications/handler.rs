//! Handler notifications.
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::json;

use crate::middleware::auth::CurrentUser;
use crate::shared::error::AppError;
use crate::state::AppState;

fn dberr(e: sqlx::Error) -> AppError {
    tracing::error!("db: {e}");
    AppError::Internal(format!("db: {e}"))
}

#[derive(Deserialize)]
pub struct Q {
    pub unread_only: Option<bool>,
    pub limit: Option<usize>,
    pub cursor: Option<i64>,
}

pub async fn list(cu: CurrentUser, State(st): State<AppState>, Query(q): Query<Q>) -> Result<Response, AppError> {
    let limit = q.limit.unwrap_or(20).clamp(1, 100);
    let unread = q.unread_only.unwrap_or(false);
    let rows: Vec<(i64, Option<String>, String, String, Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT id, template_code, title, body, CAST(data AS CHAR), DATE_FORMAT(created_at, '%Y-%m-%dT%H:%i:%sZ'), DATE_FORMAT(read_at, '%Y-%m-%dT%H:%i:%sZ') \
         FROM user_notifications WHERE user_id = ? AND (? IS NULL OR id < ?) \
         AND (? = 0 OR read_at IS NULL) ORDER BY id DESC LIMIT ?")
        .bind(cu.user_id).bind(q.cursor).bind(q.cursor).bind(unread as i8).bind((limit + 1) as i64)
        .fetch_all(&st.pool).await.map_err(dberr)?;
    let has_more = rows.len() > limit;
    let items: Vec<serde_json::Value> = rows.iter().take(limit).map(|r| json!({
        "id": r.0, "template_code": r.1, "title": r.2, "body": r.3,
        "data": r.4.as_deref().and_then(|d| serde_json::from_str::<serde_json::Value>(d).ok()),
        "created_at": r.5,
        "read_at": r.6,
    })).collect();
    let next = if has_more { items.last().and_then(|i| i.get("id")).and_then(|v| v.as_i64()).map(|v| v.to_string()) } else { None };
    Ok((StatusCode::OK, Json(json!({
        "data": items,
        "meta": { "pagination": { "next_cursor": next, "has_more": has_more } }
    }))).into_response())
}

pub async fn read_one(cu: CurrentUser, State(st): State<AppState>, Path(id): Path<i64>) -> Result<Response, AppError> {
    let n = sqlx::query("UPDATE user_notifications SET read_at = UTC_TIMESTAMP() WHERE id = ? AND user_id = ? AND read_at IS NULL")
        .bind(id).bind(cu.user_id)
        .execute(&st.pool).await.map_err(dberr)?.rows_affected();
    if n == 0 { return Err(AppError::NotFound("notifikasi tidak ada".into())); }
    Ok((StatusCode::OK, Json(json!({ "data": { "read": true }, "meta": {} }))).into_response())
}

pub async fn read_all(cu: CurrentUser, State(st): State<AppState>) -> Result<Response, AppError> {
    sqlx::query("UPDATE user_notifications SET read_at = UTC_TIMESTAMP() WHERE user_id = ? AND read_at IS NULL")
        .bind(cu.user_id).execute(&st.pool).await.map_err(dberr)?;
    Ok((StatusCode::OK, Json(json!({ "data": { "read_all": true }, "meta": {} }))).into_response())
}
