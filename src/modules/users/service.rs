//! Service modul users.
use sqlx::MySqlPool;

use crate::modules::users::dto::*;
use crate::shared::error::AppError;

fn dberr(e: sqlx::Error) -> AppError {
    tracing::error!("db: {e}");
    AppError::Internal(format!("db: {e}"))
}

pub async fn patch_me(pool: &MySqlPool, user_id: i64, req: PatchMeReq) -> Result<(), AppError> {
    if let Some(g) = req.gender.as_deref() {
        if !matches!(g, "MALE" | "FEMALE") {
            return Err(AppError::Unprocessable("gender harus MALE/FEMALE".into()));
        }
    }
    if let Some(d) = req.birth_date.as_deref() {
        chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d")
            .map_err(|_| AppError::Unprocessable("birth_date harus YYYY-MM-DD".into()))?;
    }
    sqlx::query(
        "INSERT INTO user_profiles (user_id, full_name, gender, birth_date, address_text, city, province, photo_media_id, bio) \
         VALUES (?, COALESCE(?, 'Belum Diisi'), ?, ?, ?, ?, ?, ?, ?) AS new \
         ON DUPLICATE KEY UPDATE full_name = COALESCE(new.full_name, user_profiles.full_name), gender = COALESCE(new.gender, user_profiles.gender), \
         birth_date = COALESCE(new.birth_date, user_profiles.birth_date), address_text = COALESCE(new.address_text, user_profiles.address_text), \
         city = COALESCE(new.city, user_profiles.city), province = COALESCE(new.province, user_profiles.province), \
         photo_media_id = COALESCE(new.photo_media_id, user_profiles.photo_media_id), bio = COALESCE(new.bio, user_profiles.bio)")
        .bind(user_id)
        .bind(req.full_name).bind(req.gender)
        .bind(req.birth_date.as_deref().and_then(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()))
        .bind(req.address_text).bind(req.city).bind(req.province)
        .bind(req.photo_media_id).bind(req.bio)
        .execute(pool).await.map_err(dberr)?;
    Ok(())
}

/// DELETE /me — PDP keputusan #15: soft-DELETED + anonymize PII, histori sah dipertahankan.
pub async fn delete_me(pool: &MySqlPool, user_id: i64) -> Result<(), AppError> {
    let mut tx = pool.begin().await.map_err(dberr)?;
    sqlx::query("UPDATE users SET phone = NULL, email = NULL, password_hash = NULL, status = 'DELETED' WHERE id = ?")
        .bind(user_id).execute(&mut *tx).await.map_err(dberr)?;
    sqlx::query("UPDATE user_profiles SET full_name = 'Akun Terhapus', gender = NULL, birth_date = NULL, \
                 address_text = NULL, city = NULL, province = NULL, photo_media_id = NULL, bio = NULL WHERE user_id = ?")
        .bind(user_id).execute(&mut *tx).await.map_err(dberr)?;
    sqlx::query("UPDATE ustadz_profiles SET code = NULL, title = NULL, bio = NULL, photo_media_id = NULL, \
                 is_accepting_questions = 0 WHERE user_id = ?")
        .bind(user_id).execute(&mut *tx).await.map_err(dberr)?;
    sqlx::query("UPDATE user_sessions SET revoked_at = UTC_TIMESTAMP() WHERE user_id = ? AND revoked_at IS NULL")
        .bind(user_id).execute(&mut *tx).await.map_err(dberr)?;
    sqlx::query("UPDATE user_devices SET is_active = 0 WHERE user_id = ?")
        .bind(user_id).execute(&mut *tx).await.map_err(dberr)?;
    sqlx::query("INSERT INTO activity_events (user_id, event_type, payload) VALUES (?, 'auth.account_deleted', CAST('{}' AS JSON))")
        .bind(user_id).execute(&mut *tx).await.map_err(dberr)?;
    tx.commit().await.map_err(dberr)?;
    Ok(())
}

