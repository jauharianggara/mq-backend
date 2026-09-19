//! Handler modul khatmil.
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::json;

use crate::middleware::auth::CurrentUser;
use crate::modules::khatmil::dto::*;
use crate::modules::khatmil::penugasan;
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
    let (juz, group_id) = body.and_then(|Json(b)| Some((b.juz, b.group_id))).unwrap_or((None, None));
    // rate-limit klaim in-process (anti spam; 5/detik per user lebih dari cukup utk UX)
    if !st.khatmil_claim_gate(&cu.user_id) {
        return Err(AppError::RateLimited);
    }
    let a = svc::claim(&st.pool, cu.user_id, id, juz, group_id).await?;
    Ok(ok(a, StatusCode::CREATED))
}

pub async fn participants(cu: CurrentUser, State(st): State<AppState>, Path(id): Path<i64>) -> Result<Response, AppError> {
    // khatmil.read (bukan manage) — SANTRI juga boleh baca (leaderboard mobile, rev 3.2)
    cu.require("khatmil.read")?;
    Ok(ok(svc::participants(&st.pool, id).await?, StatusCode::OK))
}

#[derive(Deserialize)]
pub struct ActivityQ { pub limit: Option<i64> }

pub async fn activity(cu: CurrentUser, State(st): State<AppState>, Path(id): Path<i64>, Query(q): Query<ActivityQ>) -> Result<Response, AppError> {
    cu.require("khatmil.read")?;
    let limit = q.limit.unwrap_or(50).clamp(1, 100);
    Ok(ok(svc::activity(&st.pool, id, limit).await?, StatusCode::OK))
}

pub async fn post_progress(cu: CurrentUser, State(st): State<AppState>, Path(id): Path<i64>, Json(req): Json<ProgressReq>) -> Result<Response, AppError> {
    cu.require("khatmil.join")?;
    // rate-limit in-process 5 dtk (rev 3.3: posisi idempotent — spam tak berbahaya; auto-save reader mobile perlu interval pendek)
    if !st.khatmil_progress_gate(&cu.user_id) {
        return Err(AppError::RateLimited);
    }
    let (a, created) = svc::post_progress(&st.pool, cu.user_id, id, req).await?;
    Ok(ok(a, if created { StatusCode::CREATED } else { StatusCode::OK }))
}

pub async fn my_assignments(cu: CurrentUser, State(st): State<AppState>) -> Result<Response, AppError> {
    cu.require("khatmil.join")?;
    Ok(ok(svc::my_assignments(&st.pool, cu.user_id).await?, StatusCode::OK))
}

// ===================== penugasan pembina (rev khatmil v2) =====================

pub async fn ustadz_khatmil_overview(cu: CurrentUser, State(st): State<AppState>) -> Result<Response, AppError> {
    cu.require("khatmil.read")?;
    Ok(ok(penugasan::overview(&st.pool, cu.user_id).await?, StatusCode::OK))
}

pub async fn ustadz_group_progress(cu: CurrentUser, State(st): State<AppState>, Path(id): Path<i64>) -> Result<Response, AppError> {
    cu.require("khatmil.read")?;
    Ok(ok(penugasan::group_progress(&st.pool, cu.user_id, id).await?, StatusCode::OK))
}

pub async fn ustadz_accept(cu: CurrentUser, State(st): State<AppState>, Path(id): Path<i64>) -> Result<Response, AppError> {
    cu.require("khatmil.read")?;
    penugasan::accept_group(&st.pool, cu.user_id, id).await?;
    Ok(ok(json!({ "accepted": true }), StatusCode::OK))
}

pub async fn ustadz_reject(cu: CurrentUser, State(st): State<AppState>, Path(id): Path<i64>) -> Result<Response, AppError> {
    cu.require("khatmil.read")?;
    penugasan::reject_group(&st.pool, cu.user_id, id).await?;
    Ok(ok(json!({ "rejected": true }), StatusCode::OK))
}

#[derive(serde::Deserialize)]
pub struct AssignPembinaPath {
    pub campaign_id: i64,
    pub group_no: i64,
}

#[derive(serde::Deserialize)]
pub struct AssignPembinaReq { pub ustadz_id: i64 }

pub async fn admin_assign_pembina(cu: CurrentUser, State(st): State<AppState>, Path(p): Path<AssignPembinaPath>, Json(req): Json<AssignPembinaReq>) -> Result<Response, AppError> {
    cu.require("khatmil.manage")?;
    // pastikan baris kelompok tersedia (campaign lama bisa belum punya), lalu resolve id
    penugasan::ensure_groups(&st.pool, p.campaign_id).await?;
    let gid: (i64,) = sqlx::query_as(
        "SELECT g.id FROM khatmil_groups g JOIN khatmil_campaigns c ON c.id = g.campaign_id \
         WHERE g.campaign_id = ? AND g.group_no = ? AND g.group_no <= c.group_count")
        .bind(p.campaign_id).bind(p.group_no).fetch_one(&st.pool).await.map_err(|e| {
            tracing::error!("db: {e}");
            AppError::NotFound("kelompok tidak ada".into())
        })?;
    penugasan::admin_assign(&st.pool, cu.user_id, gid.0, req.ustadz_id).await?;
    Ok(ok(json!({ "assigned": true, "campaign_id": p.campaign_id, "group_no": p.group_no }), StatusCode::OK))
}
