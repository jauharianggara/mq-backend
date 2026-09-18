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
pub struct AdjustQ {
    pub q: Option<String>,
    pub cursor: Option<i64>,
    /// SANTRI (default) | USTADZ
    pub role: Option<String>,
}

pub async fn admin_list_balances(cu: CurrentUser, State(st): State<AppState>, Query(q): Query<AdjustQ>) -> Result<Response, AppError> {
    cu.require("visits.admin")?;
    let limit = 50;
    let role = match q.role.as_deref() {
        Some("USTADZ") => "USTADZ",
        _ => "SANTRI",
    };
    let (items, next) = admin_service::list_balances(&st.pool, role, q.q.as_deref(), limit, q.cursor).await?;
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

#[derive(Deserialize)]
pub struct AdjustListQ { pub status: Option<String>, pub cursor: Option<i64> }

/// Monitoring semua penyesuaian saldo (nama santri + admin pengaju).
pub async fn admin_list_adjustments(cu: CurrentUser, State(st): State<AppState>, Query(q): Query<AdjustListQ>) -> Result<Response, AppError> {
    admin_perm(&cu)?;
    let (items, next) = admin_service::list_adjustments(&st.pool, q.status.as_deref(), 50, q.cursor).await?;
    Ok(paged2(items, next))
}

#[derive(Deserialize)]
pub struct PaymentsQ {
    pub status: Option<String>,
    /// ustadz_visit | wallet_topup
    pub subject_type: Option<String>,
    pub cursor: Option<i64>,
    pub sort: Option<String>,
    pub order: Option<String>,
    pub page: Option<i64>,
}

const PAYMENTS_SORT: &[(&str, &str)] = &[
    ("id", "p.id"),
    ("amount", "p.amount"),
    ("created_at", "p.created_at"),
    ("paid_at", "COALESCE(p.paid_at,'1000-01-01 00:00:00')"),
    ("status", "p.status"),
];

/// SEMUA invoice Xendit (kunjungan & top-up deposit) — visibilitas admin.
/// Refund v2 selalu otomatis ke deposit — TIDAK ADA mark-refunded.
pub async fn admin_list_payments(cu: CurrentUser, State(st): State<AppState>, Query(q): Query<PaymentsQ>) -> Result<Response, AppError> {
    admin_perm(&cu)?;
    let limit: i64 = 50;
    let page: Option<i64> = q.page.filter(|p| *p >= 1);
    let srt = crate::shared::sorting::parse(q.sort.as_deref(), q.order.as_deref(), PAYMENTS_SORT, "p.id");
    let cursor: Option<i64> = if srt.custom { None } else { q.cursor };
    let mut sql = format!(
        "SELECT p.id, p.external_id, p.provider, p.channel, p.amount, p.status, p.subject_type, \
         DATE_FORMAT(p.created_at, '%Y-%m-%dT%H:%i:%sZ'), DATE_FORMAT(p.paid_at, '%Y-%m-%dT%H:%i:%sZ'), p.subject_id \
         FROM payments p \
         WHERE (? IS NULL OR p.status = ?) AND (? IS NULL OR p.subject_type = ?) AND (? IS NULL OR p.id < ?) \
         {} LIMIT ?",
        srt.order_by("p.id")
    );
    if srt.custom {
        sql.push_str(" OFFSET ?");
    }
    let mut qy = sqlx::query_as::<_, (i64, String, String, Option<String>, i64, String, Option<String>, String, Option<String>, Option<i64>)>(&sql)
        .bind(&q.status).bind(&q.status)
        .bind(&q.subject_type).bind(&q.subject_type)
        .bind(cursor).bind(cursor)
        .bind(limit + 1);
    if srt.custom {
        qy = qy.bind((page.unwrap_or(1) - 1) * limit);
    }
    let rows = qy.fetch_all(&st.pool).await.map_err(|e| {
        tracing::error!("admin payments db: {e}");
        AppError::Internal("db".into())
    })?;
    let has_more = rows.len() as i64 > limit;
    let next = if !srt.custom { rows.get(limit as usize).map(|r| r.0.to_string()) } else { None };
    let mut items = Vec::new();
    for r in rows.into_iter().take(limit as usize) {
        // label subject: nama pihak terkait (subject_type/subject_id nullable)
        let stype = r.6.clone().unwrap_or_default();
        let sid = r.9.unwrap_or(0);
        let label = match stype.as_str() {
            "ustadz_visit" => {
                let names: Option<(String, String)> = sqlx::query_as(
                    "SELECT COALESCE(NULLIF(su.full_name,''),'(tanpa nama)'), COALESCE(NULLIF(uu.full_name,''),'(tanpa nama)') \
                     FROM ustadz_visits v \
                     LEFT JOIN user_profiles su ON su.user_id = v.user_id \
                     LEFT JOIN user_profiles uu ON uu.user_id = v.ustadz_id \
                     WHERE v.id = ?")
                    .bind(sid).fetch_optional(&st.pool).await.unwrap_or(None);
                match names {
                    Some((s, u)) => format!("Kunjungan #{sid} — {s} → {u}"),
                    None => format!("Kunjungan #{sid}"),
                }
            }
            "wallet_topup" => {
                let n: Option<(String,)> = sqlx::query_as(
                    "SELECT COALESCE(NULLIF(up.full_name,''),'(tanpa nama)') FROM users u \
                     LEFT JOIN user_profiles up ON up.user_id = u.id WHERE u.id = ?")
                    .bind(sid).fetch_optional(&st.pool).await.unwrap_or(None);
                match n {
                    Some((s,)) => format!("Top-up deposit — {s}"),
                    None => format!("Top-up deposit #{sid}"),
                }
            }
            other => format!("{other} #{sid}"),
        };
        items.push(json!({
            "id": r.0, "external_id": r.1, "provider": r.2, "channel": r.3,
            "amount": r.4, "status": r.5, "subject_type": stype, "subject_id": sid,
            "subject_label": label, "created_at": r.7, "paid_at": r.8,
        }));
    }
    Ok(paged2_h(items, next, has_more))
}

fn admin_perm(cu: &CurrentUser) -> Result<(), AppError> {
    cu.require("visits.admin")
}

fn paged2<T: serde::Serialize>(items: Vec<T>, next: Option<String>) -> Response {
    (StatusCode::OK, Json(json!({ "data": items, "meta": { "pagination": { "next_cursor": next, "has_more": next.is_some() } } }))).into_response()
}

/// Sama seperti `paged2` tapi `has_more` eksplisit (mode sort/offset: next kosong namun masih ada halaman).
fn paged2_h<T: serde::Serialize>(items: Vec<T>, next: Option<String>, has_more: bool) -> Response {
    (StatusCode::OK, Json(json!({ "data": items, "meta": { "pagination": { "next_cursor": next, "has_more": has_more } } }))).into_response()
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
