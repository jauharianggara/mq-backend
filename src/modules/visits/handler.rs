//! Handler modul visits (Bagian V — Pesan Ustadz).
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::json;

use crate::middleware::auth::CurrentUser;
use crate::modules::visits::dto::*;
use crate::modules::visits::{payments as pay, service as svc};
use crate::shared::error::AppError;
use crate::state::AppState;

fn ok<T: serde::Serialize>(data: T, status: StatusCode) -> Response {
    (status, Json(json!({ "data": data, "meta": {} }))).into_response()
}

fn paged<T: serde::Serialize>(items: Vec<T>, next: Option<String>) -> Response {
    ok(json!({ "items": items, "meta": { "next_cursor": next } }), StatusCode::OK)
}

#[derive(Deserialize)]
pub struct NearbyQ {
    pub lat: f64,
    pub lng: f64,
    pub service_type_id: Option<i64>,
}

#[derive(Deserialize)]
pub struct ListQ {
    pub cursor: Option<i64>,
    pub limit: Option<i64>,
}

// ===================== santri =====================

pub async fn list_services(cu: CurrentUser, State(st): State<AppState>) -> Result<Response, AppError> {
    cu.require_active()?;
    Ok(ok(svc::list_services(&st.pool).await?, StatusCode::OK))
}

pub async fn nearby(cu: CurrentUser, State(st): State<AppState>, Query(q): Query<NearbyQ>) -> Result<Response, AppError> {
    cu.require("visits.book")?;
    cu.require_active()?;
    // kill-switch dulu, baru rate-limit (off harus 403, bukan 429)
    if !svc::visit_enabled(&st.pool).await? {
        return Err(AppError::Forbidden("modul Pesan Ustadz sedang nonaktif".into()));
    }
    if !st.visit_nearby_gate(&cu.user_id) {
        return Err(AppError::RateLimited);
    }
    Ok(ok(svc::nearby(&st, cu.user_id, q.lat, q.lng, q.service_type_id).await?, StatusCode::OK))
}

pub async fn create_visit(
    cu: CurrentUser,
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateVisitReq>,
) -> Result<Response, AppError> {
    cu.require("visits.book")?;
    cu.require_active()?;
    let key = headers
        .get("idempotency-key")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AppError::Unprocessable("header Idempotency-Key wajib (UUID per percobaan)".into()))?;
    let out = svc::create_visit(&st, cu.user_id, req, key).await?;
    let replay = out.out.replay;
    Ok(ok(out.out, if replay { StatusCode::OK } else { StatusCode::CREATED }))
}

pub async fn list_my_visits(cu: CurrentUser, State(st): State<AppState>, Query(q): Query<ListQ>) -> Result<Response, AppError> {
    cu.require("visits.book")?;
    let limit = q.limit.unwrap_or(20).clamp(1, 50);
    let (items, next) = svc::list_my(&st.pool, cu.user_id, limit, q.cursor).await?;
    Ok(paged(items, next))
}

pub async fn visit_detail(cu: CurrentUser, State(st): State<AppState>, Path(id): Path<i64>) -> Result<Response, AppError> {
    let v = svc::require_visit_access(&st.pool, &cu, id).await?;
    let unread = svc::unread_count(&st.pool, cu.user_id, id).await?;
    let full = svc::visit_detail_out(&st.pool, &v, Some(cu.user_id)).await?;
    let mut val = serde_json::to_value(&full).unwrap_or_default();
    val["unread"] = json!(unread);
    Ok(ok(val, StatusCode::OK))
}

pub async fn pay(cu: CurrentUser, State(st): State<AppState>, Path(id): Path<i64>) -> Result<Response, AppError> {
    cu.require("visits.book")?;
    let v = svc::require_visit_access(&st.pool, &cu, id).await?;
    if v.user_id != cu.user_id {
        return Err(AppError::NotFound("pesanan tidak ada".into()));
    }
    let url = pay::issue_invoice(&st, id).await?;
    Ok(ok(json!({ "invoice_url": url }), StatusCode::OK))
}

pub async fn cancel(cu: CurrentUser, State(st): State<AppState>, Path(id): Path<i64>) -> Result<Response, AppError> {
    cu.require("visits.book")?;
    let v = svc::require_visit_access(&st.pool, &cu, id).await?;
    if v.user_id != cu.user_id {
        return Err(AppError::Forbidden("hanya pemesan yang bisa membatalkan".into()));
    }
    let out = svc::cancel(&st, &cu, id).await?;
    Ok(ok(out, StatusCode::OK))
}

