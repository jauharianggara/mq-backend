//! Handler modul learning.
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::json;

use crate::middleware::auth::{CurrentUser, OptionalUser};
use crate::modules::learning::dto::*;
use crate::modules::learning::service as svc;
use crate::shared::error::AppError;
use crate::state::AppState;

fn ok<T: serde::Serialize>(data: T, status: StatusCode) -> Response {
    (status, Json(json!({ "data": data, "meta": {} }))).into_response()
}

#[derive(Deserialize)]
pub struct ListQ {
    pub status: Option<String>,
    pub tajwid: Option<String>,
}

pub async fn list_materials(OptionalUser(cu): OptionalUser, State(st): State<AppState>, Query(q): Query<ListQ>) -> Result<Response, AppError> {
    let can_manage = cu.map(|c| c.permissions.contains("learning.manage")).unwrap_or(false);
    if q.status.is_some() && !can_manage {
        return Err(AppError::Forbidden("filter status butuh learning.manage".into()));
    }
    let list = svc::list_materials(&st.pool, q.status, q.tajwid, can_manage).await?;
    Ok(ok(list, StatusCode::OK))
}

pub async fn get_material(OptionalUser(cu): OptionalUser, State(st): State<AppState>, Path(id): Path<i64>) -> Result<Response, AppError> {
    let can_manage = cu.map(|c| c.permissions.contains("learning.manage")).unwrap_or(false);
    let m = svc::get_material(&st.pool, id, can_manage).await?;
    Ok(ok(m, StatusCode::OK))
}

pub async fn put_progress(cu: CurrentUser, State(st): State<AppState>, Path(material_id): Path<i64>, Json(req): Json<PutProgressReq>) -> Result<Response, AppError> {
    cu.require_active()?;
    svc::put_progress(&st.pool, cu.user_id, material_id, req).await?;
    Ok(ok(json!({ "saved": true }), StatusCode::OK))
}

// ==================== ADMIN CRUD (learning.manage) ====================

#[derive(Deserialize)]
pub struct MaterialUpsertReq {
    pub slug: String,
    pub title: String,
    pub tajwid_rule_code: Option<String>,
    pub content_md: String,
    pub cover_media_id: Option<i64>,
    pub status: Option<String>,
}

pub async fn admin_create(cu: CurrentUser, State(st): State<AppState>, Json(req): Json<MaterialUpsertReq>) -> Result<Response, AppError> {
    cu.require("learning.manage")?;
    validate_material(&req)?;
    let dup: Option<(i64,)> = sqlx::query_as("SELECT id FROM learning_materials WHERE slug = ?")
        .bind(&req.slug).fetch_optional(&st.pool).await
        .map_err(|e| { tracing::error!("{e}"); AppError::Internal("db".into()) })?;
    if dup.is_some() {
        return Err(AppError::Conflict("slug sudah dipakai".into()));
    }
    let tajwid_id: Option<i64> = match &req.tajwid_rule_code {
        Some(code) => sqlx::query_as::<_, (i64,)>("SELECT id FROM tajwid_rules WHERE code = ? AND is_active = 1")
            .bind(code).fetch_optional(&st.pool).await
            .map_err(|e| AppError::Internal("db".into()))?
            .map(|r| r.0),
        None => None,
    };
    let ins = sqlx::query(
        "INSERT INTO learning_materials (slug, title, tajwid_rule_id, content_md, cover_media_id, status, created_by) \
         VALUES (?, ?, ?, ?, ?, ?, ?)")
        .bind(&req.slug).bind(&req.title).bind(tajwid_id)
        .bind(&req.content_md).bind(req.cover_media_id)
        .bind(req.status.as_deref().unwrap_or("DRAFT")).bind(cu.user_id)
        .execute(&st.pool).await
        .map_err(|e| { tracing::error!("{e}"); AppError::Internal("db".into()) })?;
    // kalau PUBLISHED, set published_at
    if req.status.as_deref() == Some("PUBLISHED") {
        let _ = sqlx::query("UPDATE learning_materials SET published_at = UTC_TIMESTAMP() WHERE id = ?")
            .bind(ins.last_insert_id()).execute(&st.pool).await;
    }
    let _ = sqlx::query("INSERT INTO audit_logs (actor_id, action, module, entity_type, entity_id) VALUES (?, 'CREATE', 'learning', 'learning_materials', ?)")
        .bind(cu.user_id).bind(ins.last_insert_id()).execute(&st.pool).await;
    let m = svc::get_material(&st.pool, ins.last_insert_id() as i64, true).await?;
    Ok(ok(m, StatusCode::CREATED))
}

