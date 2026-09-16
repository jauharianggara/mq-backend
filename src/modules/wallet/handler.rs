//! Handler wallet (deposit santri & penghasilan ustadz).
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::json;

use crate::middleware::auth::CurrentUser;
use crate::modules::wallet::admin_service;
use crate::modules::wallet::service as wsvc;
use crate::shared::error::AppError;
use crate::state::AppState;

fn ok<T: serde::Serialize>(data: T, status: StatusCode) -> Response {
    (status, Json(json!({ "data": data, "meta": {} }))).into_response()
}

pub async fn get_wallet(cu: CurrentUser, State(st): State<AppState>) -> Result<Response, AppError> {
    cu.require_active()?;
    let balance = wsvc::balance(&st.pool, cu.user_id).await?;
    Ok(ok(json!({ "balance": balance }), StatusCode::OK))
}

#[derive(Deserialize)]
pub struct TopupReq { pub amount: i64 }

pub async fn topup(cu: CurrentUser, State(st): State<AppState>, Json(req): Json<TopupReq>) -> Result<Response, AppError> {
    cu.require_active()?;
    if req.amount < 10_000 || req.amount > 10_000_000 {
        return Err(AppError::Unprocessable("top-up Rp 10.000 - Rp 10.000.000".into()));
    }
    let (payment_id, url) = crate::modules::visits::payments::create_topup_invoice(&st, cu.user_id, req.amount).await?;
    Ok(ok(json!({ "payment_id": payment_id, "invoice_url": url }), StatusCode::CREATED))
}

#[derive(Deserialize)]
pub struct TxQ { pub cursor: Option<i64>, pub limit: Option<i64> }

pub async fn transactions(cu: CurrentUser, State(st): State<AppState>, Query(q): Query<TxQ>) -> Result<Response, AppError> {
    cu.require_active()?;
    let limit = q.limit.unwrap_or(50).clamp(1, 100);
    let rows: Vec<(i64, String, i64, i64, Option<String>, Option<i64>, String)> = sqlx::query_as(
        "SELECT id, tx_type, amount, balance_after, subject_type, subject_id, \
         DATE_FORMAT(created_at, '%Y-%m-%dT%H:%i:%sZ') \
         FROM wallet_transactions WHERE user_id = ? AND (? IS NULL OR id < ?) ORDER BY id DESC LIMIT ?")
        .bind(cu.user_id).bind(q.cursor).bind(q.cursor).bind(limit + 1)
        .fetch_all(&st.pool).await.map_err(|e| {
            tracing::error!("db: {e}");
            AppError::Internal("db".into())
        })?;
    let mut items = Vec::new();
    let mut next = None;
    for (i, r) in rows.into_iter().enumerate() {
        if (i as i64) == limit { next = Some(r.0.to_string()); break; }
        items.push(json!({
            "id": r.0, "tx_type": r.1, "amount": r.2, "balance_after": r.3,
            "subject_type": r.4, "subject_id": r.5, "created_at": r.6,
        }));
    }
    Ok(ok(json!({ "items": items, "meta": { "pagination": { "next_cursor": next, "has_more": next.is_some() } } }), StatusCode::OK))
}


// ===================== admin & santri adjustments =====================

#[derive(Deserialize)]
pub struct AdjustQ { pub q: Option<String>, pub cursor: Option<i64> }

pub async fn admin_list_balances(cu: CurrentUser, State(st): State<AppState>, Query(q): Query<AdjustQ>) -> Result<Response, AppError> {
    cu.require("visits.admin")?;
    let limit = 50;
    let (items, next) = admin_service::list_balances(&st.pool, q.q.as_deref(), limit, q.cursor).await?;
    Ok(ok(json!({ "items": items, "meta": { "pagination": { "next_cursor": next, "has_more": next.is_some() } } }), StatusCode::OK))
}

pub async fn admin_transactions(cu: CurrentUser, State(st): State<AppState>, Path(user_id): Path<i64>, Query(q): Query<TxQ>) -> Result<Response, AppError> {
    admin_perm(&cu)?;
    let limit = q.limit.unwrap_or(50).clamp(1, 100);
    let (items, next) = admin_service::transactions(&st.pool, user_id, limit, q.cursor).await?;
    Ok(paged2(items, next))
}

pub async fn admin_propose_adjustment(cu: CurrentUser, State(st): State<AppState>, Path(user_id): Path<i64>, Json(req): Json<AdminAdjustReq>) -> Result<Response, AppError> {
    admin_perm(&cu)?;
    let id = admin_service::propose_adjustment(&st.pool, cu.user_id, user_id, req.amount, &req.reason).await?;
    Ok(ok(json!({ "id": id, "status": "PENDING_ACC" }), StatusCode::CREATED))
}

fn admin_perm(cu: &CurrentUser) -> Result<(), AppError> {
    cu.require("visits.admin")
}

fn paged2<T: serde::Serialize>(items: Vec<T>, next: Option<String>) -> Response {
    (StatusCode::OK, Json(json!({ "data": items, "meta": { "pagination": { "next_cursor": next, "has_more": next.is_some() } } }))).into_response()
}

#[derive(Deserialize)]
pub struct AdminAdjustReq { pub amount: i64, pub reason: String }

pub async fn my_adjustments(cu: CurrentUser, State(st): State<AppState>) -> Result<Response, AppError> {
    cu.require_active()?;
    Ok(ok(admin_service::my_adjustments(&st.pool, cu.user_id).await?, StatusCode::OK))
}

pub async fn accept_adjustment(cu: CurrentUser, State(st): State<AppState>, Path(id): Path<i64>) -> Result<Response, AppError> {
    cu.require_active()?;
    admin_service::accept_adjustment(&st.pool, cu.user_id, id).await?;
    Ok(ok(json!({ "accepted": true }), StatusCode::OK))
}

pub async fn reject_adjustment(cu: CurrentUser, State(st): State<AppState>, Path(id): Path<i64>) -> Result<Response, AppError> {
    cu.require_active()?;
    admin_service::reject_adjustment(&st.pool, cu.user_id, id).await?;
    Ok(ok(json!({ "rejected": true }), StatusCode::OK))
}