// ===================== review =====================

pub async fn submit_review(cu: CurrentUser, State(st): State<AppState>, Path(id): Path<i64>, Json(req): Json<ReviewReq>) -> Result<Response, AppError> {
    cu.require_active()?;
    svc::submit_review(&st.pool, cu.user_id, id, req.rating, req.comment).await?;
    Ok(ok(json!({ "submitted": true }), StatusCode::CREATED))
}

pub async fn review_status(cu: CurrentUser, State(st): State<AppState>, Path(id): Path<i64>) -> Result<Response, AppError> {
    cu.require_active()?;
    Ok(ok(svc::review_status(&st.pool, cu.user_id, id).await?, StatusCode::OK))
}

pub async fn ustadz_public_reviews(cu: CurrentUser, State(st): State<AppState>, Path(id): Path<i64>, Query(q): Query<ListQ>) -> Result<Response, AppError> {
    cu.require_active()?;
    let limit = q.limit.unwrap_or(20).clamp(1, 50);
    let (items, next) = svc::ustadz_public_reviews(&st.pool, id, limit, q.cursor).await?;
    Ok(paged(items, next))
}

// ===================== chat =====================

pub async fn messages_list(cu: CurrentUser, State(st): State<AppState>, Path(id): Path<i64>, Query(q): Query<ListQ>) -> Result<Response, AppError> {
    cu.require_active()?;
    let limit = q.limit.unwrap_or(50).clamp(1, 100);
    let (items, next) = svc::messages_list(&st.pool, &cu, id, limit, q.cursor).await?;
    Ok(paged(items, next))
}

pub async fn messages_send(cu: CurrentUser, State(st): State<AppState>, Path(id): Path<i64>, Json(req): Json<SendMessageReq>) -> Result<Response, AppError> {
    cu.require_active()?;
    let out = svc::messages_send(&st, &cu, id, &req.body).await?;
    Ok(ok(out, StatusCode::CREATED))
}

// ===================== lokasi =====================

pub async fn put_my_location(cu: CurrentUser, State(st): State<AppState>, Json(req): Json<PutLocationReq>) -> Result<Response, AppError> {
    cu.require_active()?;
    if !(-90.0..=90.0).contains(&req.lat) || !(-180.0..=180.0).contains(&req.lng) {
        return Err(AppError::Unprocessable("koordinat tidak valid".into()));
    }
    svc::put_my_location(&st, cu.user_id, req.lat, req.lng, req.accuracy_m).await?;
    Ok(ok(json!({ "updated": true }), StatusCode::OK))
}

// ===================== ustadz =====================

fn ustadz_perm(cu: &CurrentUser) -> Result<(), AppError> {
    cu.require("ustadz.visits.manage")
}

pub async fn get_visit_settings(cu: CurrentUser, State(st): State<AppState>) -> Result<Response, AppError> {
    ustadz_perm(&cu)?;
    Ok(ok(svc::get_visit_settings(&st.pool, cu.user_id).await?, StatusCode::OK))
}

pub async fn put_visit_settings(cu: CurrentUser, State(st): State<AppState>, Json(req): Json<VisitSettingsReq>) -> Result<Response, AppError> {
    ustadz_perm(&cu)?;
    let out = svc::put_visit_settings(&st.pool, cu.user_id, req).await?;
    Ok(ok(out, StatusCode::OK))
}

pub async fn list_tarif(cu: CurrentUser, State(st): State<AppState>) -> Result<Response, AppError> {
    ustadz_perm(&cu)?;
    Ok(ok(svc::list_tarif(&st.pool, cu.user_id).await?, StatusCode::OK))
}

pub async fn upsert_tarif(cu: CurrentUser, State(st): State<AppState>, Json(req): Json<TarifUpsertReq>) -> Result<Response, AppError> {
    ustadz_perm(&cu)?;
    svc::upsert_tarif(&st.pool, cu.user_id, req).await?;
    Ok(ok(json!({ "saved": true }), StatusCode::CREATED))
}

pub async fn delete_tarif(cu: CurrentUser, State(st): State<AppState>, Path(id): Path<i64>) -> Result<Response, AppError> {
    ustadz_perm(&cu)?;
    svc::delete_tarif(&st.pool, cu.user_id, id).await?;
    Ok(ok(json!({ "deleted": true }), StatusCode::OK))
}

