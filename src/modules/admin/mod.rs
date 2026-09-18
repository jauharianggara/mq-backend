//! Modul admin (Bagian I Phase 3 #2-admin/#12/#13) — dashboard, settings, audit, users.
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, patch, post, put};
use axum::{Json, Router};
use serde_json::json;

use crate::middleware::auth::CurrentUser;
use crate::shared::error::AppError;
use crate::state::AppState;

fn dberr(e: sqlx::Error) -> AppError {
    tracing::error!("db: {e}");
    AppError::Internal(format!("db: {e}"))
}
fn ok<T: serde::Serialize>(data: T, status: StatusCode) -> Response {
    (status, Json(json!({ "data": data, "meta": {} }))).into_response()
}

/// Envelope list seragam seluruh admin: `{data: [items], meta: {pagination: {next_cursor, has_more}}}`
/// (shape-A — sama dgn helper `paged` modul visits; apiGetPage FE membaca data sbg array).
fn ok_paged<T: serde::Serialize>(items: &[T], next: Option<String>, has_more: bool, status: StatusCode) -> Response {
    (status, Json(json!({
        "data": items,
        "meta": { "pagination": { "next_cursor": next, "has_more": has_more } }
    }))).into_response()
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/admin/dashboard", get(dashboard))
        .route("/admin/settings", get(settings_list))
        .route("/admin/settings/{key}", put(settings_put))
        .route("/admin/audit-logs", get(audit_list))
        .route("/admin/users", get(users_list))
        .route("/admin/santri", get(santri_list))
        .route("/admin/ustadz", get(ustadz_list))
        .route("/admin/users-count", get(users_count))
        .route("/admin/count", get(admin_count))
        .route("/admin/reviews", get(admin_reviews))
        .route("/admin/reviews/{id}/hide", post(admin_review_hide))
        .route("/admin/reviews/{id}/unhide", post(admin_review_unhide))
        .route("/admin/santri/{id}", get(santri_detail))
        .route("/admin/ustadz/{id}", get(ustadz_detail))
        .route("/admin/users/{id}", patch(users_patch))
}

async fn dashboard(cu: CurrentUser, State(st): State<AppState>) -> Result<Response, AppError> {
    cu.require("dashboard.view")?;
    let (users_total, santri_active, ustadz_active): (i64, i64, i64) = sqlx::query_as(
        "SELECT \
         (SELECT COUNT(*) FROM users WHERE status = 'ACTIVE'), \
         (SELECT COUNT(*) FROM users u JOIN user_roles ur ON ur.user_id = u.id JOIN roles r ON r.id = ur.role_id WHERE r.code = 'SANTRI' AND u.status = 'ACTIVE'), \
         (SELECT COUNT(*) FROM ustadz_profiles up JOIN users u ON u.id = up.user_id WHERE up.verified_at IS NOT NULL AND u.status = 'ACTIVE')")
        .fetch_one(&st.pool).await.map_err(dberr)?;
    let (setoran_total, setoran_pending, setoran_passed, avg_review): (i64, i64, i64, Option<f64>) = sqlx::query_as(
        "SELECT \
         (SELECT COUNT(*) FROM memorization_submissions), \
         (SELECT COUNT(*) FROM memorization_submissions WHERE status IN ('PENDING','IN_REVIEW')), \
         (SELECT COUNT(*) FROM memorization_submissions WHERE status = 'PASSED'), \
         (SELECT CAST(AVG(TIMESTAMPDIFF(MINUTE, submitted_at, reviewed_at)) AS DOUBLE) FROM memorization_submissions WHERE reviewed_at IS NOT NULL)")
        .fetch_one(&st.pool).await.map_err(dberr)?;
    let (khatmil_active, khatmil_juz_done): (i64, i64) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM khatmil_campaigns WHERE status = 'ACTIVE'), \
         (SELECT COUNT(*) FROM khatmil_juz_assignments WHERE status = 'COMPLETED')")
        .fetch_one(&st.pool).await.map_err(dberr)?;
    let (q_total, q_answered, q_published, avg_answer_min): (i64, i64, i64, Option<f64>) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM questions), \
         (SELECT COUNT(*) FROM questions WHERE status IN ('ANSWERED','PUBLISH_REQUESTED','PUBLISHED')), \
         (SELECT COUNT(*) FROM questions WHERE status = 'PUBLISHED'), \
         (SELECT CAST(AVG(TIMESTAMPDIFF(MINUTE, q.created_at, (SELECT MIN(h.created_at) FROM question_status_history h WHERE h.question_id = q.id AND h.to_status = 'ANSWERED'))) AS DOUBLE) FROM questions q WHERE q.status IN ('ANSWERED','PUBLISH_REQUESTED','PUBLISHED'))")
        .fetch_one(&st.pool).await.map_err(dberr)?;
    // time-series aktivitas 7 hari (learning journey)
    let series: Vec<(String, i64)> = sqlx::query_as(
        "SELECT DATE_FORMAT(d.day, '%Y-%m-%d'), \
         (SELECT COUNT(*) FROM activity_events ae WHERE DATE(ae.occurred_at) = DATE(d.day)) \
         FROM (SELECT DATE_ADD(UTC_TIMESTAMP(), INTERVAL -n DAY) AS day \
               FROM (SELECT 0 n UNION SELECT 1 UNION SELECT 2 UNION SELECT 3 UNION SELECT 4 UNION SELECT 5 UNION SELECT 6) x) d \
         ORDER BY d.day")
        .fetch_all(&st.pool).await.map_err(dberr)?;
    Ok(ok(json!({
        "users": { "active": users_total, "santri_active": santri_active, "ustadz_verified": ustadz_active },
        "memorization": { "total": setoran_total, "pending": setoran_pending, "passed": setoran_passed, "avg_review_minutes": avg_review },
        "khatmil": { "campaigns_active": khatmil_active, "juz_completed": khatmil_juz_done },
        "questions": { "total": q_total, "answered": q_answered, "published": q_published, "avg_answer_minutes": avg_answer_min },
        "activity_series_7d": series.into_iter().map(|(d, n)| json!({ "date": d, "events": n })).collect::<Vec<_>>(),
    }), StatusCode::OK))
}

