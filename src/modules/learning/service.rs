//! Service modul learning.
use sqlx::MySqlPool;

use crate::modules::learning::dto::*;
use crate::shared::error::AppError;

fn dberr(e: sqlx::Error) -> AppError {
    tracing::error!("db: {e}");
    AppError::Internal(format!("db: {e}"))
}

pub async fn list_materials(
    pool: &MySqlPool, status: Option<String>, tajwid: Option<String>, can_manage: bool,
) -> Result<Vec<MaterialOut>, AppError> {
    // default: PUBLISHED utk semua; param status hanya utk learning.manage
    let status = if can_manage { status.unwrap_or_else(|| "PUBLISHED".into()) } else { "PUBLISHED".to_string() };
    let rows: Vec<(i64, String, String, Option<String>, String, Option<String>)> = sqlx::query_as(
        "SELECT lm.id, lm.slug, lm.title, tr.code, lm.status, \
         DATE_FORMAT(lm.published_at, '%Y-%m-%dT%H:%i:%sZ') \
         FROM learning_materials lm LEFT JOIN tajwid_rules tr ON tr.id = lm.tajwid_rule_id \
         WHERE lm.status = ? AND (? IS NULL OR tr.code = ?) ORDER BY lm.published_at DESC, lm.id DESC")
        .bind(&status).bind(tajwid.as_deref()).bind(tajwid.as_deref())
        .fetch_all(pool).await.map_err(dberr)?;
    Ok(rows.into_iter().map(|r| MaterialOut {
        id: r.0, slug: r.1, title: r.2, tajwid_rule: r.3, content_md: None,
        status: r.4, published_at: r.5,
    }).collect())
}

pub async fn get_material(pool: &MySqlPool, id: i64, can_manage: bool) -> Result<MaterialOut, AppError> {
    let row: Option<(i64, String, String, Option<String>, String, Option<String>, String)> = sqlx::query_as(
        "SELECT lm.id, lm.slug, lm.title, tr.code, lm.status, \
         DATE_FORMAT(lm.published_at, '%Y-%m-%dT%H:%i:%sZ'), lm.content_md \
         FROM learning_materials lm LEFT JOIN tajwid_rules tr ON tr.id = lm.tajwid_rule_id WHERE lm.id = ?")
        .bind(id).fetch_optional(pool).await.map_err(dberr)?;
    let (mid, slug, title, rule, status, published, content) = row
        .ok_or_else(|| AppError::NotFound("materi tidak ditemukan".into()))?;
    if status != "PUBLISHED" && !can_manage {
        return Err(AppError::NotFound("materi tidak ditemukan".into()));
    }
    Ok(MaterialOut {
        id: mid, slug, title, tajwid_rule: rule, content_md: Some(content),
        status, published_at: published,
    })
}

/// Progress materi = activity event (learning journey — keputusan #6; tabel khusus tidak diperlukan).
pub async fn put_progress(
    pool: &MySqlPool, user_id: i64, material_id: i64, req: PutProgressReq,
) -> Result<(), AppError> {
    let exists: Option<(i64,)> = sqlx::query_as(
        "SELECT id FROM learning_materials WHERE id = ? AND status = 'PUBLISHED'")
        .bind(material_id).fetch_optional(pool).await.map_err(dberr)?;
    if exists.is_none() {
        return Err(AppError::NotFound("materi tidak ditemukan".into()));
    }
    let completed = req.completed.unwrap_or(false);
    let pos = req.last_position.clone().unwrap_or_default();
    let payload = serde_json::json!({ "completed": completed, "last_position": pos }).to_string();
    sqlx::query(
        "INSERT INTO activity_events (user_id, event_type, ref_type, ref_id, payload) \
         VALUES (?, 'material.read', 'learning_material', ?, CAST(? AS JSON))")
        .bind(user_id).bind(material_id.to_string()).bind(&payload)
        .execute(pool).await.map_err(dberr)?;
    Ok(())
}