pub async fn my_visits(cu: CurrentUser, State(st): State<AppState>) -> Result<Response, AppError> {
    ustadz_perm(&cu)?;
    Ok(ok(svc::my_visits(&st.pool, cu.user_id).await?, StatusCode::OK))
}

pub async fn requester_reviews(cu: CurrentUser, State(st): State<AppState>, Path(id): Path<i64>, Query(q): Query<ListQ>) -> Result<Response, AppError> {
    ustadz_perm(&cu)?;
    let limit = q.limit.unwrap_or(20).clamp(1, 50);
    let (items, next) = svc::requester_reviews(&st.pool, cu.user_id, id, limit, q.cursor).await?;
    Ok(paged(items, next))
}

pub async fn confirm_visit(cu: CurrentUser, State(st): State<AppState>, Path(id): Path<i64>) -> Result<Response, AppError> {
    ustadz_perm(&cu)?;
    let out = svc::confirm_visit(&st, cu.user_id, id).await?;
    Ok(ok(out, StatusCode::OK))
}

#[derive(Deserialize)]
pub struct DeclineReq {
    pub reason: Option<String>,
}

pub async fn decline_visit(cu: CurrentUser, State(st): State<AppState>, Path(id): Path<i64>, body: Option<Json<DeclineReq>>) -> Result<Response, AppError> {
    ustadz_perm(&cu)?;
    let reason = body.map(|Json(b)| b.reason.unwrap_or_default()).unwrap_or_default();
    let out = svc::decline_visit(&st, cu.user_id, id, &reason).await?;
    Ok(ok(out, StatusCode::OK))
}

pub async fn complete_visit(cu: CurrentUser, State(st): State<AppState>, Path(id): Path<i64>) -> Result<Response, AppError> {
    ustadz_perm(&cu)?;
    let out = svc::complete_visit(&st, cu.user_id, id).await?;
    Ok(ok(out, StatusCode::OK))
}

// ===================== webhook & dev-simulate (TANPA auth user) =====================

pub async fn xendit_callback(State(st): State<AppState>, headers: HeaderMap, body: String) -> Result<Response, AppError> {
    // verifikasi x-callback-token (constant-time)
    let expected = st.payments.callback_token.clone().unwrap_or_default();
    let got = headers
        .get("x-callback-token")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if expected.is_empty() {
        return Err(AppError::Forbidden("webhook belum dikonfigurasi (XENDIT_CALLBACK_TOKEN)".into()));
    }
    if !crate::infrastructure::xendit::token_eq(got, &expected) {
        return Err(AppError::Unauthorized("callback token tidak valid".into()));
    }
    let v: serde_json::Value = serde_json::from_str(&body)
        .map_err(|_| AppError::Unprocessable("body JSON tidak valid".into()))?;
    let external_id = v["external_id"].as_str().unwrap_or_default().to_string();
    let invoice_id = v["id"].as_str().map(str::to_string);
    let status = v["status"].as_str().unwrap_or_default().to_string();
    let channel = v["payment_method"].as_str().map(str::to_string);
    if external_id.is_empty() {
        return Err(AppError::Unprocessable("external_id kosong".into()));
    }
    let res = match status.as_str() {
        "PAID" | "SETTLED" => pay::apply_paid(&st, &external_id, invoice_id.as_deref(), channel.as_deref(), &body).await?,
        "EXPIRED" | "EXPIRING" => pay::apply_expired(&st, &external_id, invoice_id.as_deref(), &body).await?,
        other => format!("ignored status {other}"),
    };
    Ok(ok(json!({ "received": true, "result": res }), StatusCode::OK))
}

#[derive(Deserialize)]
pub struct SimulateReq {
    pub external_id: String,
    pub event: Option<String>,
}

/// DEV ONLY (gateway mock + MQ_DEV_EXPOSE_TOKENS): simulasikan webhook Xendit.
pub async fn dev_simulate(State(st): State<AppState>, Json(req): Json<SimulateReq>) -> Result<Response, AppError> {
    if !st.payments.is_mock() || !st.dev_expose_tokens() {
        return Err(AppError::Forbidden("hanya utk mode dev/mock".into()));
    }
    let event = req.event.unwrap_or_else(|| "paid".into());
    let res = match event.as_str() {
        "paid" => pay::apply_paid(&st, &req.external_id, None, Some("MOCK"), "{}").await?,
        "expired" => pay::apply_expired(&st, &req.external_id, None, "{}").await?,
        other => return Err(AppError::Unprocessable(format!("event {other} tidak dikenal"))),
    };
    Ok(ok(json!({ "simulated": event, "result": res }), StatusCode::OK))
}

