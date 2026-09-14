//! Service CMS — publik read + admin CRUD (handler di sini, service query murni).
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
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

// ---------- PUBlik ----------

pub async fn pub_banners_h(State(st): State<AppState>) -> Result<Response, AppError> {
    let rows: Vec<(i64, String, Option<String>, Option<String>, i64)> = sqlx::query_as(
        "SELECT b.id, b.title, tt.target_value, m.mime_type, b.sort_order FROM banners b \
         LEFT JOIN (SELECT 1 AS x) tt ON TRUE LEFT JOIN media m ON m.id = b.image_media_id \
         WHERE b.is_active = 1 AND (b.starts_at IS NULL OR b.starts_at <= UTC_TIMESTAMP()) \
         AND (b.ends_at IS NULL OR b.ends_at >= UTC_TIMESTAMP()) ORDER BY b.sort_order, b.id")
        .fetch_all(&st.pool).await.map_err(dberr)?;
    Ok(ok(rows.iter().map(|r| json!({ "id": r.0, "title": r.1, "target": r.2, "mime": r.3, "sort": r.4 })).collect::<Vec<_>>(), StatusCode::OK))
}

pub async fn pub_articles_h(State(st): State<AppState>, Query(q): Query<std::collections::HashMap<String, String>>) -> Result<Response, AppError> {
    let limit: i64 = q.get("limit").and_then(|v| v.parse().ok()).unwrap_or(20).clamp(1, 50);
    let cursor: Option<i64> = q.get("cursor").and_then(|v| v.parse().ok());
    let rows: Vec<(i64, String, String, Option<String>, Option<i64>, Option<String>)> = sqlx::query_as(
        "SELECT a.id, a.slug, a.title, a.excerpt, a.reading_minutes, DATE_FORMAT(a.published_at, '%Y-%m-%dT%H:%i:%sZ') \
         FROM articles a WHERE a.status = 'PUBLISHED' AND (? IS NULL OR a.id < ?) ORDER BY a.published_at DESC, a.id DESC LIMIT ?")
        .bind(cursor).bind(cursor).bind(limit + 1)
        .fetch_all(&st.pool).await.map_err(dberr)?;
    let items: Vec<_> = rows.iter().take(limit as usize).map(|r| json!({
        "id": r.0, "slug": r.1, "title": r.2, "excerpt": r.3, "reading_minutes": r.4, "published_at": r.5
    })).collect();
    Ok(ok(items, StatusCode::OK))
}

pub async fn pub_article_h(State(st): State<AppState>, Path(slug): Path<String>) -> Result<Response, AppError> {
    let row: Option<(i64, String, String, Option<String>, String, Option<i64>, Option<String>)> = sqlx::query_as(
        "SELECT id, slug, title, excerpt, content_html, reading_minutes, DATE_FORMAT(published_at, '%Y-%m-%dT%H:%i:%sZ') \
         FROM articles WHERE slug = ? AND status = 'PUBLISHED'")
        .bind(&slug).fetch_optional(&st.pool).await.map_err(dberr)?;
    match row {
        Some(r) => Ok(ok(json!({
            "id": r.0, "slug": r.1, "title": r.2, "excerpt": r.3,
            "content_html": r.4, "reading_minutes": r.5, "published_at": r.6
        }), StatusCode::OK)),
        None => Err(AppError::NotFound("artikel tidak ditemukan".into())),
    }
}

pub async fn pub_announcements_h(State(st): State<AppState>) -> Result<Response, AppError> {
    let rows: Vec<(i64, String, String, String, Option<String>)> = sqlx::query_as(
        "SELECT id, title, body, level, DATE_FORMAT(created_at, '%Y-%m-%dT%H:%i:%sZ') FROM announcements \
         WHERE is_active = 1 AND (starts_at IS NULL OR starts_at <= UTC_TIMESTAMP()) \
         AND (ends_at IS NULL OR ends_at >= UTC_TIMESTAMP()) ORDER BY id DESC LIMIT 50")
        .fetch_all(&st.pool).await.map_err(dberr)?;
    Ok(ok(rows.iter().map(|r| json!({ "id": r.0, "title": r.1, "body": r.2, "level": r.3, "created_at": r.4 })).collect::<Vec<_>>(), StatusCode::OK))
}

pub async fn pub_faqs_h(State(st): State<AppState>) -> Result<Response, AppError> {
    let rows: Vec<(i64, String, String)> = sqlx::query_as(
        "SELECT id, question, answer FROM faqs WHERE is_active = 1 ORDER BY sort_order, id")
        .fetch_all(&st.pool).await.map_err(dberr)?;
    Ok(ok(rows.iter().map(|r| json!({ "id": r.0, "question": r.1, "answer": r.2 })).collect::<Vec<_>>(), StatusCode::OK))
}

// ---------- ADMIN CRUD (cms.manage) — entity table-driven dgn whitelist kolom ----------

const ENTITIES: &[(&str, &[&str], &[&str])] = &[
    // (tabel, kolom insertable, kolom updateable)
    ("articles", &["slug", "category_id", "title", "excerpt", "content_html", "cover_media_id", "author_id", "status", "reading_minutes"], &["title", "excerpt", "content_html", "status", "reading_minutes", "category_id"]),
    ("article_categories", &["slug", "name"], &["name"]),
    ("banners", &["title", "image_media_id", "position", "target_type", "target_value", "sort_order", "starts_at", "ends_at"], &["title", "position", "target_type", "target_value", "sort_order", "starts_at", "ends_at", "is_active"]),
    ("announcements", &["title", "body", "level", "starts_at", "ends_at"], &["title", "body", "level", "starts_at", "ends_at", "is_active"]),
    ("faqs", &["question", "answer", "sort_order"], &["question", "answer", "sort_order", "is_active"]),
];

