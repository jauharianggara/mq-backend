//! Handler modul users.
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

use crate::middleware::auth::CurrentUser;
use crate::modules::users::dto::*;
use crate::modules::users::service as svc;
use crate::shared::error::AppError;
use crate::state::AppState;

fn ok<T: serde::Serialize>(data: T, status: StatusCode) -> Response {
    (status, Json(json!({ "data": data, "meta": {} }))).into_response()
}

pub async fn patch_me(cu: CurrentUser, State(st): State<AppState>, Json(req): Json<PatchMeReq>) -> Result<Response, AppError> {
    cu.require_active()?;
    svc::patch_me(&st.pool, cu.user_id, req).await?;
    Ok(ok(json!({ "updated": true }), StatusCode::OK))
}

pub async fn my_profile(cu: CurrentUser, State(st): State<AppState>) -> Result<axum::response::Response, AppError> {
    cu.require_active()?;
    let d = svc::my_profile(&st, cu.user_id).await?;
    Ok(ok(d, StatusCode::OK))
}

pub async fn delete_me(cu: CurrentUser, State(st): State<AppState>) -> Result<Response, AppError> {
    cu.require_active()?;
    // kontrak: confirmation di sisi klien; server langsung anonymize (keputusan #15)
    svc::delete_me(&st.pool, cu.user_id).await?;
    Ok(ok(json!({ "deleted": true }), StatusCode::OK))
}

pub async fn progress_summary(cu: CurrentUser, State(st): State<AppState>) -> Result<Response, AppError> {
    let r = svc::progress_summary(&st.pool, cu.user_id).await?;
    Ok(ok(r, StatusCode::OK))
}

pub async fn register_device(cu: CurrentUser, State(st): State<AppState>, Json(req): Json<DeviceReq>) -> Result<Response, AppError> {
    cu.require_active()?;
    let is_ustadz = cu.permissions.contains("ustadz.profile.self");
    let d = svc::register_device(&st.pool, cu.user_id, req, is_ustadz).await?;
    Ok(ok(d, StatusCode::CREATED))
}

pub async fn list_devices(cu: CurrentUser, State(st): State<AppState>) -> Result<Response, AppError> {
    let d = svc::list_devices(&st.pool, cu.user_id).await?;
    Ok(ok(d, StatusCode::OK))
}

pub async fn delete_device(cu: CurrentUser, State(st): State<AppState>, Path(id): Path<i64>) -> Result<Response, AppError> {
    svc::delete_device(&st.pool, cu.user_id, id).await?;
    Ok(ok(json!({ "deleted": true }), StatusCode::OK))
}

pub async fn get_home_point(cu: CurrentUser, State(st): State<AppState>) -> Result<axum::response::Response, AppError> {
    cu.require_active()?;
    let d = svc::get_home_point(&st.pool, cu.user_id).await?;
    Ok(ok(d, StatusCode::OK))
}

pub async fn put_home_point(cu: CurrentUser, State(st): State<AppState>, Json(req): Json<crate::modules::users::dto::HomePointReq>) -> Result<axum::response::Response, AppError> {
    cu.require_active()?;
    svc::put_home_point(&st.pool, cu.user_id, req).await?;
    Ok(ok(json!({ "updated": true }), StatusCode::OK))
}
