//! Handler modul memorization.
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::json;

use crate::middleware::auth::CurrentUser;
use crate::modules::memorization::dto::*;
use crate::modules::memorization::service as svc;
use crate::modules::ustadz::service as ustadz_svc;
use crate::shared::error::AppError;
use crate::state::AppState;

fn ok<T: serde::Serialize>(data: T, status: StatusCode) -> Response {
    (status, Json(json!({ "data": data, "meta": {} }))).into_response()
}
fn page<T: serde::Serialize>(items: Vec<T>, next: Option<String>, has_more: bool) -> Response {
    (StatusCode::OK, Json(json!({
        "data": items,
        "meta": { "pagination": { "next_cursor": next, "has_more": has_more } }
    }))).into_response()
}

#[derive(Deserialize)]
pub struct PageQ {
    pub limit: Option<usize>,
    pub cursor: Option<i64>,
    pub status: Option<String>,
}

pub async fn submit(
    cu: CurrentUser, State(st): State<AppState>,
    headers: axum::http::HeaderMap, Json(req): Json<SubmitReq>,
) -> Result<Response, AppError> {
    cu.require("memorization.submit")?;
    let idem = headers.get("idempotency-key").and_then(|v| v.to_str().ok()).map(str::to_owned);
    let storage = st.storage.as_ref().ok_or_else(|| AppError::Internal("storage belum dikonfigurasi".into()))?;
    let _ = storage; // presign audio dipakai di detail
    let (sub, created) = svc::submit(&st.pool, cu.user_id, req, idem).await?;
    Ok(ok(sub, if created { StatusCode::CREATED } else { StatusCode::OK }))
}

pub async fn my_submissions(cu: CurrentUser, State(st): State<AppState>, Query(q): Query<PageQ>) -> Result<Response, AppError> {
    let limit = q.limit.unwrap_or(20).clamp(1, 100);
    let (items, next, more) = svc::my_submissions(&st.pool, cu.user_id, q.cursor, limit).await?;
    Ok(page(items, next, more))
}

pub async fn my_submission_detail(cu: CurrentUser, State(st): State<AppState>, Path(id): Path<i64>) -> Result<Response, AppError> {
    cu.require_active()?;
    let storage = st.storage.as_ref().ok_or_else(|| AppError::Internal("storage belum dikonfigurasi".into()))?;
    let can_review = cu.permissions.contains("memorization.review");
    let admin = cu.permissions.contains("users.read");
    let d = svc::detail(&st.pool, storage, cu.user_id, can_review, admin, id).await?;
    Ok(ok(d, StatusCode::OK))
}

pub async fn my_progress(cu: CurrentUser, State(st): State<AppState>) -> Result<Response, AppError> {
    let p = svc::my_progress(&st.pool, cu.user_id).await?;
    Ok(ok(p, StatusCode::OK))
}

pub async fn queue(cu: CurrentUser, State(st): State<AppState>, Query(q): Query<PageQ>) -> Result<Response, AppError> {
    cu.require("memorization.review")?;
    ustadz_svc::require_verified(&st.pool, cu.user_id).await?;
    let limit = q.limit.unwrap_or(20).clamp(1, 100);
    let (items, next, more) = svc::queue(&st.pool, cu.user_id, q.status, q.cursor, limit).await?;
    Ok(page(items, next, more))
}

pub async fn review(
    cu: CurrentUser, State(st): State<AppState>, Path(id): Path<i64>, Json(req): Json<ReviewReq>,
) -> Result<Response, AppError> {
    cu.require("memorization.review")?;
    ustadz_svc::require_verified(&st.pool, cu.user_id).await?;
    let storage = st.storage.as_ref().ok_or_else(|| AppError::Internal("storage belum dikonfigurasi".into()))?;
    let d = svc::review(&st.pool, storage, cu.user_id, id, req).await?;
    Ok(ok(d, StatusCode::OK))
}