pub async fn admin_update(cu: CurrentUser, State(st): State<AppState>, Path(id): Path<i64>, Json(req): Json<MaterialUpsertReq>) -> Result<Response, AppError> {
    cu.require("learning.manage")?;
    let tajwid_id: Option<i64> = match &req.tajwid_rule_code {
        Some(code) => sqlx::query_as::<_, (i64,)>("SELECT id FROM tajwid_rules WHERE code = ? AND is_active = 1")
            .bind(code).fetch_optional(&st.pool).await
            .map_err(|e| AppError::Internal("db".into()))?
            .map(|r| r.0),
        None => None,
    };
    let n = sqlx::query(
        "UPDATE learning_materials SET title = ?, tajwid_rule_id = ?, content_md = ?, cover_media_id = ?, status = ? \
         WHERE id = ?")
        .bind(&req.title).bind(tajwid_id).bind(&req.content_md)
        .bind(req.cover_media_id).bind(req.status.as_deref().unwrap_or("DRAFT")).bind(id)
        .execute(&st.pool).await
        .map_err(|e| { tracing::error!("{e}"); AppError::Internal("db".into()) })?
        .rows_affected();
    if n == 0 { return Err(AppError::NotFound("materi tidak ada".into())); }
    if req.status.as_deref() == Some("PUBLISHED") {
        let _ = sqlx::query("UPDATE learning_materials SET published_at = COALESCE(published_at, UTC_TIMESTAMP()) WHERE id = ?")
            .bind(id).execute(&st.pool).await;
    }
    let _ = sqlx::query("INSERT INTO audit_logs (actor_id, action, module, entity_type, entity_id) VALUES (?, 'UPDATE', 'learning', 'learning_materials', ?)")
        .bind(cu.user_id).bind(id).execute(&st.pool).await;
    let m = svc::get_material(&st.pool, id, true).await?;
    Ok(ok(m, StatusCode::OK))
}

pub async fn admin_delete(cu: CurrentUser, State(st): State<AppState>, Path(id): Path<i64>) -> Result<Response, AppError> {
    cu.require("learning.manage")?;
    let n = sqlx::query("UPDATE learning_materials SET status = 'ARCHIVED' WHERE id = ? AND status != 'ARCHIVED'")
        .bind(id).execute(&st.pool).await
        .map_err(|e| AppError::Internal("db".into()))?
        .rows_affected();
    if n == 0 { return Err(AppError::NotFound("materi tidak ada / sudah archived".into())); }
    let _ = sqlx::query("INSERT INTO audit_logs (actor_id, action, module, entity_type, entity_id) VALUES (?, 'DELETE', 'learning', 'learning_materials', ?)")
        .bind(cu.user_id).bind(id).execute(&st.pool).await;
    Ok(ok(json!({ "archived": true }), StatusCode::OK))
}

fn validate_material(req: &MaterialUpsertReq) -> Result<(), AppError> {
    if req.slug.trim().is_empty() || req.title.trim().is_empty() || req.content_md.trim().is_empty() {
        return Err(AppError::Unprocessable("slug, title, content_md wajib".into()));
    }
    if let Some(s) = req.status.as_deref() {
        if !matches!(s, "DRAFT" | "PUBLISHED" | "ARCHIVED") {
            return Err(AppError::Unprocessable("status DRAFT/PUBLISHED/ARCHIVED".into()));
        }
    }
    Ok(())
}
