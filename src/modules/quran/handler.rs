//! Handler modul quran.
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::json;

use crate::middleware::auth::CurrentUser;
use crate::modules::quran::dto::*;
use crate::modules::quran::service as svc;
use crate::shared::error::AppError;
use crate::state::AppState;

fn ok<T: serde::Serialize>(data: T, status: StatusCode) -> Response {
    (status, Json(json!({ "data": data, "meta": {} }))).into_response()
}
fn page<T: serde::Serialize>(p: crate::shared::pagination::CursorPage<T>) -> Response {
    (StatusCode::OK, Json(json!({
        "data": p.items,
        "meta": { "pagination": { "next_cursor": p.next_cursor, "has_more": p.has_more } }
    }))).into_response()
}

#[derive(Deserialize)]
pub struct AyahQ {
    #[serde(default = "default_translator")]
    pub translation: String,
    pub limit: Option<usize>,
    pub cursor: Option<i64>,
}
fn default_translator() -> String { "KEMENAG".into() }

#[derive(Deserialize)]
pub struct AudioQ {
    pub ayah_id: i64,
    pub reciter: Option<String>,
}

#[derive(Deserialize)]
pub struct PageQ {
    pub limit: Option<usize>,
    pub cursor: Option<i64>,
}

pub async fn surahs(State(st): State<AppState>) -> Result<Response, AppError> {
    // cache in-process 24h (keputusan #13 + modul 5)
    if let Some(cached) = st.cache_str.get(&"quran:surahs") {
        // body ter-cache SUDAH ber-envelope — parse & kirim apa adanya (jangan bungkus ganda)
        let body: serde_json::Value = serde_json::from_str(&cached).unwrap_or_default();
        return Ok((StatusCode::OK, Json(body)).into_response());
    }
    let list = svc::surahs(&st.pool).await?;
    let body = json!({ "data": list, "meta": {} });
    st.cache_str.insert("quran:surahs", std::sync::Arc::new(body.to_string()));
    Ok((StatusCode::OK, Json(body)).into_response())
}

pub async fn ayahs(State(st): State<AppState>, Path(id): Path<i64>, Query(q): Query<AyahQ>) -> Result<Response, AppError> {
    let limit = q.limit.unwrap_or(50).clamp(1, 200);
    let p = svc::ayahs(&st.pool, id, &q.translation, q.cursor, limit).await?;
    Ok(page(p))
}

pub async fn audio(State(st): State<AppState>, Query(q): Query<AudioQ>) -> Result<Response, AppError> {
    let a = svc::audio(&st.pool, q.ayah_id, q.reciter).await?;
    Ok(ok(a, StatusCode::OK))
}

pub async fn get_last_read(cu: CurrentUser, State(st): State<AppState>) -> Result<Response, AppError> {
    let r = svc::get_last_read(&st.pool, cu.user_id).await?;
    Ok(ok(r, StatusCode::OK))
}

pub async fn put_last_read(cu: CurrentUser, State(st): State<AppState>, Json(req): Json<PutLastReadReq>) -> Result<Response, AppError> {
    cu.require_active()?;
    svc::put_last_read(&st.pool, cu.user_id, req.ayah_id).await?;
    Ok(ok(json!({ "saved": true }), StatusCode::OK))
}

pub async fn add_bookmark(cu: CurrentUser, State(st): State<AppState>, Json(req): Json<AddBookmarkReq>) -> Result<Response, AppError> {
    cu.require_active()?;
    let b = svc::add_bookmark(&st.pool, cu.user_id, req.ayah_id, req.note).await?;
    Ok(ok(b, StatusCode::CREATED))
}

pub async fn list_bookmarks(cu: CurrentUser, State(st): State<AppState>, Query(q): Query<PageQ>) -> Result<Response, AppError> {
    let limit = q.limit.unwrap_or(20).clamp(1, 100);
    let p = svc::list_bookmarks(&st.pool, cu.user_id, q.cursor, limit).await?;
    Ok(page(p))
}

pub async fn delete_bookmark(cu: CurrentUser, State(st): State<AppState>, Path(ayah_id): Path<i64>) -> Result<Response, AppError> {
    svc::delete_bookmark(&st.pool, cu.user_id, ayah_id).await?;
    Ok(ok(json!({ "deleted": true }), StatusCode::OK))
}