async fn settings_list(cu: CurrentUser, State(st): State<AppState>) -> Result<Response, AppError> {
    cu.require("settings.manage")?;
    let rows: Vec<(String, String, Option<String>)> = sqlx::query_as(
        "SELECT `key`, CAST(value AS CHAR), description FROM settings ORDER BY `key`")
        .fetch_all(&st.pool).await.map_err(dberr)?;
    let mut m = serde_json::Map::new();
    for (k, v, d) in rows {
        m.insert(k, json!({ "value": serde_json::from_str::<serde_json::Value>(&v).unwrap_or(serde_json::Value::String(v)), "description": d }));
    }
    Ok(ok(m, StatusCode::OK))
}

async fn settings_put(cu: CurrentUser, State(st): State<AppState>, Path(key): Path<String>, Json(body): Json<serde_json::Value>) -> Result<Response, AppError> {
    cu.require("settings.manage")?;
    let raw = body.to_string();
    let n = sqlx::query("INSERT INTO settings (`key`, value, updated_by) VALUES (?, CAST(? AS JSON), ?) AS new \
                         ON DUPLICATE KEY UPDATE value = CAST(? AS JSON), updated_by = new.updated_by, updated_at = CURRENT_TIMESTAMP")
        .bind(&key).bind(&raw).bind(cu.user_id).bind(&raw)
        .execute(&st.pool).await.map_err(dberr)?;
    let _ = n;
    let _ = sqlx::query("INSERT INTO audit_logs (actor_id, action, module, entity_type, entity_id, new_value) \
                         VALUES (?, 'UPDATE', 'settings', 'settings', ?, CAST(? AS JSON))")
        .bind(cu.user_id).bind(&key).bind(&raw).execute(&st.pool).await;
    Ok(ok(json!({ "updated": true, "key": key }), StatusCode::OK))
}

