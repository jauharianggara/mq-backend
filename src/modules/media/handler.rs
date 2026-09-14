//! Handler modul media.
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

use crate::middleware::auth::CurrentUser;
use crate::modules::media::dto::*;
use crate::modules::media::service as svc;
use crate::shared::error::AppError;
use crate::state::AppState;

fn ok<T: serde::Serialize>(data: T, status: StatusCode) -> Response {
    (status, Json(json!({ "data": data, "meta": {} }))).into_response()
}

pub async fn create_upload(cu: CurrentUser, State(st): State<AppState>, Json(req): Json<CreateUploadReq>) -> Result<Response, AppError> {
    cu.require("media.upload")?;
    let storage = st.storage.as_ref().ok_or_else(|| AppError::Internal("storage belum dikonfigurasi (S3_*)".into()))?;
    let r = svc::create_upload(&st.pool, storage, cu.user_id, req).await?;
    Ok(ok(r, StatusCode::CREATED))
}

pub async fn complete_upload(cu: CurrentUser, State(st): State<AppState>, Path(id): Path<i64>) -> Result<Response, AppError> {
    cu.require("media.upload")?;
    let storage = st.storage.as_ref().ok_or_else(|| AppError::Internal("storage belum dikonfigurasi".into()))?;
    let r = svc::complete_upload(&st.pool, storage, cu.user_id, id).await?;
    Ok(ok(r, StatusCode::OK))
}

pub async fn get_media(cu: CurrentUser, State(st): State<AppState>, Path(id): Path<i64>) -> Result<Response, AppError> {
    cu.require_active()?;
    let storage = st.storage.as_ref().ok_or_else(|| AppError::Internal("storage belum dikonfigurasi".into()))?;
    let admin = cu.permissions.contains("users.read");
    let r = svc::get_media(&st.pool, storage, cu.user_id, admin, id).await?;
    Ok(ok(r, StatusCode::OK))
}
