//! Service modul memorization.
use sqlx::MySqlPool;

use crate::infrastructure::storage::Storage;
use crate::modules::memorization::dto::*;
use crate::shared::error::AppError;

fn dberr(e: sqlx::Error) -> AppError {
    tracing::error!("db: {e}");
    AppError::Internal(format!("db: {e}"))
}

const PRESIGN_SECS: u64 = 15 * 60;

type SubRow = (
    i64, i64, Option<String>, i64, String, i64, i64, i64, Option<i32>,
    Option<String>, String, Option<i64>, String, Option<String>,
);

const SUB_SELECT: &str = "SELECT ms.id, ms.user_id, \
    (SELECT p.full_name FROM user_profiles p WHERE p.user_id = ms.user_id), \
    ms.surah_id, (SELECT s.name_id FROM quran_surahs s WHERE s.id = ms.surah_id), \
    ms.ayah_start, ms.ayah_end, ms.audio_media_id, ms.duration_ms, ms.note, ms.status, \
    ms.ustadz_id, DATE_FORMAT(ms.submitted_at, '%Y-%m-%dT%H:%i:%sZ'), \
    DATE_FORMAT(ms.reviewed_at, '%Y-%m-%dT%H:%i:%sZ') \
    FROM memorization_submissions ms";

fn to_out(r: SubRow) -> SubmissionOut {
    SubmissionOut {
        id: r.0, user_id: r.1, user_name: r.2, surah_id: r.3, surah_name: r.4,
        ayah_start: r.5, ayah_end: r.6, audio_media_id: r.7, duration_ms: r.8,
        note: r.9, status: r.10, ustadz_id: r.11, submitted_at: r.12, reviewed_at: r.13,
    }
}

/// Validasi media AUDIO READY milik user; kembalikan (id, storage_key, duration_ms).
async fn own_audio(pool: &MySqlPool, user_id: i64, media_id: i64) -> Result<(i64, String, Option<i32>), AppError> {
    let row: Option<(i64, String, String, Option<i32>)> = sqlx::query_as(
        "SELECT id, storage_key, status, duration_ms FROM media WHERE id = ? AND owner_id = ? AND kind = 'AUDIO'")
        .bind(media_id).bind(user_id)
        .fetch_optional(pool).await.map_err(dberr)?;
    match row {
        Some((id, key, status, dur)) if status == "READY" => Ok((id, key, dur)),
        Some((_, _, status, _)) => Err(AppError::Unprocessable(format!("media berstatus {status} — butuh READY"))),
        None => Err(AppError::Unprocessable("audio_media_id bukan media AUDIO milik anda".into())),
    }
}

pub async fn submit(
    pool: &MySqlPool, user_id: i64, req: SubmitReq, client_key: Option<String>,
) -> Result<(SubmissionOut, bool), AppError> {
    if let Some(k) = &client_key {
        if k.len() > 64 || k.is_empty() {
            return Err(AppError::Unprocessable("Idempotency-Key 1-64 karakter".into()));
        }
    } else {
        return Err(AppError::Unprocessable("header Idempotency-Key wajib".into()));
    }
    if req.ayah_start < 1 || req.ayah_end < req.ayah_start {
        return Err(AppError::Unprocessable("rentang ayah tidak valid".into()));
    }
    let surah: Option<(i64,)> = sqlx::query_as(
        "SELECT ayah_count FROM quran_surahs WHERE id = ?")
        .bind(req.surah_id).fetch_optional(pool).await.map_err(dberr)?;
    let (count,) = surah.ok_or_else(|| AppError::Unprocessable("surah_id tidak ada".into()))?;
    if req.ayah_end > count as i64 {
        return Err(AppError::Unprocessable(format!("surah hanya {count} ayat")));
    }
    let (media_id, _key, duration_ms) = own_audio(pool, user_id, req.audio_media_id).await?;

    // replay check DULU — deterministik (affected_rows ODKU no-op driver-dependent: sqlx mysql = 1)
    let existing: Option<SubRow> = sqlx::query_as(
        &format!("{SUB_SELECT} WHERE ms.user_id = ? AND ms.client_key = ?"))
        .bind(user_id).bind(client_key.as_deref())
        .fetch_optional(pool).await.map_err(dberr)?;
    if let Some(row) = existing {
        return Ok((to_out(row), false)); // replay idempotent (200)
    }
    sqlx::query(
        "INSERT INTO memorization_submissions \
         (user_id, surah_id, ayah_start, ayah_end, audio_media_id, duration_ms, note, client_key) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)")
        .bind(user_id).bind(req.surah_id).bind(req.ayah_start).bind(req.ayah_end)
        .bind(media_id).bind(duration_ms).bind(&req.note).bind(client_key.as_deref())
        .execute(pool).await.map_err(dberr)?;
    let row: SubRow = sqlx::query_as(&format!("{SUB_SELECT} WHERE ms.user_id = ? AND ms.client_key = ?"))
        .bind(user_id).bind(client_key.as_deref())
        .fetch_one(pool).await.map_err(dberr)?;
    sqlx::query("INSERT INTO activity_events (user_id, event_type, ref_type, ref_id, payload) \
                 VALUES (?, 'memorization.submitted', 'memorization_submission', ?, CAST(? AS JSON))")
        .bind(user_id).bind(row.0.to_string())
        .bind(format!("{{\"surah_id\":{},\"ayah_start\":{},\"ayah_end\":{}}}", req.surah_id, req.ayah_start, req.ayah_end))
        .execute(pool).await.map_err(dberr)?;
    Ok((to_out(row), true))
}

