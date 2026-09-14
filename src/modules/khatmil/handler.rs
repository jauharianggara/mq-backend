//! Handler modul khatmil.
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::json;

use crate::middleware::auth::CurrentUser;
use crate::modules::khatmil::dto::*;
use crate::modules::khatmil::service as svc;
use crate::shared::error::AppError;
use crate::state::AppState;

fn ok<T: serde::Serialize>(data: T, status: StatusCode) -> Response {
    (status, Json(json!({ "data": data, "meta": {} }))).into_response()
}

#[derive(Deserialize)]
pub struct ListQ { pub status: Option<String> }

pub async fn list_campaigns(cu: CurrentUser, State(st): State<AppState>, Query(q): Query<ListQ>) -> Result<Response, AppError> {
    cu.require("khatmil.read")?;
    Ok(ok(svc::list_campaigns(&st.pool, q.status).await?, StatusCode::OK))
}

pub async fn create_campaign(cu: CurrentUser, State(st): State<AppState>, Json(req): Json<CampaignUpsertReq>) -> Result<Response, AppError> {
    cu.require("khatmil.manage")?;
    let id = svc::create_campaign(&st.pool, cu.user_id, req).await?;
    let d = svc::campaign_detail(&st.pool, id).await?;
    Ok(ok(d, StatusCode::CREATED))
}

pub async fn update_campaign(cu: CurrentUser, State(st): State<AppState>, Path(id): Path<i64>, Json(req): Json<CampaignUpsertReq>) -> Result<Response, AppError> {
    cu.require("khatmil.manage")?;
    svc::update_campaign(&st.pool, id, req).await?;
    Ok(ok(json!({ "updated": true }), StatusCode::OK))
}

pub async fn campaign_detail(cu: CurrentUser, State(st): State<AppState>, Path(id): Path<i64>) -> Result<Response, AppError> {
    cu.require("khatmil.read")?;
    Ok(ok(svc::campaign_detail(&st.pool, id).await?, StatusCode::OK))
}

pub async fn join(cu: CurrentUser, State(st): State<AppState>, Path(id): Path<i64>) -> Result<Response, AppError> {
    cu.require("khatmil.join")?;
    svc::join(&st.pool, cu.user_id, id).await?;
    Ok(ok(json!({ "joined": true }), StatusCode::CREATED))
}

pub async fn claim(cu: CurrentUser, State(st): State<AppState>, Path(id): Path<i64>, body: Option<Json<ClaimReq>>) -> Result<Response, AppError> {
    cu.require("khatmil.join")?;
    let juz = body.and_then(|Json(b)| b.juz);
    // rate-limit klaim in-process (anti spam; 5/detik per user lebih dari cukup utk UX)
    if !st.khatmil_claim_gate(&cu.user_id) {
        return Err(AppError::RateLimited);
    }
    let a = svc::claim(&st.pool, cu.user_id, id, juz).await?;
    Ok(ok(a, StatusCode::CREATED))
}

pub async fn participants(cu: CurrentUser, State(st): State<AppState>, Path(id): Path<i64>) -> Result<Response, AppError> {
    cu.require("khatmil.manage")?;
    Ok(ok(svc::participants(&st.pool, id).await?, StatusCode::OK))
}

pub async fn post_progress(cu: CurrentUser, State(st): State<AppState>, Path(id): Path<i64>, Json(req): Json<ProgressReq>) -> Result<Response, AppError> {
    cu.require("khatmil.join")?;
    // rate-limit progress in-process (keputusan #13 — BUKAN di DB): 1 post / 30 detik / user
    if !st.khatmil_progress_gate(&cu.user_id) {
        return Err(AppError::RateLimited);
    }
    let a = svc::post_progress(&st.pool, cu.user_id, id, req).await?;
    Ok(ok(a, StatusCode::CREATED))
}

pub async fn my_assignments(cu: CurrentUser, State(st): State<AppState>) -> Result<Response, AppError> {
    cu.require("khatmil.join")?;
    Ok(ok(svc::my_assignments(&st.pool, cu.user_id).await?, StatusCode::OK))
}
