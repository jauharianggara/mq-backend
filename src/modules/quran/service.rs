//! Service modul quran.
use sqlx::MySqlPool;

use crate::modules::quran::dto::*;
use crate::shared::error::AppError;
use crate::shared::pagination::CursorPage;

fn dberr(e: sqlx::Error) -> AppError {
    tracing::error!("db: {e}");
    AppError::Internal(format!("db: {e}"))
}

pub async fn surahs(pool: &MySqlPool) -> Result<Vec<SurahOut>, AppError> {
    let rows: Vec<(i64, String, String, String, i64, String)> = sqlx::query_as(
        "SELECT id, name_arabic, name_latin, name_id, ayah_count, revelation FROM quran_surahs ORDER BY id")
        .fetch_all(pool).await.map_err(dberr)?;
    Ok(rows.into_iter().map(|r| SurahOut {
        id: r.0, name_arabic: r.1, name_latin: r.2, name_id: r.3, ayah_count: r.4, revelation: r.5,
    }).collect())
}

pub async fn ayahs(
    pool: &MySqlPool, surah_id: i64, translator: &str, cursor: Option<i64>, limit: usize,
) -> Result<CursorPage<AyahOut>, AppError> {
    if !(1..=114).contains(&surah_id) {
        return Err(AppError::NotFound("surah tidak ada (1-114)".into()));
    }
    // ambil limit+1 untuk has_more
    let rows: Vec<(i64, i64, i64, String, Option<String>, i64, Option<i64>, i64, i8, Option<String>)> =
        sqlx::query_as(
            "SELECT a.id, a.surah_id, a.ayah_number, a.text_uthmani, a.text_imlaei, a.juz, a.hizb, a.page, a.sajda, \
             (SELECT t.text FROM quran_translations t WHERE t.ayah_id = a.id AND t.translator_code = ?) \
             FROM quran_ayahs a WHERE a.surah_id = ? AND (? IS NULL OR a.ayah_number > ?) \
             ORDER BY a.ayah_number LIMIT ?")
            .bind(translator).bind(surah_id).bind(cursor).bind(cursor)
            .bind((limit + 1) as i64)
            .fetch_all(pool).await.map_err(dberr)?;
    let has_more = rows.len() > limit;
    let items = rows.into_iter().take(limit).map(|r| AyahOut {
        id: r.0, surah_id: r.1, ayah_number: r.2, text_uthmani: r.3, text_imlaei: r.4,
        juz: r.5, hizb: r.6, page: r.7, sajda: r.8 != 0, translation: r.9,
    }).collect::<Vec<_>>();
    let next_cursor = if has_more { items.last().map(|a| a.ayah_number.to_string()) } else { None };
    Ok(CursorPage { items, next_cursor, has_more })
}

pub async fn audio(pool: &MySqlPool, ayah_id: i64, reciter: Option<String>) -> Result<Vec<AudioOut>, AppError> {
    let rows: Vec<(String, String, Option<i32>)> = sqlx::query_as(
        "SELECT reciter_code, audio_url, duration_ms FROM quran_audio_files WHERE ayah_id = ? \
         AND (? IS NULL OR reciter_code = ?) ORDER BY reciter_code")
        .bind(ayah_id).bind(reciter.as_deref()).bind(reciter.as_deref())
        .fetch_all(pool).await.map_err(dberr)?;
    if rows.is_empty() {
        return Err(AppError::NotFound("audio tidak ada utk ayah/reciter ini".into()));
    }
    Ok(rows.into_iter().map(|r| AudioOut {
        ayah_id, reciter_code: r.0, audio_url: r.1, duration_ms: r.2,
    }).collect())
}

pub async fn get_last_read(pool: &MySqlPool, user_id: i64) -> Result<LastReadOut, AppError> {
    let row: Option<(i64, i64, String, i64, i64, i64)> = sqlx::query_as(
        "SELECT qa.id, qa.surah_id, qs.name_id, qa.ayah_number, qa.page, qa.juz \
         FROM user_reading_progress urp JOIN quran_ayahs qa ON qa.id = urp.last_ayah_id \
         JOIN quran_surahs qs ON qs.id = qa.surah_id WHERE urp.user_id = ?")
        .bind(user_id).fetch_optional(pool).await.map_err(dberr)?;
    row.map(|r| LastReadOut {
        ayah_id: r.0, surah_id: r.1, surah_name: r.2, ayah_number: r.3, page: r.4, juz: r.5,
    }).ok_or_else(|| AppError::NotFound("belum ada riwayat baca".into()))
}