pub async fn my_submissions(
    pool: &MySqlPool, user_id: i64, cursor: Option<i64>, limit: usize,
) -> Result<(Vec<SubmissionOut>, Option<String>, bool), AppError> {
    let rows: Vec<SubRow> = sqlx::query_as(&format!(
        "{SUB_SELECT} WHERE ms.user_id = ? AND (? IS NULL OR ms.id < ?) ORDER BY ms.id DESC LIMIT ?"))
        .bind(user_id).bind(cursor).bind(cursor).bind((limit + 1) as i64)
        .fetch_all(pool).await.map_err(dberr)?;
    let has_more = rows.len() > limit;
    let items = rows.into_iter().take(limit).map(to_out).collect::<Vec<_>>();
    let next = if has_more { items.last().map(|s| s.id.to_string()) } else { None };
    Ok((items, next, has_more))
}

/// ACL: pemilik / ustadz ter-assign atau ber-permission review / admin (users.read).
pub async fn detail(
    pool: &MySqlPool, storage: &Storage, viewer_id: i64, viewer_can_review: bool, viewer_admin: bool, id: i64,
) -> Result<SubmissionDetail, AppError> {
    let row: Option<SubRow> = sqlx::query_as(&format!("{SUB_SELECT} WHERE ms.id = ?"))
        .bind(id).fetch_optional(pool).await.map_err(dberr)?;
    let sub = to_out(row.ok_or_else(|| AppError::NotFound("setoran tidak ditemukan".into()))?);
    let allowed = sub.user_id == viewer_id || viewer_admin || viewer_can_review;
    if !allowed {
        return Err(AppError::NotFound("setoran tidak ditemukan".into()));
    }
    let audio_url = media_presign(pool, storage, sub.audio_media_id).await.ok();
    let review: Option<(i64, String, Option<String>, Option<i64>, String)> = sqlx::query_as(
        "SELECT reviewer_id, verdict, notes, reply_audio_media_id, DATE_FORMAT(created_at, '%Y-%m-%dT%H:%i:%sZ') \
         FROM memorization_reviews WHERE submission_id = ?")
        .bind(id).fetch_optional(pool).await.map_err(dberr)?;
    let review_out = match review {
        Some((rid, verdict, notes, reply_id, created)) => {
            let reply_url = match reply_id {
                Some(mid) => media_presign(pool, storage, mid).await.ok(),
                None => None,
            };
            Some(ReviewOut { reviewer_id: rid, verdict, notes, reply_audio_media_id: reply_id, reply_audio_presigned_url: reply_url, created_at: created })
        }
        None => None,
    };
    Ok(SubmissionDetail { submission: sub, audio_presigned_url: audio_url, review: review_out })
}

async fn media_presign(pool: &MySqlPool, storage: &Storage, media_id: i64) -> Result<String, AppError> {
    let (key,): (String,) = sqlx::query_as("SELECT storage_key FROM media WHERE id = ? AND status = 'READY'")
        .bind(media_id).fetch_one(pool).await.map_err(dberr)?;
    storage.presign_get(&key, PRESIGN_SECS).await
        .map_err(|e| { tracing::error!("{e}"); AppError::Internal("presign".into()) })
}

