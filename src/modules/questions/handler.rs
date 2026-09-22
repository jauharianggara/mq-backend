//! Handler modul questions.
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::json;

use crate::middleware::auth::{CurrentUser, OptionalUser};
use crate::modules::questions::dto::*;
use crate::modules::questions::service as svc;
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
#[derive(Deserialize)]
pub struct ArchiveQ {
    pub q: Option<String>,
    pub category: Option<String>,
    pub limit: Option<usize>,
    pub cursor: Option<i64>,
}

fn idem(headers: &axum::http::HeaderMap) -> Option<String> {
    headers.get("idempotency-key").and_then(|v| v.to_str().ok()).map(str::to_owned)
}

pub async fn categories(State(st): State<AppState>) -> Result<Response, AppError> {
    Ok(ok(svc::categories(&st.pool).await?, StatusCode::OK))
}

pub async fn create(
    cu: CurrentUser, State(st): State<AppState>,
    headers: axum::http::HeaderMap, Json(req): Json<CreateQuestionReq>,
) -> Result<Response, AppError> {
    cu.require("question.create")?;
    let (t, created) = svc::create(&st.pool, cu.user_id, req, idem(&headers)).await?;
    Ok(ok(t, if created { StatusCode::CREATED } else { StatusCode::OK }))
}

pub async fn my_questions(cu: CurrentUser, State(st): State<AppState>, Query(q): Query<PageQ>) -> Result<Response, AppError> {
    let limit = q.limit.unwrap_or(20).clamp(1, 100);
    let (items, next, more) = svc::my_questions(&st.pool, cu.user_id, q.cursor, limit).await?;
    Ok(page(items, next, more))
}

pub async fn detail(
    OptionalUser(cu): OptionalUser, State(st): State<AppState>, Path(id): Path<i64>,
) -> Result<Response, AppError> {
    let (viewer, can_mod, can_admin) = match cu {
        Some(c) => (c.user_id, c.permissions.contains("question.moderate"), c.permissions.contains("users.read")),
        None => (0i64, false, false), // arsip PUBLISHED via OptionalUser publik
    };
    Ok(ok(svc::detail(&st.pool, st.storage.as_deref(), Some(&st), viewer, can_mod, can_admin, id).await?, StatusCode::OK))
}

pub async fn send_message(
    cu: CurrentUser, State(st): State<AppState>, Path(id): Path<i64>,
    headers: axum::http::HeaderMap, Json(req): Json<SendMessageReq>,
) -> Result<Response, AppError> {
    cu.require_active()?;
    let (m, created) = svc::send_message(&st.pool, st.storage.is_some(), cu.user_id, id, req, idem(&headers)).await?;
    Ok(ok(m, if created { StatusCode::CREATED } else { StatusCode::OK }))
}

pub async fn close(cu: CurrentUser, State(st): State<AppState>, Path(id): Path<i64>) -> Result<Response, AppError> {
    cu.require_active()?;
    svc::close(&st.pool, cu.user_id, id).await?;
    Ok(ok(json!({ "closed": true }), StatusCode::OK))
}

pub async fn answer(cu: CurrentUser, State(st): State<AppState>, Path(id): Path<i64>) -> Result<Response, AppError> {
    cu.require("question.answer")?;
    ustadz_svc::require_verified(&st.pool, cu.user_id).await?;
    svc::answer(&st.pool, cu.user_id, id).await?;
    Ok(ok(json!({ "answered": true }), StatusCode::OK))
}

pub async fn publish_request(cu: CurrentUser, State(st): State<AppState>, Path(id): Path<i64>) -> Result<Response, AppError> {
    cu.require("question.answer")?;
    svc::publish_request(&st.pool, cu.user_id, id).await?;
    Ok(ok(json!({ "requested": true }), StatusCode::OK))
}

pub async fn inbox(cu: CurrentUser, State(st): State<AppState>, Query(q): Query<PageQ>) -> Result<Response, AppError> {
    cu.require("question.answer")?;
    let limit = q.limit.unwrap_or(20).clamp(1, 100);
    let (items, next, more) = svc::inbox(&st.pool, cu.user_id, q.cursor, limit).await?;
    Ok(page(items, next, more))
}

pub async fn archive(State(st): State<AppState>, Query(q): Query<ArchiveQ>) -> Result<Response, AppError> {
    let limit = q.limit.unwrap_or(20).clamp(1, 100);
    let (items, next, more) = svc::archive(&st.pool, q.q, q.category, q.cursor, limit).await?;
    Ok(page(items, next, more))
}

pub async fn moderation_queue(cu: CurrentUser, State(st): State<AppState>, Query(q): Query<PageQ>) -> Result<Response, AppError> {
    cu.require("question.moderate")?;
    Ok(ok(svc::moderation_queue(&st.pool, q.status).await?, StatusCode::OK))
}

pub async fn publish(cu: CurrentUser, State(st): State<AppState>, Path(id): Path<i64>) -> Result<Response, AppError> {
    cu.require("question.publish.moderate")?;
    svc::publish(&st.pool, cu.user_id, id).await?;
    Ok(ok(json!({ "published": true }), StatusCode::OK))
}

pub async fn reject_publish(cu: CurrentUser, State(st): State<AppState>, Path(id): Path<i64>, body: Option<Json<RejectReq>>) -> Result<Response, AppError> {
    cu.require("question.publish.moderate")?;
    let reason = body.map(|Json(b)| b.reason).flatten();
    svc::reject_publish(&st.pool, cu.user_id, id, reason).await?;
    Ok(ok(json!({ "rejected": true }), StatusCode::OK))
}