pub async fn put_last_read(pool: &MySqlPool, user_id: i64, ayah_id: i64) -> Result<(), AppError> {
    let exists: Option<(i64, i64)> = sqlx::query_as("SELECT id, surah_id FROM quran_ayahs WHERE id = ?")
        .bind(ayah_id).fetch_optional(pool).await.map_err(dberr)?;
    let (_, surah_id) = exists.ok_or_else(|| AppError::Unprocessable("ayah_id tidak valid".into()))?;
    sqlx::query(
        "INSERT INTO user_reading_progress (user_id, last_ayah_id, last_page) \
         SELECT ?, id, page FROM quran_ayahs WHERE id = ? \
         ON DUPLICATE KEY UPDATE last_ayah_id = VALUES(last_ayah_id), last_page = VALUES(last_page), updated_at = CURRENT_TIMESTAMP")
        .bind(user_id).bind(ayah_id)
        .execute(pool).await.map_err(dberr)?;
    // Learning journey (keputusan #6) — client debounce; server catat ringan
    sqlx::query(
        "INSERT INTO activity_events (user_id, event_type, ref_type, ref_id, payload) \
         VALUES (?, 'reading.last_read', 'quran_ayah', ?, CAST(? AS JSON))")
        .bind(user_id).bind(ayah_id.to_string())
        .bind(format!("{{\"surah_id\":{surah_id},\"ayah_id\":{ayah_id}}}"))
        .execute(pool).await.map_err(dberr)?;
    Ok(())
}

pub async fn add_bookmark(pool: &MySqlPool, user_id: i64, ayah_id: i64, note: Option<String>) -> Result<BookmarkOut, AppError> {
    let exists: Option<(i64, i64, i64, i64)> = sqlx::query_as(
        "SELECT id, surah_id, ayah_number, page FROM quran_ayahs WHERE id = ?")
        .bind(ayah_id).fetch_optional(pool).await.map_err(dberr)?;
    let (_, surah_id, ayah_number, page) =
        exists.ok_or_else(|| AppError::Unprocessable("ayah_id tidak valid".into()))?;
    sqlx::query(
        "INSERT INTO bookmarks (user_id, ayah_id, note) VALUES (?, ?, ?) AS new \
         ON DUPLICATE KEY UPDATE note = new.note")
        .bind(user_id).bind(ayah_id).bind(&note)
        .execute(pool).await.map_err(dberr)?;
    let id: (i64,) = sqlx::query_as("SELECT id FROM bookmarks WHERE user_id = ? AND ayah_id = ?")
        .bind(user_id).bind(ayah_id).fetch_one(pool).await.map_err(dberr)?;
    Ok(BookmarkOut { id: id.0, ayah_id, surah_id, ayah_number, page, note })
}

pub async fn list_bookmarks(
    pool: &MySqlPool, user_id: i64, cursor: Option<i64>, limit: usize,
) -> Result<CursorPage<BookmarkOut>, AppError> {
    let rows: Vec<(i64, i64, i64, i64, i64, Option<String>)> = sqlx::query_as(
        "SELECT b.id, b.ayah_id, a.surah_id, a.ayah_number, a.page, b.note \
         FROM bookmarks b JOIN quran_ayahs a ON a.id = b.ayah_id \
         WHERE b.user_id = ? AND (? IS NULL OR b.id < ?) ORDER BY b.id DESC LIMIT ?")
        .bind(user_id).bind(cursor).bind(cursor).bind((limit + 1) as i64)
        .fetch_all(pool).await.map_err(dberr)?;
    let has_more = rows.len() > limit;
    let items = rows.into_iter().take(limit).map(|r| BookmarkOut {
        id: r.0, ayah_id: r.1, surah_id: r.2, ayah_number: r.3, page: r.4, note: r.5,
    }).collect::<Vec<_>>();
    let next_cursor = if has_more { items.last().map(|b| b.id.to_string()) } else { None };
    Ok(CursorPage { items, next_cursor, has_more })
}

pub async fn delete_bookmark(pool: &MySqlPool, user_id: i64, ayah_id: i64) -> Result<(), AppError> {
    let n = sqlx::query("DELETE FROM bookmarks WHERE user_id = ? AND ayah_id = ?")
        .bind(user_id).bind(ayah_id)
        .execute(pool).await.map_err(dberr)?.rows_affected();
    if n == 0 {
        return Err(AppError::NotFound("bookmark tidak ada".into()));
    }
    Ok(())
}