pub async fn my_progress(pool: &MySqlPool, user_id: i64) -> Result<Vec<ProgressRow>, AppError> {
    let rows: Vec<(i64, String, i64, i64)> = sqlx::query_as(
        "SELECT mp.surah_id, s.name_id, mp.last_passed_ayah, mp.passed_count \
         FROM memorization_progress mp JOIN quran_surahs s ON s.id = mp.surah_id \
         WHERE mp.user_id = ? ORDER BY mp.surah_id")
        .bind(user_id).fetch_all(pool).await.map_err(dberr)?;
    Ok(rows.into_iter().map(|r| ProgressRow {
        surah_id: r.0, surah_name: r.1, last_passed_ayah: r.2, passed_count: r.3,
    }).collect())
}

pub async fn queue(
    pool: &MySqlPool, ustadz_id: i64, status: Option<String>, cursor: Option<i64>, limit: usize,
) -> Result<(Vec<SubmissionOut>, Option<String>, bool), AppError> {
    // default: PENDING global + IN_REVIEW milik saya
    let rows: Vec<SubRow> = sqlx::query_as(&format!(
        "{SUB_SELECT} WHERE (? IS NULL OR ms.id < ?) AND \
         (CASE WHEN ? = 'PENDING' THEN ms.status = 'PENDING' AND ms.ustadz_id IS NULL \
               WHEN ? = 'IN_REVIEW' THEN ms.status = 'IN_REVIEW' AND ms.ustadz_id = ? \
               ELSE (ms.status = 'PENDING' AND ms.ustadz_id IS NULL) OR (ms.status = 'IN_REVIEW' AND ms.ustadz_id = ?) END) \
         ORDER BY ms.submitted_at ASC, ms.id ASC LIMIT ?"))
        .bind(cursor).bind(cursor)
        .bind(status.as_deref()).bind(status.as_deref()).bind(ustadz_id).bind(ustadz_id)
        .bind((limit + 1) as i64)
        .fetch_all(pool).await.map_err(dberr)?;
    let has_more = rows.len() > limit;
    let items = rows.into_iter().take(limit).map(to_out).collect::<Vec<_>>();
    let next = if has_more { items.last().map(|s| s.id.to_string()) } else { None };
    Ok((items, next, has_more))
}