pub async fn register_device(
    pool: &MySqlPool,
    user_id: i64,
    req: DeviceReq,
    is_ustadz: bool,
) -> Result<DeviceOut, AppError> {
    if !matches!(req.platform.as_str(), "ANDROID" | "IOS" | "WEB") {
        return Err(AppError::Unprocessable("platform harus ANDROID/IOS/WEB".into()));
    }
    if !matches!(req.app_id.as_str(), "SANTRI_APP" | "USTADZ_APP" | "ADMIN_WEB") {
        return Err(AppError::Unprocessable("app_id tidak valid".into()));
    }
    if req.app_id == "USTADZ_APP" && !is_ustadz {
        return Err(AppError::Forbidden("app_id USTADZ_APP hanya untuk ustadz".into()));
    }
    // token dipunyai user lain -> 409
    let owner: Option<(i64,)> = sqlx::query_as("SELECT user_id FROM user_devices WHERE push_token = ?")
        .bind(&req.push_token).fetch_optional(pool).await.map_err(dberr)?;
    if let Some((oid,)) = owner {
        if oid != user_id {
            return Err(AppError::Conflict("push_token sudah dipakai device lain".into()));
        }
    }
    sqlx::query(
        "INSERT INTO user_devices (user_id, platform, push_token, device_name, app_version, app_id, is_active, last_seen_at) \
         VALUES (?, ?, ?, ?, ?, ?, 1, UTC_TIMESTAMP()) AS new \
         ON DUPLICATE KEY UPDATE device_name = new.device_name, app_version = new.app_version, \
         app_id = new.app_id, is_active = 1, last_seen_at = UTC_TIMESTAMP(), platform = new.platform")
        .bind(user_id).bind(&req.platform).bind(&req.push_token)
        .bind(req.device_name).bind(req.app_version).bind(&req.app_id)
        .execute(pool).await.map_err(dberr)?;

    let row: (i64, String, String, Option<String>, Option<String>, i8, Option<String>) = sqlx::query_as(
        "SELECT id, platform, app_id, device_name, app_version, is_active, DATE_FORMAT(last_seen_at, '%Y-%m-%dT%H:%i:%sZ') FROM user_devices WHERE push_token = ?")
        .bind(&req.push_token).fetch_one(pool).await.map_err(dberr)?;
    Ok(DeviceOut {
        id: row.0, platform: row.1, app_id: row.2, device_name: row.3,
        app_version: row.4, is_active: row.5 != 0, last_seen_at: row.6,
    })
}

pub async fn list_devices(pool: &MySqlPool, user_id: i64) -> Result<Vec<DeviceOut>, AppError> {
    let rows: Vec<(i64, String, String, Option<String>, Option<String>, i8, Option<String>)> = sqlx::query_as(
        "SELECT id, platform, app_id, device_name, app_version, is_active, DATE_FORMAT(last_seen_at, '%Y-%m-%dT%H:%i:%sZ') \
         FROM user_devices WHERE user_id = ? ORDER BY id")
        .bind(user_id).fetch_all(pool).await.map_err(dberr)?;
    Ok(rows.into_iter().map(|r| DeviceOut {
        id: r.0, platform: r.1, app_id: r.2, device_name: r.3,
        app_version: r.4, is_active: r.5 != 0, last_seen_at: r.6,
    }).collect())
}

pub async fn delete_device(pool: &MySqlPool, user_id: i64, device_id: i64) -> Result<(), AppError> {
    let n = sqlx::query("DELETE FROM user_devices WHERE id = ? AND user_id = ?")
        .bind(device_id).bind(user_id)
        .execute(pool).await.map_err(dberr)?.rows_affected();
    if n == 0 {
        return Err(AppError::NotFound("device tidak ditemukan".into()));
    }
    Ok(())
}

pub async fn progress_summary(pool: &MySqlPool, user_id: i64) -> Result<ProgressSummary, AppError> {
    let (ayat_passed, surah_started, total_setoran, setoran_pending): (i64, i64, i64, i64) = sqlx::query_as(
        "SELECT \
           (SELECT CAST(COALESCE(SUM(passed_count), 0) AS SIGNED) FROM memorization_progress WHERE user_id = ?), \
           (SELECT COUNT(*) FROM memorization_progress WHERE user_id = ?), \
           (SELECT COUNT(*) FROM memorization_submissions WHERE user_id = ?), \
           (SELECT COUNT(*) FROM memorization_submissions WHERE user_id = ? AND status IN ('PENDING','IN_REVIEW'))")
        .bind(user_id).bind(user_id).bind(user_id).bind(user_id)
        .fetch_one(pool).await.map_err(dberr)?;
    let last: Option<(i64, String, i64, i64)> = sqlx::query_as(
        "SELECT qa.surah_id, qs.name_id, qa.ayah_number, qa.page \
         FROM user_reading_progress urp JOIN quran_ayahs qa ON qa.id = urp.last_ayah_id \
         JOIN quran_surahs qs ON qs.id = qa.surah_id WHERE urp.user_id = ?")
        .bind(user_id).fetch_optional(pool).await.map_err(dberr)?;
    Ok(ProgressSummary {
        ayat_passed, surah_started, total_setoran, setoran_pending,
        last_read: last.map(|(s, n, a, p)| LastRead { surah_id: s, surah_name: n, ayah_number: a, page: p }),
    })
}