fn find_entity(name: &str) -> Option<(&'static str, &'static [&'static str], &'static [&'static str])> {
    ENTITIES.iter().copied().find(|(t, _, _)| *t == name)
}

fn audit(st: &AppState, actor: i64, action: &'static str, entity_type: &'static str, entity_id: String, new_value: Option<String>) {
    let pool = st.pool.clone();
    let nv = new_value.map(|v| v.to_string());
    tokio::spawn(async move {
        let _ = sqlx::query("INSERT INTO audit_logs (actor_id, action, module, entity_type, entity_id, new_value) \
                             VALUES (?, ?, 'cms', ?, ?, CAST(? AS JSON))")
            .bind(actor).bind(action).bind(entity_type).bind(entity_id)
            .bind(nv.as_deref().unwrap_or("{}"))
            .execute(&pool).await;
    });
}

pub async fn admin_create(cu: CurrentUser, State(st): State<AppState>, Path(entity): Path<String>, Json(body): Json<serde_json::Value>) -> Result<Response, AppError> {
    cu.require("cms.manage")?;
    let Some((table, cols, _)) = find_entity(&entity) else {
        return Err(AppError::NotFound("entity tidak dikenal".into()));
    };
    let mut names = Vec::new();
    let mut marks = Vec::new();
    let mut binds: Vec<String> = Vec::new();
    for c in cols {
        if let Some(v) = body.get(*c).and_then(|v| match v {
            serde_json::Value::Null => None,
            v => Some(v.to_string().trim_matches('"').to_string()),
        }) {
            names.push(format!("`{c}`"));
            marks.push("?".to_string());
            binds.push(v);
        }
    }
    if names.is_empty() {
        return Err(AppError::Unprocessable("minimal satu kolom valid".into()));
    }
    if table == "articles" && body.get("status").and_then(|s| s.as_str()) == Some("PUBLISHED") {
        names.push("`published_at`".into());
        marks.push("UTC_TIMESTAMP()".into());
    }
    let sql = format!("INSERT INTO {table} ({}) VALUES ({})", names.join(", "), marks.join(", "));
    let mut q = sqlx::query(&sql);
    for b in &binds { q = q.bind(b); }
    let r = q.execute(&st.pool).await.map_err(dberr)?;
    let id = r.last_insert_id();
    audit(&st, cu.user_id, "CREATE", table, id.to_string(), Some(body.to_string()));
    Ok(ok(json!({ "id": id }), StatusCode::CREATED))
}

pub async fn admin_update(cu: CurrentUser, State(st): State<AppState>, Path((entity, id)): Path<(String, i64)>, Json(body): Json<serde_json::Value>) -> Result<Response, AppError> {
    cu.require("cms.manage")?;
    let Some((table, _, cols)) = find_entity(&entity) else {
        return Err(AppError::NotFound("entity tidak dikenal".into()));
    };
    let mut sets = Vec::new();
    let mut binds: Vec<String> = Vec::new();
    for c in cols {
        if let Some(v) = body.get(*c) {
            if v.is_null() { continue; }
            sets.push(format!("`{c}` = ?"));
            binds.push(v.to_string().trim_matches('"').to_string());
        }
    }
    if sets.is_empty() {
        return Err(AppError::Unprocessable("tidak ada kolom untuk diupdate".into()));
    }
    let sql = format!("UPDATE {table} SET {} WHERE id = ?", sets.join(", "));
    let mut q = sqlx::query(&sql);
    for b in &binds { q = q.bind(b); }
    q = q.bind(id);
    let n = q.execute(&st.pool).await.map_err(dberr)?.rows_affected();
    if n == 0 { return Err(AppError::NotFound("row tidak ada".into())); }
    audit(&st, cu.user_id, "UPDATE", table, id.to_string(), Some(body.to_string()));
    Ok(ok(json!({ "updated": true }), StatusCode::OK))
}

pub async fn admin_delete(cu: CurrentUser, State(st): State<AppState>, Path((entity, id)): Path<(String, i64)>) -> Result<Response, AppError> {
    cu.require("cms.manage")?;
    let Some((table, _, _)) = find_entity(&entity) else {
        return Err(AppError::NotFound("entity tidak dikenal".into()));
    };
    let n = sqlx::query(&format!("DELETE FROM {table} WHERE id = ?")).bind(id)
        .execute(&st.pool).await.map_err(dberr)?.rows_affected();
    if n == 0 { return Err(AppError::NotFound("row tidak ada".into())); }
    audit(&st, cu.user_id, "DELETE", table, id.to_string(), None);
    Ok(ok(json!({ "deleted": true }), StatusCode::OK))
}

pub fn admin_routes() -> axum::Router<AppState> {
    use axum::routing::{delete, patch, post};
    axum::Router::new()
        .route("/admin/cms/{entity}", post(admin_create))
        .route("/admin/cms/{entity}/{id}", patch(admin_update).delete(admin_delete))
}