pub async fn review(
    pool: &MySqlPool, storage: &Storage, ustadz_id: i64, submission_id: i64, req: ReviewReq,
) -> Result<SubmissionDetail, AppError> {
    let mut tx = pool.begin().await.map_err(dberr)?;
    match req.action.as_str() {
        "CLAIM" => {
            // atomic: hanya PENDING-tanpa-ustadz, atau IN_REVIEW milik saya (idempotent)
            let n = sqlx::query(
                "UPDATE memorization_submissions SET status = 'IN_REVIEW', ustadz_id = ? \
                 WHERE id = ? AND ((status = 'PENDING' AND ustadz_id IS NULL) OR (status = 'IN_REVIEW' AND ustadz_id = ?))")
                .bind(ustadz_id).bind(submission_id).bind(ustadz_id)
                .execute(&mut *tx).await.map_err(dberr)?.rows_affected();
            if n == 0 {
                return Err(AppError::Conflict("setoran sudah diklaim ustadz lain / status tidak cocok".into()));
            }
        }
        "SUBMIT" => {
            let verdict = req.verdict
                .ok_or_else(|| AppError::Unprocessable("verdict wajib utk SUBMIT".into()))?;
            if !matches!(verdict.as_str(), "PASSED" | "REVISION" | "REJECTED") {
                return Err(AppError::Unprocessable("verdict harus PASSED/REVISION/REJECTED".into()));
            }
            let cur: Option<(String, i64, i64)> = sqlx::query_as(
                "SELECT status, user_id, IFNULL(ustadz_id, -1) FROM memorization_submissions WHERE id = ?")
                .bind(submission_id).fetch_optional(&mut *tx).await.map_err(dberr)?;
            let (status, santri_id, owner_ustadz) = cur
                .ok_or_else(|| AppError::NotFound("setoran tidak ditemukan".into()))?;
            if status != "IN_REVIEW" || owner_ustadz != ustadz_id {
                return Err(AppError::Conflict("setoran belum IN_REVIEW milik anda (CLAIM dulu)".into()));
            }
            let reply_media: Option<i64> = match req.reply_audio_media_id {
                Some(mid) => {
                    // voice note balasan: media READY milik USTADZ (bukan santri)
                    let m: Option<(i64,)> = sqlx::query_as(
                        "SELECT id FROM media WHERE id = ? AND owner_id = ? AND kind = 'AUDIO' AND status = 'READY'")
                        .bind(mid).bind(ustadz_id).fetch_optional(&mut *tx).await.map_err(dberr)?;
                    Some(m.ok_or_else(|| AppError::Unprocessable("reply_audio_media_id tidak valid".into()))?.0)
                }
                None => None,
            };
            let ins = sqlx::query(
                "INSERT INTO memorization_reviews (submission_id, reviewer_id, verdict, notes, reply_audio_media_id) \
                 VALUES (?, ?, ?, ?, ?) AS new \
                 ON DUPLICATE KEY UPDATE verdict = new.verdict, notes = new.notes, reply_audio_media_id = new.reply_audio_media_id")
                .bind(submission_id).bind(ustadz_id).bind(&verdict).bind(&req.notes).bind(reply_media)
                .execute(&mut *tx).await.map_err(dberr)?;
            if ins.rows_affected() == 0 && false { unreachable!() }
            sqlx::query("UPDATE memorization_submissions SET status = ?, reviewed_at = UTC_TIMESTAMP() WHERE id = ?")
                .bind(&verdict).bind(submission_id)
                .execute(&mut *tx).await.map_err(dberr)?;
            if verdict == "PASSED" {
                // rollup deterministik dari tabel submissions (bukan increment)
                let (start, end, surah): (i64, i64, i64) = sqlx::query_as(
                    "SELECT ayah_start, ayah_end, surah_id FROM memorization_submissions WHERE id = ?")
                    .bind(submission_id).fetch_one(&mut *tx).await.map_err(dberr)?;
                let (passed_count,): (i64,) = sqlx::query_as(
                    "SELECT COUNT(*) FROM memorization_submissions WHERE user_id = ? AND surah_id = ? AND status = 'PASSED'")
                    .bind(santri_id).bind(surah).fetch_one(&mut *tx).await.map_err(dberr)?;
                sqlx::query(
                    "INSERT INTO memorization_progress (user_id, surah_id, last_passed_ayah, passed_count) \
                     VALUES (?, ?, ?, ?) AS new \
                     ON DUPLICATE KEY UPDATE last_passed_ayah = GREATEST(new.last_passed_ayah, memorization_progress.last_passed_ayah), passed_count = new.passed_count")
                    .bind(santri_id).bind(surah).bind(end).bind(passed_count)
                    .execute(&mut *tx).await.map_err(dberr)?;
            }
            sqlx::query("INSERT INTO activity_events (user_id, event_type, ref_type, ref_id, payload) \
                         VALUES (?, 'memorization.reviewed', 'memorization_submission', ?, CAST(? AS JSON))")
                .bind(santri_id).bind(submission_id.to_string())
                .bind(format!("{{\"verdict\":\"{verdict}\",\"ustadz_id\":{ustadz_id}}}"))
                .execute(&mut *tx).await.map_err(dberr)?;
            // notif in-app santri (modul 10 tinggal fan-out/push)
            sqlx::query(
                "INSERT INTO user_notifications (user_id, template_code, title, body, data, channel) \
                 VALUES (?, 'SETORAN_REVIEWED', 'Setoran hafalan diperiksa', ?, CAST(? AS JSON), 'IN_APP')")
                .bind(santri_id)
                .bind(format!("Setoran surah telah diperiksa ustadz: {verdict}"))
                .bind(format!("{{\"deeplink\":\"submission:{submission_id}\",\"verdict\":\"{verdict}\"}}"))
                .execute(&mut *tx).await.map_err(dberr)?;
        }
        other => return Err(AppError::Unprocessable(format!("action harus CLAIM/SUBMIT (dapat {other})"))),
    }
    tx.commit().await.map_err(dberr)?;
    detail(pool, storage, ustadz_id, true, false, submission_id).await
}
