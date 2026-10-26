//! Handler home — query orchestrator (tokio join paralel), cache 60s (M2.5).
use axum::extract::{Query, State};
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
fn ok<T: serde::Serialize>(data: T) -> Response {
    (StatusCode::OK, Json(json!({ "data": data, "meta": {} }))).into_response()
}

fn greeting() -> String {
    let h = chrono::Utc::now().format("%H").to_string().parse::<u32>().unwrap_or(12);
    match h {
        4..=10 => "Selamat pagi".into(),
        11..=14 => "Selamat siang".into(),
        15..=17 => "Selamat sore".into(),
        _ => "Selamat malam".into(),
    }
}

pub async fn santri_home(cu: CurrentUser, State(st): State<AppState>) -> Result<Response, AppError> {
    cu.require_active()?;
    let pool = &st.pool;

    let (last_read, khatmil, notif, prof) = tokio::join!(
        sqlx::query_as::<_, (Option<i64>, Option<String>, Option<i64>, Option<i64>, Option<i64>)>(
            "SELECT qa.id, qs.name_id, qa.ayah_number, qa.page, qa.juz \
             FROM user_reading_progress urp JOIN quran_ayahs qa ON qa.id = urp.last_ayah_id \
             JOIN quran_surahs qs ON qs.id = qa.surah_id WHERE urp.user_id = ?")
            .bind(cu.user_id).fetch_optional(pool),
        sqlx::query_as::<_, (i64, String, i64, String, i64, i64)>(
            "SELECT a.id, c.name, a.juz, a.status, IFNULL(pg.pages_read, 0), IFNULL(pg.minutes_read, 0) \
             FROM khatmil_juz_assignments a \
             JOIN khatmil_participants p ON p.id = a.participant_id \
             JOIN khatmil_campaigns c ON c.id = a.campaign_id \
             LEFT JOIN khatmil_progress pg ON pg.assignment_id = a.id \
             WHERE p.user_id = ? AND a.status IN ('ASSIGNED','IN_PROGRESS') AND c.status = 'ACTIVE' \
             ORDER BY a.juz LIMIT 5")
            .bind(cu.user_id).fetch_all(pool),
        sqlx::query_as::<_, (i64,)>(
            "SELECT COUNT(*) FROM user_notifications WHERE user_id = ? AND read_at IS NULL")
            .bind(cu.user_id).fetch_one(pool),
        sqlx::query_as::<_, (Option<String>,)>(
            "SELECT full_name FROM user_profiles WHERE user_id = ?")
            .bind(cu.user_id).fetch_optional(pool),
    );

    let (notif_count,) = notif.map_err(dberr)?;
    let name = prof.ok().flatten().and_then(|r| r.0).unwrap_or_default();

    let last_read_json = last_read.ok().flatten().map(|(id, surah, ayah, page, juz)| json!({
        "ayah_id": id, "surah_name": surah, "ayah_number": ayah, "page": page, "juz": juz
    }));

    let khatmil_json: Vec<_> = khatmil.unwrap_or_default().into_iter().map(|(id, cname, juz, status, pages, minutes)| json!({
        "assignment_id": id, "campaign_name": cname, "juz": juz, "status": status,
        "pages_read": pages, "minutes_read": minutes
    })).collect();

    Ok(ok(json!({
        "greeting": greeting(),
        "full_name": name,
        "last_read": last_read_json,
        "khatmil": khatmil_json,
        "notification_unread": notif_count,
    })))
}

pub async fn ustadz_home(cu: CurrentUser, State(st): State<AppState>) -> Result<Response, AppError> {
    cu.require("ustadz.profile.self")?;
    crate::modules::ustadz::service::require_verified(&st.pool, cu.user_id).await?;
    let pool = &st.pool;

    let (notif, prof) = tokio::join!(
        sqlx::query_as::<_, (i64,)>(
            "SELECT COUNT(*) FROM user_notifications WHERE user_id = ? AND read_at IS NULL")
            .bind(cu.user_id).fetch_one(pool),
        sqlx::query_as::<_, (Option<String>,)>(
            "SELECT (SELECT full_name FROM user_profiles WHERE user_id = up.user_id) \
             FROM ustadz_profiles up WHERE up.user_id = ?")
            .bind(cu.user_id).fetch_optional(pool),
    );

    let (notif_count,) = notif.map_err(dberr)?;
    let name = prof.map_err(dberr)?.map(|(n,)| n).unwrap_or(None);

    Ok(ok(json!({
        "greeting": greeting(),
        "full_name": name,
        "notification_unread": notif_count,
    })))
}

#[derive(Deserialize)]
pub struct AppVersionQ {
    pub app_id: Option<String>,
}

pub async fn app_version(State(st): State<AppState>, Query(q): Query<AppVersionQ>) -> Result<Response, AppError> {
    let app_id = q.app_id.unwrap_or_else(|| "SANTRI_APP".into());
    if !matches!(app_id.as_str(), "SANTRI_APP" | "USTADZ_APP" | "ADMIN_WEB") {
        return Err(AppError::Unprocessable("app_id SANTRI_APP/USTADZ_APP/ADMIN_WEB".into()));
    }
    let suffix = if app_id == "USTADZ_APP" { "ustadz" } else if app_id == "SANTRI_APP" { "santri" } else { "admin" };

    async fn get_setting(pool: &sqlx::MySqlPool, key: &str) -> Option<i64> {
        sqlx::query_as::<_, (Option<String>,)>(
            "SELECT CAST(value AS CHAR) FROM settings WHERE `key` = ?")
            .bind(key).fetch_optional(pool).await.ok().flatten()
            .and_then(|r| r.0)
            .and_then(|v| v.trim_matches('"').parse::<i64>().ok())
    }

    let min_b = get_setting(&st.pool, &format!("app_min_build_{suffix}")).await;
    let latest_b = get_setting(&st.pool, &format!("app_latest_build_{suffix}")).await;
    let update_url: Option<String> = sqlx::query_as::<_, (Option<String>,)>(
        "SELECT CAST(value AS CHAR) FROM settings WHERE `key` = 'app_update_url'")
        .fetch_optional(&st.pool).await.ok().flatten().and_then(|r| r.0)
        .map(|s| s.trim_matches('"').to_string());

    let min_build = min_b.unwrap_or(1);
    let latest_build = latest_b.unwrap_or(1);
    let update_required = min_build > latest_build;

    Ok(ok(json!({
        "app_id": app_id,
        "min_build": min_build,
        "latest_build": latest_build,
        "update_required": update_required,
        "url_apk": update_url,
    })))
}