async fn audit_list(cu: CurrentUser, State(st): State<AppState>, Query(q): Query<std::collections::HashMap<String, String>>) -> Result<Response, AppError> {
    cu.require("audit.read")?;
    let module = q.get("module").cloned();
    let actor: Option<i64> = q.get("actor_id").and_then(|v| v.parse().ok());
    let cursor: Option<i64> = q.get("cursor").and_then(|v| v.parse().ok());
    let limit: i64 = q.get("limit").and_then(|v| v.parse().ok()).unwrap_or(20).clamp(1, 100);
    let rows: Vec<(i64, Option<i64>, String, String, String, Option<String>, Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT id, actor_id, action, module, entity_type, entity_id, CAST(old_value AS CHAR), CAST(new_value AS CHAR), DATE_FORMAT(created_at, '%Y-%m-%dT%H:%i:%sZ') \
         FROM audit_logs WHERE (? IS NULL OR id < ?) AND (? IS NULL OR module = ?) AND (? IS NULL OR actor_id = ?) \
         ORDER BY id DESC LIMIT ?")
        .bind(cursor).bind(cursor).bind(module.as_deref()).bind(module.as_deref()).bind(actor).bind(actor).bind(limit + 1)
        .fetch_all(&st.pool).await.map_err(dberr)?;
    let has_more = rows.len() as i64 > limit;
    let next = if has_more { rows.get(limit as usize).map(|r| r.0.to_string()) } else { None };
    let items: Vec<_> = rows.iter().take(limit as usize).map(|r| json!({
        "id": r.0, "actor_id": r.1, "action": r.2, "module": r.3, "entity_type": r.4,
        "entity_id": r.5, "old": r.6.as_deref().and_then(|v| serde_json::from_str::<serde_json::Value>(v).ok()),
        "new": r.7.as_deref().and_then(|v| serde_json::from_str::<serde_json::Value>(v).ok()), "created_at": r.8,
    })).collect();
    Ok(ok_paged(&items, next, has_more, StatusCode::OK))
}

/// Kolom sort whitelist `/admin/users` (param FE -> ekspresi SQL).
const USERS_SORT: &[(&str, &str)] = &[
    ("id", "u.id"),
    ("full_name", "COALESCE(NULLIF(upn3.full_name,''),'')"),
    ("status", "u.status"),
    ("last_login_at", "u.last_login_at"),
];

async fn users_list(cu: CurrentUser, State(st): State<AppState>, Query(q): Query<std::collections::HashMap<String, String>>) -> Result<Response, AppError> {
    cu.require("users.read")?;
    let status = q.get("status").cloned();
    let qq = q.get("q").map(|s| format!("%{s}%"));
    let role = q.get("role").filter(|r| matches!(r.as_str(), "SANTRI" | "USTADZ" | "ADMIN" | "MODERATOR")).cloned();
    let limit: i64 = q.get("limit").and_then(|v| v.parse().ok()).unwrap_or(20).clamp(1, 100);
    let page: Option<i64> = q.get("page").and_then(|v| v.parse().ok()).filter(|p| *p >= 1);
    let srt = crate::shared::sorting::parse(q.get("sort").map(String::as_str), q.get("order").map(String::as_str), USERS_SORT, "u.id");
    let cursor: Option<i64> = if srt.custom { None } else { q.get("cursor").and_then(|v| v.parse().ok()) };
    let mut sql = format!(
        "SELECT u.id, u.email, u.phone, u.account_type, u.status,          (SELECT GROUP_CONCAT(r.code) FROM user_roles ur JOIN roles r ON r.id = ur.role_id WHERE ur.user_id = u.id),          DATE_FORMAT(u.last_login_at, '%Y-%m-%dT%H:%i:%sZ'),          (SELECT upn.full_name FROM user_profiles upn WHERE upn.user_id = u.id),          (SELECT upn.city FROM user_profiles upn WHERE upn.user_id = u.id),          (SELECT upu.pendidikan_terakhir FROM ustadz_profiles upu WHERE upu.user_id = u.id),          (SELECT upu.point_label FROM ustadz_profiles upu WHERE upu.user_id = u.id),          (SELECT CAST(upu.point_lat AS DOUBLE) FROM ustadz_profiles upu WHERE upu.user_id = u.id), (SELECT CAST(upu.point_lng AS DOUBLE) FROM ustadz_profiles upu WHERE upu.user_id = u.id)          FROM users u LEFT JOIN user_profiles upn3 ON upn3.user_id = u.id WHERE (? IS NULL OR u.id < ?) AND (? IS NULL OR u.status = ?)          AND (? IS NULL OR EXISTS (SELECT 1 FROM user_roles ur2 JOIN roles r2 ON r2.id = ur2.role_id WHERE ur2.user_id = u.id AND r2.code = ?))          AND (? IS NULL OR u.email LIKE ? OR u.phone LIKE ?               OR EXISTS (SELECT 1 FROM user_profiles upn2 WHERE upn2.user_id = u.id AND upn2.full_name LIKE ?))          {} LIMIT ?",
        srt.order_by("u.id")
    );
    if srt.custom {
        sql.push_str(" OFFSET ?");
    }
    let mut qy = sqlx::query_as::<_, (
        i64, Option<String>, Option<String>, String, String, Option<String>, Option<String>,
        Option<String>, Option<String>, Option<String>, Option<String>, Option<f64>, Option<f64>,
    )>(&sql)
        .bind(cursor).bind(cursor).bind(status.as_deref()).bind(status.as_deref())
        .bind(role.as_deref()).bind(role.as_deref())
        .bind(qq.as_deref()).bind(qq.as_deref()).bind(qq.as_deref()).bind(qq.as_deref()).bind(limit + 1);
    if srt.custom {
        qy = qy.bind((page.unwrap_or(1) - 1) * limit);
    }
    let rows = qy.fetch_all(&st.pool).await.map_err(dberr)?;
    let has_more = rows.len() as i64 > limit;
    let next = if !srt.custom { rows.get(limit as usize).map(|r| r.0.to_string()) } else { None };
    let items: Vec<_> = rows.iter().take(limit as usize).map(|r| json!({
        "id": r.0, "email": r.1, "phone": r.2, "account_type": r.3, "status": r.4,
        "roles": r.5.as_deref().map(|s| s.split(',').collect::<Vec<_>>()), "last_login_at": r.6,
        "full_name": r.7, "city": r.8,
        "pendidikan_terakhir": r.9,
        "point": r.11.map(|lat| json!({
            "lat": lat,
            "lng": r.12,
            "label": r.10,
        })),
    })).collect();
    Ok(ok_paged(&items, next, has_more, StatusCode::OK))
}

// ===================== daftar khusus: SANTRI & USTADZ (field beda per jenis) =====================

/// Kolom sort whitelist `/admin/santri`.
const SANTRI_SORT: &[(&str, &str)] = &[
    ("id", "u.id"),
    ("full_name", "COALESCE(NULLIF(up.full_name,''),'')"),
    ("status", "u.status"),
    ("last_login_at", "u.last_login_at"),
    ("deposit", "COALESCE(w.balance,0)"),
];

#[allow(clippy::type_complexity)]
async fn santri_list(cu: CurrentUser, State(st): State<AppState>, Query(q): Query<std::collections::HashMap<String, String>>) -> Result<Response, AppError> {
    cu.require("users.read")?;
    let status = q.get("status").cloned();
    let qq = q.get("q").map(|s| format!("%{s}%"));
    let limit: i64 = q.get("limit").and_then(|v| v.parse().ok()).unwrap_or(20).clamp(1, 100);
    let page: Option<i64> = q.get("page").and_then(|v| v.parse().ok()).filter(|p| *p >= 1);
    let srt = crate::shared::sorting::parse(q.get("sort").map(String::as_str), q.get("order").map(String::as_str), SANTRI_SORT, "u.id");
    let cursor: Option<i64> = if srt.custom { None } else { q.get("cursor").and_then(|v| v.parse().ok()) };
    let mut sql = format!(
        "SELECT u.id, COALESCE(NULLIF(up.full_name,''),'(tanpa nama)'), u.email, u.phone, up.city, u.status, \
         DATE_FORMAT(u.last_login_at, '%Y-%m-%dT%H:%i:%sZ'), COALESCE(w.balance, 0), \
         (SELECT COUNT(DISTINCT kp.campaign_id) FROM khatmil_participants kp \
            JOIN khatmil_campaigns kc ON kc.id = kp.campaign_id AND kc.status = 'ACTIVE' WHERE kp.user_id = u.id), \
         (SELECT COUNT(*) FROM ustadz_visits v WHERE v.user_id = u.id AND v.status IN ('COMPLETED','REVIEWED')) \
         FROM users u \
         JOIN user_roles ur ON ur.user_id = u.id \
         JOIN roles r ON r.id = ur.role_id AND r.code = 'SANTRI' \
         LEFT JOIN user_profiles up ON up.user_id = u.id \
         LEFT JOIN wallets w ON w.user_id = u.id \
         WHERE (? IS NULL OR u.id < ?) AND (? IS NULL OR u.status = ?) \
           AND (? IS NULL OR u.email LIKE ? OR u.phone LIKE ? \
                OR EXISTS (SELECT 1 FROM user_profiles upn2 WHERE upn2.user_id = u.id AND upn2.full_name LIKE ?)) \
         {} LIMIT ?",
        srt.order_by("u.id")
    );
    if srt.custom {
        sql.push_str(" OFFSET ?");
    }
    let mut qy = sqlx::query_as::<_, (i64, String, Option<String>, Option<String>, Option<String>, String, Option<String>, i64, i64, i64)>(&sql)
        .bind(cursor).bind(cursor).bind(status.as_deref()).bind(status.as_deref())
        .bind(qq.as_deref()).bind(qq.as_deref()).bind(qq.as_deref()).bind(qq.as_deref()).bind(limit + 1);
    if srt.custom {
        qy = qy.bind((page.unwrap_or(1) - 1) * limit);
    }
    let rows = qy.fetch_all(&st.pool).await.map_err(dberr)?;
    let has_more = rows.len() as i64 > limit;
    let next = if !srt.custom { rows.get(limit as usize).map(|r| r.0.to_string()) } else { None };
    let items: Vec<_> = rows.iter().take(limit as usize).map(|r| json!({
        "id": r.0, "full_name": r.1, "email": r.2, "phone": r.3, "city": r.4,
        "status": r.5, "last_login_at": r.6, "deposit": r.7,
        "khatmil_aktif": r.8, "kunjungan_selesai": r.9,
    })).collect();
    Ok(ok_paged(&items, next, has_more, StatusCode::OK))
}

/// Kolom sort whitelist `/admin/ustadz`.
const USTADZ_SORT: &[(&str, &str)] = &[
    ("id", "u.id"),
    ("full_name", "COALESCE(NULLIF(up.full_name,''),'')"),
    ("status", "u.status"),
    ("last_login_at", "u.last_login_at"),
    ("saldo", "COALESCE(w.balance,0)"),
];

#[allow(clippy::type_complexity)]
async fn ustadz_list(cu: CurrentUser, State(st): State<AppState>, Query(q): Query<std::collections::HashMap<String, String>>) -> Result<Response, AppError> {
    cu.require("users.read")?;
    let status = q.get("status").cloned();
    let qq = q.get("q").map(|s| format!("%{s}%"));
    let limit: i64 = q.get("limit").and_then(|v| v.parse().ok()).unwrap_or(20).clamp(1, 100);
    let page: Option<i64> = q.get("page").and_then(|v| v.parse().ok()).filter(|p| *p >= 1);
    let srt = crate::shared::sorting::parse(q.get("sort").map(String::as_str), q.get("order").map(String::as_str), USTADZ_SORT, "u.id");
    let cursor: Option<i64> = if srt.custom { None } else { q.get("cursor").and_then(|v| v.parse().ok()) };
    let mut sql = format!(
        "SELECT u.id, COALESCE(NULLIF(up.full_name,''),'(tanpa nama)'), u.email, u.phone, up.city, u.status, \
         DATE_FORMAT(u.last_login_at, '%Y-%m-%dT%H:%i:%sZ'), \
         (up2.verified_at IS NOT NULL), up2.pendidikan_terakhir, vs.is_accepting, vs.price_per_hour, \
         CAST((SELECT AVG(vr.rating) FROM visit_reviews vr \
            WHERE vr.reviewee_id = u.id AND vr.direction = 'SANTRI_TO_USTADZ' \
              AND vr.revealed_at IS NOT NULL AND vr.hidden = 0) AS DOUBLE), \
         (SELECT COUNT(*) FROM visit_reviews vr \
            WHERE vr.reviewee_id = u.id AND vr.direction = 'SANTRI_TO_USTADZ' \
              AND vr.revealed_at IS NOT NULL AND vr.hidden = 0), \
         COALESCE(w.balance, 0), \
         (SELECT COUNT(*) FROM ustadz_visits v WHERE v.ustadz_id = u.id AND v.status IN ('COMPLETED','REVIEWED')) \
         FROM users u \
         JOIN user_roles ur ON ur.user_id = u.id \
         JOIN roles r ON r.id = ur.role_id AND r.code = 'USTADZ' \
         LEFT JOIN user_profiles up ON up.user_id = u.id \
         LEFT JOIN ustadz_profiles up2 ON up2.user_id = u.id \
         LEFT JOIN ustadz_visit_settings vs ON vs.ustadz_id = u.id \
         LEFT JOIN wallets w ON w.user_id = u.id \
         WHERE (? IS NULL OR u.id < ?) AND (? IS NULL OR u.status = ?) \
           AND (? IS NULL OR u.email LIKE ? OR u.phone LIKE ? \
                OR EXISTS (SELECT 1 FROM user_profiles upn2 WHERE upn2.user_id = u.id AND upn2.full_name LIKE ?)) \
         {} LIMIT ?",
        srt.order_by("u.id")
    );
    if srt.custom {
        sql.push_str(" OFFSET ?");
    }
    let mut qy = sqlx::query_as::<_, (i64, String, Option<String>, Option<String>, Option<String>, String, Option<String>, i8, Option<String>, Option<i8>, Option<i64>, Option<f64>, i64, i64, i64)>(&sql)
        .bind(cursor).bind(cursor).bind(status.as_deref()).bind(status.as_deref())
        .bind(qq.as_deref()).bind(qq.as_deref()).bind(qq.as_deref()).bind(qq.as_deref()).bind(limit + 1);
    if srt.custom {
        qy = qy.bind((page.unwrap_or(1) - 1) * limit);
    }
    let rows = qy.fetch_all(&st.pool).await.map_err(dberr)?;
    let has_more = rows.len() as i64 > limit;
    let next = if !srt.custom { rows.get(limit as usize).map(|r| r.0.to_string()) } else { None };
    let items: Vec<_> = rows.iter().take(limit as usize).map(|r| json!({
        "id": r.0, "full_name": r.1, "email": r.2, "phone": r.3, "city": r.4,
        "status": r.5, "last_login_at": r.6,
        "verified": r.7 != 0, "pendidikan": r.8,
        "is_accepting": r.9.map(|v| v != 0), "price_per_hour": r.10,
        "rating_avg": r.11, "rating_count": r.12,
        "saldo_penghasilan": r.13, "kunjungan_selesai": r.14,
    })).collect();
    Ok(ok_paged(&items, next, has_more, StatusCode::OK))
}

async fn users_patch(cu: CurrentUser, State(st): State<AppState>, Path(id): Path<i64>, Json(body): Json<serde_json::Value>) -> Result<Response, AppError> {
    cu.require("users.manage")?;
    let status = body.get("status").and_then(|v| v.as_str());
    if let Some(s) = status {
        if !matches!(s, "PENDING_VERIFICATION" | "ACTIVE" | "SUSPENDED" | "DEACTIVATED") {
            return Err(AppError::Unprocessable("status tidak valid".into()));
        }
        sqlx::query("UPDATE users SET status = ? WHERE id = ?").bind(s).bind(id)
            .execute(&st.pool).await.map_err(dberr)?;
    }
    if let Some(role) = body.get("add_role").and_then(|v| v.as_str()) {
        sqlx::query("INSERT IGNORE INTO user_roles (user_id, role_id) SELECT ?, id FROM roles WHERE code = ?")
            .bind(id).bind(role).execute(&st.pool).await.map_err(dberr)?;
    }
    if let Some(role) = body.get("remove_role").and_then(|v| v.as_str()) {
        sqlx::query("DELETE ur FROM user_roles ur JOIN roles r ON r.id = ur.role_id WHERE ur.user_id = ? AND r.code = ?")
            .bind(id).bind(role).execute(&st.pool).await.map_err(dberr)?;
    }
    let _ = sqlx::query("INSERT INTO audit_logs (actor_id, action, module, entity_type, entity_id, new_value) \
                         VALUES (?, 'UPDATE', 'users', 'users', ?, CAST(? AS JSON))")
        .bind(cu.user_id).bind(id.to_string()).bind(body.to_string())
        .execute(&st.pool).await;
    Ok(ok(json!({ "updated": true }), StatusCode::OK))
}

// ===================== detail agregat: SANTRI & USTADZ =====================

/// Total user per role (dengan filter q/status sama seperti list) — utk counter header halaman.
async fn users_count(cu: CurrentUser, State(st): State<AppState>, Query(q): Query<std::collections::HashMap<String, String>>) -> Result<Response, AppError> {
    cu.require("users.read")?;
    let role = q.get("role").filter(|r| matches!(r.as_str(), "SANTRI" | "USTADZ" | "ADMIN" | "MODERATOR")).cloned();
    let status = q.get("status").cloned();
    let qq = q.get("q").map(|s| format!("%{s}%"));
    let total: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM users u \
         WHERE (? IS NULL OR EXISTS (SELECT 1 FROM user_roles ur JOIN roles r ON r.id = ur.role_id WHERE ur.user_id = u.id AND r.code = ?)) \
           AND (? IS NULL OR u.status = ?) \
           AND (? IS NULL OR u.email LIKE ? OR u.phone LIKE ? \
                OR EXISTS (SELECT 1 FROM user_profiles upn2 WHERE upn2.user_id = u.id AND upn2.full_name LIKE ?))")
        .bind(role.as_deref()).bind(role.as_deref())
        .bind(status.as_deref()).bind(status.as_deref())
        .bind(qq.as_deref()).bind(qq.as_deref()).bind(qq.as_deref()).bind(qq.as_deref())
        .fetch_one(&st.pool).await.map_err(dberr)?;
    Ok(ok(json!({ "total": total }), StatusCode::OK))
}

async fn santri_detail(cu: CurrentUser, State(st): State<AppState>, Path(id): Path<i64>) -> Result<Response, AppError> {
    cu.require("users.read")?;
    let row: Option<(String, Option<String>, Option<String>, Option<String>, String, Option<String>, Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT COALESCE(NULLIF(up.full_name,''),'(tanpa nama)'), u.email, u.phone, up.city, u.status, \
         up.gender, DATE_FORMAT(u.created_at, '%Y-%m-%dT%H:%i:%sZ'), DATE_FORMAT(u.last_login_at, '%Y-%m-%dT%H:%i:%sZ'), up.bio \
         FROM users u LEFT JOIN user_profiles up ON up.user_id = u.id \
         WHERE u.id = ? AND EXISTS (SELECT 1 FROM user_roles ur JOIN roles r ON r.id = ur.role_id WHERE ur.user_id = u.id AND r.code = 'SANTRI')")
        .bind(id).fetch_optional(&st.pool).await.map_err(dberr)?;
    let Some(r) = row else { return Err(AppError::NotFound("santri tidak ditemukan".into())); };
    // deposit + mutasi terakhir
    let balance: i64 = sqlx::query_scalar("SELECT COALESCE((SELECT balance FROM wallets WHERE user_id = ?), 0)").bind(id)
        .fetch_one(&st.pool).await.map_err(dberr)?;
    let txs: Vec<(i64, String, i64, i64, Option<String>, Option<i64>, String)> = sqlx::query_as(
        "SELECT id, tx_type, amount, balance_after, subject_type, subject_id, \
         DATE_FORMAT(created_at, '%Y-%m-%dT%H:%i:%sZ') \
         FROM wallet_transactions WHERE user_id = ? ORDER BY id DESC LIMIT 10").bind(id)
        .fetch_all(&st.pool).await.map_err(dberr)?;
    let tx_json: Vec<_> = txs.iter().map(|t| json!({
        "id": t.0, "tx_type": t.1, "amount": t.2, "balance_after": t.3,
        "subject_type": t.4, "subject_id": t.5, "created_at": t.6,
    })).collect();
    // khatmil aktif + juz yang dipegang
    let kh: Vec<(i64, String, String)> = sqlx::query_as(
        "SELECT DISTINCT kc.id, kc.name, \
         (SELECT GROUP_CONCAT(a.juz ORDER BY a.juz) FROM khatmil_juz_assignments a WHERE a.participant_id = kp.id) \
         FROM khatmil_participants kp \
         JOIN khatmil_campaigns kc ON kc.id = kp.campaign_id AND kc.status = 'ACTIVE' \
         WHERE kp.user_id = ? ORDER BY kc.id DESC LIMIT 10").bind(id)
        .fetch_all(&st.pool).await.map_err(dberr)?;
    let kh_json: Vec<_> = kh.iter().map(|k| json!({
        "campaign_id": k.0, "name": k.1, "juz": k.2.split(',').map(|x| x.trim().to_string()).collect::<Vec<_>>(),
    })).collect();
    Ok(ok(json!({
        "id": id, "full_name": r.0, "email": r.1, "phone": r.2, "city": r.3,
        "status": r.4, "gender": r.5, "created_at": r.6, "last_login_at": r.7, "bio": r.8,
        "deposit": balance, "transactions": tx_json, "khatmil_aktif": kh_json,
    }), StatusCode::OK))
}

#[allow(clippy::type_complexity)]
async fn ustadz_detail(cu: CurrentUser, State(st): State<AppState>, Path(id): Path<i64>) -> Result<Response, AppError> {
    cu.require("users.read")?;
    let row: Option<(String, Option<String>, Option<String>, Option<String>, String, Option<String>, Option<String>, Option<String>, Option<String>, i8, Option<i64>, Option<f64>, i64)> = sqlx::query_as(
        "SELECT COALESCE(NULLIF(up.full_name,''),'(tanpa nama)'), u.email, u.phone, up.city, u.status, \
         up2.bio, up2.pendidikan_terakhir, DATE_FORMAT(u.last_login_at, '%Y-%m-%dT%H:%i:%sZ'), DATE_FORMAT(up2.verified_at, '%Y-%m-%dT%H:%i:%sZ'), \
         COALESCE(vs.is_accepting, 0), vs.price_per_hour, \
         CAST((SELECT AVG(vr.rating) FROM visit_reviews vr WHERE vr.reviewee_id = u.id AND vr.direction = 'SANTRI_TO_USTADZ' AND vr.revealed_at IS NOT NULL AND vr.hidden = 0) AS DOUBLE), \
         (SELECT COUNT(*) FROM visit_reviews vr WHERE vr.reviewee_id = u.id AND vr.direction = 'SANTRI_TO_USTADZ' AND vr.revealed_at IS NOT NULL AND vr.hidden = 0) \
         FROM users u \
         LEFT JOIN user_profiles up ON up.user_id = u.id \
         LEFT JOIN ustadz_profiles up2 ON up2.user_id = u.id \
         LEFT JOIN ustadz_visit_settings vs ON vs.ustadz_id = u.id \
         WHERE u.id = ? AND EXISTS (SELECT 1 FROM user_roles ur JOIN roles r ON r.id = ur.role_id WHERE ur.user_id = u.id AND r.code = 'USTADZ')")
        .bind(id).fetch_optional(&st.pool).await.map_err(dberr)?;
    let Some(r) = row else { return Err(AppError::NotFound("ustadz tidak ditemukan".into())); };
    // ketersediaan mingguan + libur
    let slots: Vec<(i64, i8, i64, i64)> = sqlx::query_as(
        "SELECT id, weekday, start_minute, end_minute FROM ustadz_availability_slots WHERE ustadz_id = ? ORDER BY weekday, start_minute")
        .bind(id).fetch_all(&st.pool).await.map_err(dberr)?;
    let slots_json: Vec<_> = slots.iter().map(|s| json!({ "id": s.0, "weekday": s.1, "start_minute": s.2, "end_minute": s.3 })).collect();
    let bo: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT DATE_FORMAT(off_date, '%Y-%m-%d'), note FROM ustadz_blackout_dates WHERE ustadz_id = ? ORDER BY off_date LIMIT 60")
        .bind(id).fetch_all(&st.pool).await.map_err(dberr)?;
    let bo_json: Vec<_> = bo.iter().map(|b| json!({ "off_date": b.0, "note": b.1 })).collect();
    // saldo penghasilan + penarikan terakhir
    let balance: i64 = sqlx::query_scalar("SELECT COALESCE((SELECT balance FROM wallets WHERE user_id = ?), 0)").bind(id)
        .fetch_one(&st.pool).await.map_err(dberr)?;
    let pays: Vec<(i64, i64, i64, String, String, String, String, Option<String>)> = sqlx::query_as(
        "SELECT id, amount, fee, bank_name, bank_account_no, bank_account_name, status, \
         DATE_FORMAT(created_at, '%Y-%m-%dT%H:%i:%sZ') \
         FROM payout_requests WHERE ustadz_id = ? ORDER BY id DESC LIMIT 10").bind(id)
        .fetch_all(&st.pool).await.map_err(dberr)?;
    let pays_json: Vec<_> = pays.iter().map(|p| json!({
        "id": p.0, "amount": p.1, "fee": p.2, "bank_name": p.3, "account_no": p.4,
        "account_name": p.5, "status": p.6, "created_at": p.7,
    })).collect();
    Ok(ok(json!({
        "id": id, "full_name": r.0, "email": r.1, "phone": r.2, "city": r.3,
        "status": r.4, "bio": r.5, "pendidikan": r.6, "last_login_at": r.7,
        "verified_at": r.8, "is_accepting": r.9 != 0, "price_per_hour": r.10,
        "rating_avg": r.11, "rating_count": r.12,
        "saldo_penghasilan": balance, "slots": slots_json, "blackouts": bo_json, "payouts": pays_json,
    }), StatusCode::OK))
}

// ===================== reviews admin (moderasi) =====================

#[derive(serde::Deserialize)]
struct ReviewsQ {
    direction: Option<String>, // SANTRI_TO_USTADZ | USTADZ_TO_SANTRI
    cursor: Option<i64>,
    sort: Option<String>,
    order: Option<String>,
    page: Option<i64>,
}

const REVIEWS_SORT: &[(&str, &str)] = &[
    ("id", "vr.id"),
    ("rating", "vr.rating"),
    ("created_at", "vr.created_at"),
];

async fn admin_reviews(cu: CurrentUser, State(st): State<AppState>, Query(q): Query<ReviewsQ>) -> Result<Response, AppError> {
    cu.require("visits.admin")?;
    let limit: i64 = 50;
    let page: Option<i64> = q.page.filter(|p| *p >= 1);
    let srt = crate::shared::sorting::parse(q.sort.as_deref(), q.order.as_deref(), REVIEWS_SORT, "vr.id");
    let cursor: Option<i64> = if srt.custom { None } else { q.cursor };
    let mut sql = format!(
        "SELECT vr.id, vr.visit_id, vr.direction, vr.reviewer_id, \
         COALESCE(NULLIF(rvp.full_name,''),'(tanpa nama)'), vr.rating, vr.comment, vr.hidden, \
         DATE_FORMAT(vr.revealed_at, '%Y-%m-%dT%H:%i:%sZ'), DATE_FORMAT(vr.created_at, '%Y-%m-%dT%H:%i:%sZ') \
         FROM visit_reviews vr \
         LEFT JOIN user_profiles rvp ON rvp.user_id = vr.reviewer_id \
         WHERE (? IS NULL OR vr.direction = ?) AND (? IS NULL OR vr.id < ?) \
         {} LIMIT ?",
        srt.order_by("vr.id")
    );
    if srt.custom {
        sql.push_str(" OFFSET ?");
    }
    let mut qy = sqlx::query_as::<_, (i64, i64, String, i64, String, i8, Option<String>, i8, Option<String>, String)>(&sql)
        .bind(q.direction.as_deref()).bind(q.direction.as_deref())
        .bind(cursor).bind(cursor).bind(limit + 1);
    if srt.custom {
        qy = qy.bind((page.unwrap_or(1) - 1) * limit);
    }
    let rows = qy.fetch_all(&st.pool).await.map_err(dberr)?;
    let has_more = rows.len() as i64 > limit;
    let next = if !srt.custom { rows.get(limit as usize).map(|r| r.0.to_string()) } else { None };
    let items: Vec<_> = rows.iter().take(limit as usize).map(|r| json!({
        "id": r.0, "visit_id": r.1, "direction": r.2, "reviewer_id": r.3,
        "reviewer_name": r.4, "rating": r.5, "comment": r.6, "hidden": r.7 != 0,
        "revealed": r.8.is_some(), "created_at": r.9,
    })).collect();
    Ok(ok_paged(&items, next, has_more, StatusCode::OK))
}

async fn admin_review_hide(cu: CurrentUser, State(st): State<AppState>, Path(id): Path<i64>) -> Result<Response, AppError> {
    cu.require("visits.admin")?;
    sqlx::query("UPDATE visit_reviews SET hidden = 1 WHERE id = ?").bind(id)
        .execute(&st.pool).await.map_err(dberr)?;
    Ok(ok(json!({ "hidden": true }), StatusCode::OK))
}

async fn admin_review_unhide(cu: CurrentUser, State(st): State<AppState>, Path(id): Path<i64>) -> Result<Response, AppError> {
    cu.require("visits.admin")?;
    sqlx::query("UPDATE visit_reviews SET hidden = 0 WHERE id = ?").bind(id)
        .execute(&st.pool).await.map_err(dberr)?;
    Ok(ok(json!({ "hidden": false }), StatusCode::OK))
}

// ===================== counter generik utk header halaman =====================

async fn admin_count(cu: CurrentUser, State(st): State<AppState>, Query(q): Query<std::collections::HashMap<String, String>>) -> Result<Response, AppError> {
    cu.require("users.read")?;
    let status = q.get("status").cloned();
    let entity = q.get("entity").cloned().unwrap_or_default();
    let total: i64 = match entity.as_str() {
        "visits" => sqlx::query_scalar(
            "SELECT COUNT(*) FROM ustadz_visits WHERE (? IS NULL OR status = ?)")
            .bind(status.as_deref()).bind(status.as_deref())
            .fetch_one(&st.pool).await.map_err(dberr)?,
        "payments" => {
            let stype = q.get("subject_type").cloned();
            sqlx::query_scalar(
                "SELECT COUNT(*) FROM payments WHERE (? IS NULL OR status = ?) AND (? IS NULL OR subject_type = ?)")
                .bind(status.as_deref()).bind(status.as_deref())
                .bind(stype.as_deref()).bind(stype.as_deref())
                .fetch_one(&st.pool).await.map_err(dberr)?
        }
        "payouts" => sqlx::query_scalar(
            "SELECT COUNT(*) FROM payout_requests WHERE (? IS NULL OR status = ?)")
            .bind(status.as_deref()).bind(status.as_deref())
            .fetch_one(&st.pool).await.map_err(dberr)?,
        "reviews" => {
            let dir = q.get("direction").cloned();
            sqlx::query_scalar(
                "SELECT COUNT(*) FROM visit_reviews WHERE (? IS NULL OR direction = ?)")
                .bind(dir.as_deref()).bind(dir.as_deref())
                .fetch_one(&st.pool).await.map_err(dberr)?
        }
        _ => return Err(AppError::Unprocessable("entity tidak dikenal (visits|payments|payouts|reviews)".into())),
    };
    Ok(ok(json!({ "total": total }), StatusCode::OK))
}
