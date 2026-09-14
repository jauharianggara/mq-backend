//! Service modul media — presign 3-langkah; validasi server-side (magic bytes) di complete.
use sqlx::MySqlPool;

use crate::infrastructure::storage::Storage;
use crate::modules::media::dto::*;
use crate::shared::error::AppError;

const PRESIGN_SECS: u64 = 15 * 60; // ≤15 menit (keputusan #3 Bagian III)

fn dberr(e: sqlx::Error) -> AppError {
    tracing::error!("db: {e}");
    AppError::Internal(format!("db: {e}"))
}

fn ext_of(mime: &str) -> &'static str {
    match mime {
        "audio/mp4" | "audio/aac" => "m4a",
        "audio/mpeg" => "mp3",
        "audio/ogg" | "application/ogg" => "ogg",
        "audio/webm" => "weba",
        "image/jpeg" => "jpg",
        "image/png" => "png",
        "image/webp" => "webp",
        "application/pdf" => "pdf",
        _ => "bin",
    }
}

fn mime_allowed(kind: &str, mime: &str) -> bool {
    match kind {
        "AUDIO" => matches!(mime, "audio/mp4" | "audio/aac" | "audio/mpeg" | "audio/ogg" | "application/ogg" | "audio/webm"),
        "IMAGE" => matches!(mime, "image/jpeg" | "image/png" | "image/webp"),
        "DOCUMENT" => mime == "application/pdf",
        _ => false,
    }
}

async fn max_bytes(pool: &MySqlPool, kind: &str) -> i64 {
    if kind == "AUDIO" {
        let mb: Option<(Option<String>,)> = sqlx::query_as(
            "SELECT value FROM settings WHERE `key` = 'voice_note_max_mb'")
            .fetch_optional(pool).await.ok().flatten();
        let mb: i64 = mb.and_then(|(v,)| v)
            .and_then(|v| v.trim_matches('"').parse().ok())
            .unwrap_or(10);
        mb * 1024 * 1024
    } else {
        20 * 1024 * 1024 // default non-audio 20 MB (settings menyusul)
    }
}

