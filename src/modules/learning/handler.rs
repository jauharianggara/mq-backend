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