// ===================== admin =====================

fn admin_perm(cu: &CurrentUser) -> Result<(), AppError> {
    cu.require("visits.admin")
}

pub async fn admin_list_visits(cu: CurrentUser, State(st): State<AppState>, Query(f): Query<AdminVisitFilter>) -> Result<Response, AppError> {
    admin_perm(&cu)?;
    let limit = 20;
    let (items, next) = svc::admin_list_visits(&st.pool, &f, limit).await?;
    Ok(paged(items, next))
}

pub async fn admin_visit_detail(cu: CurrentUser, State(st): State<AppState>, Path(id): Path<i64>) -> Result<Response, AppError> {
    admin_perm(&cu)?;
    let v = svc::require_visit_access(&st.pool, &cu, id).await?;
    let full = svc::visit_detail_out(&st.pool, &v, None).await?;
    Ok(ok(full, StatusCode::OK))
}

pub async fn admin_force_complete(cu: CurrentUser, State(st): State<AppState>, Path(id): Path<i64>) -> Result<Response, AppError> {
    admin_perm(&cu)?;
    let out = svc::admin_force_complete(&st.pool, cu.user_id, id).await?;
    Ok(ok(out, StatusCode::OK))
}

pub async fn admin_force_cancel(cu: CurrentUser, State(st): State<AppState>, Path(id): Path<i64>, Json(req): Json<ForceCancelReq>) -> Result<Response, AppError> {
    admin_perm(&cu)?;
    let out = svc::admin_force_cancel(&st, cu.user_id, id, req).await?;
    Ok(ok(out, StatusCode::OK))
}

pub async fn admin_messages(cu: CurrentUser, State(st): State<AppState>, Path(id): Path<i64>, Query(q): Query<ListQ>) -> Result<Response, AppError> {
    admin_perm(&cu)?;
    let limit = q.limit.unwrap_or(50).clamp(1, 100);
    let (items, next) = svc::messages_list(&st.pool, &cu, id, limit, q.cursor).await?;
    Ok(paged(items, next))
}

#[derive(Deserialize)]
pub struct PaymentQ {
    pub status: Option<String>,
    pub cursor: Option<i64>,
}

pub async fn admin_list_payments(cu: CurrentUser, State(st): State<AppState>, Query(q): Query<PaymentQ>) -> Result<Response, AppError> {
    admin_perm(&cu)?;
    let (items, next) = pay::admin_list_payments(&st.pool, q.status.as_deref(), 20, q.cursor).await?;
    Ok(paged(items, next))
}

pub async fn admin_mark_refunded(cu: CurrentUser, State(st): State<AppState>, Path(id): Path<i64>) -> Result<Response, AppError> {
    admin_perm(&cu)?;
    pay::admin_mark_refunded(&st.pool, cu.user_id, id).await?;
    Ok(ok(json!({ "marked": true }), StatusCode::OK))
}

#[derive(Deserialize)]
pub struct ReviewQ {
    pub direction: Option<String>,
    pub cursor: Option<i64>,
}

pub async fn admin_list_reviews(cu: CurrentUser, State(st): State<AppState>, Query(q): Query<ReviewQ>) -> Result<Response, AppError> {
    admin_perm(&cu)?;
    let (items, next) = svc::admin_list_reviews(&st.pool, q.direction.as_deref(), 20, q.cursor).await?;
    Ok(paged(items, next))
}

pub async fn admin_hide_review(cu: CurrentUser, State(st): State<AppState>, Path(id): Path<i64>) -> Result<Response, AppError> {
    admin_perm(&cu)?;
    svc::admin_set_review_hidden(&st.pool, id, true).await?;
    Ok(ok(json!({ "hidden": true }), StatusCode::OK))
}

pub async fn admin_unhide_review(cu: CurrentUser, State(st): State<AppState>, Path(id): Path<i64>) -> Result<Response, AppError> {
    admin_perm(&cu)?;
    svc::admin_set_review_hidden(&st.pool, id, false).await?;
    Ok(ok(json!({ "hidden": false }), StatusCode::OK))
}