/// Validasi magic bytes 64-byte pertama (keputusan #17 — server-side).
fn magic_ok(kind: &str, b: &[u8]) -> bool {
    match kind {
        "AUDIO" => {
            // m4a/mp4: "ftyp" pada offset 4; ogg: "OggS"; mp3: ID3 atau frame sync FF Ex/Fx; webm: 0x1A45DFA3
            b.len() >= 12 && &b[4..8] == b"ftyp"
                || b.starts_with(b"OggS")
                || b.starts_with(b"ID3")
                || (b.len() >= 2 && b[0] == 0xFF && (b[1] & 0xE0) == 0xE0)
                || (b.len() >= 4 && b[0] == 0x1A && b[1] == 0x45 && b[2] == 0xDF && b[3] == 0xA3)
        }
        "IMAGE" => {
            b.starts_with(&[0xFF, 0xD8])               // jpeg
                || b.starts_with(&[0x89, b'P', b'N', b'G']) // png
                || (b.len() >= 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WEBP")
        }
        "DOCUMENT" => b.starts_with(b"%PDF"),
        _ => false,
    }
}

pub async fn create_upload(
    pool: &MySqlPool,
    storage: &Storage,
    user_id: i64,
    req: CreateUploadReq,
) -> Result<UploadOut, AppError> {
    if !matches!(req.kind.as_str(), "AUDIO" | "IMAGE" | "DOCUMENT") {
        return Err(AppError::Unprocessable("kind harus AUDIO/IMAGE/DOCUMENT (VIDEO menyusul)".into()));
    }
    if !mime_allowed(&req.kind, &req.mime_type) {
        return Err(AppError::Unprocessable(format!("mime {} tidak diizinkan utk kind {}", req.mime_type, req.kind)));
    }
    if req.byte_size <= 0 {
        return Err(AppError::Unprocessable("byte_size harus > 0".into()));
    }
    let max = max_bytes(pool, &req.kind).await;
    if req.byte_size > max {
        return Err(AppError::Unprocessable(format!("ukuran melebihi batas {} bytes", max)));
    }
    if req.kind == "AUDIO" {
        let max_ms = 5 * 60 * 1000;
        match req.duration_ms {
            Some(d) if d <= 0 || d > max_ms =>
                return Err(AppError::Unprocessable("durasi audio maksimal 5 menit".into())),
            None => return Err(AppError::Unprocessable("duration_ms wajib utk AUDIO".into())),
            _ => {}
        }
    }

    let now = chrono::Utc::now();
    let key = format!("{}/{}/{}/{}.{}",
        req.kind.to_lowercase(), now.format("%Y"), now.format("%m"),
        uuid::Uuid::new_v4().simple(), ext_of(&req.mime_type));

    let url = storage.presign_put(&key, &req.mime_type, PRESIGN_SECS).await
        .map_err(|e| { tracing::error!("presign put: {e}"); AppError::Internal("presign gagal".into()) })?;

    let ins = sqlx::query(
        "INSERT INTO media (owner_id, kind, status, bucket, storage_key, mime_type, byte_size, duration_ms) \
         VALUES (?, ?, 'UPLOADING', ?, ?, ?, ?, ?)")
        .bind(user_id).bind(&req.kind).bind(&storage.bucket).bind(&key)
        .bind(&req.mime_type).bind(req.byte_size).bind(req.duration_ms)
        .execute(pool).await.map_err(dberr)?;

    Ok(UploadOut {
        media_id: ins.last_insert_id() as i64,
        upload_url: url,
        expires_at: (now + chrono::Duration::seconds(PRESIGN_SECS as i64)).format("%Y-%m-%dT%H:%M:%SZ").to_string(),
    })
}

pub async fn complete_upload(
    pool: &MySqlPool,
    storage: &Storage,
    user_id: i64,
    media_id: i64,
) -> Result<MediaOut, AppError> {
    let row: (i64, String, String, String, String, i64, Option<i32>) = sqlx::query_as(
        "SELECT id, kind, status, storage_key, mime_type, byte_size, duration_ms FROM media WHERE id = ? AND owner_id = ?")
        .bind(media_id).bind(user_id)
        .fetch_optional(pool).await.map_err(dberr)?
        .ok_or_else(|| AppError::NotFound("media tidak ditemukan (atau bukan milik anda)".into()))?;
    let (id, kind, status, key, mime, size, dur) = row;
    if status != "UPLOADING" {
        return Err(AppError::Conflict(format!("media berstatus {status} — tidak bisa complete")));
    }

    // 1) object ada + size cocok
    let actual = storage.head_size(&key).await
        .map_err(|e| { tracing::error!("{e}"); AppError::Unprocessable("object belum ter-upload (HEAD gagal)".into()) })?;
    match actual {
        Some(s) if s == size => {}
        Some(s) => {
            let _ = sqlx::query("UPDATE media SET status = 'FAILED' WHERE id = ?").bind(id)
                .execute(pool).await;
            return Err(AppError::Unprocessable(format!("ukuran object {s} != dideklarasikan {size}")));
        }
        None => return Err(AppError::Unprocessable("object tidak ditemukan di storage".into())),
    }
    // 2) magic bytes
    let head = storage.first_bytes(&key).await
        .map_err(|e| { tracing::error!("{e}"); AppError::Internal("baca object gagal".into()) })?;
    if !magic_ok(&kind, &head) {
        let _ = sqlx::query("UPDATE media SET status = 'FAILED' WHERE id = ?").bind(id)
            .execute(pool).await;
        return Err(AppError::Unprocessable("isi file tidak cocok dengan tipe yang dideklarasikan (magic bytes)".into()));
    }

    sqlx::query("UPDATE media SET status = 'READY' WHERE id = ?").bind(id)
        .execute(pool).await.map_err(dberr)?;

    let url = storage.presign_get(&key, PRESIGN_SECS).await
        .map_err(|e| { tracing::error!("presign get: {e}"); AppError::Internal("presign gagal".into()) })?;
    Ok(MediaOut {
        media_id: id, kind, status: "READY".into(), mime_type: mime, byte_size: size,
        duration_ms: dur, presigned_url: url,
        expires_at: (chrono::Utc::now() + chrono::Duration::seconds(PRESIGN_SECS as i64)).format("%Y-%m-%dT%H:%M:%SZ").to_string(),
    })
}

/// GET /media/{id} — ACL generik: pemilik ATAU users.read (admin/moderator).
/// ACL per-konteks submission/thread dipegang modul pemakai (7/9) — Bagian III keputusan #7.
pub async fn get_media(
    pool: &MySqlPool,
    storage: &Storage,
    viewer_id: i64,
    viewer_admin: bool,
    media_id: i64,
) -> Result<MediaOut, AppError> {
    let row: Option<(i64, i64, String, String, String, i64, Option<i32>)> = sqlx::query_as(
        "SELECT id, owner_id, kind, status, mime_type, byte_size, duration_ms FROM media WHERE id = ?")
        .bind(media_id).fetch_optional(pool).await.map_err(dberr)?;
    let (id, owner, kind, status, mime, size, dur) = match row {
        Some(r) => r,
        None => return Err(AppError::NotFound("media tidak ditemukan".into())),
    };
    if owner != viewer_id && !viewer_admin {
        return Err(AppError::NotFound("media tidak ditemukan".into())); // 404 anti-guess
    }
    if status != "READY" {
        return Err(AppError::Conflict(format!("media berstatus {status}")));
    }
    let key: (String,) = sqlx::query_as("SELECT storage_key FROM media WHERE id = ?")
        .bind(id).fetch_one(pool).await.map_err(dberr)?;
    let url = storage.presign_get(&key.0, PRESIGN_SECS).await
        .map_err(|e| { tracing::error!("{e}"); AppError::Internal("presign gagal".into()) })?;
    Ok(MediaOut {
        media_id: id, kind, status, mime_type: mime, byte_size: size,
        duration_ms: dur, presigned_url: url,
        expires_at: (chrono::Utc::now() + chrono::Duration::seconds(PRESIGN_SECS as i64)).format("%Y-%m-%dT%H:%M:%SZ").to_string(),
    })
}
