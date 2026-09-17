//! Handler modul ustadz.
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

use crate::middleware::auth::CurrentUser;
use crate::modules::ustadz::dto::*;
use crate::modules::ustadz::service as svc;
use crate::shared::error::AppError;
use crate::state::AppState;

fn ok<T: serde::Serialize>(data: T, status: StatusCode) -> Response {
    (status, Json(json!({ "data": data, "meta": {} }))).into_response()
}

pub async fn get_specializations(cu: CurrentUser, State(st): State<AppState>) -> Result<Response, AppError> {
    cu.require("ustadz.profile.self")?;
    Ok(ok(svc::get_specializations(&st.pool, cu.user_id).await?, StatusCode::OK))
}

pub async fn put_specializations(cu: CurrentUser, State(st): State<AppState>, Json(req): Json<PutSpecializationsReq>) -> Result<Response, AppError> {
    cu.require("ustadz.profile.self")?;
    svc::put_specializations(&st.pool, cu.user_id, req).await?;
    Ok(ok(json!({ "updated": true }), StatusCode::OK))
}

pub async fn get_availability(cu: CurrentUser, State(st): State<AppState>) -> Result<Response, AppError> {
    cu.require("ustadz.profile.self")?;
    Ok(ok(svc::get_availability(&st.pool, cu.user_id).await?, StatusCode::OK))
}

pub async fn put_availability(cu: CurrentUser, State(st): State<AppState>, Json(req): Json<PutAvailabilityReq>) -> Result<Response, AppError> {
    cu.require("ustadz.profile.self")?;
    svc::put_availability(&st.pool, cu.user_id, req).await?;
    Ok(ok(json!({ "updated": true }), StatusCode::OK))
}

pub async fn stats(cu: CurrentUser, State(st): State<AppState>) -> Result<Response, AppError> {
    cu.require("ustadz.profile.self")?;
    Ok(ok(svc::stats(&st.pool, cu.user_id).await?, StatusCode::OK))
}

pub async fn get_detail(cu: CurrentUser, State(st): State<AppState>) -> Result<Response, AppError> {
    cu.require("ustadz.profile.self")?;
    Ok(ok(svc::get_detail(&st.pool, cu.user_id).await?, StatusCode::OK))
}

pub async fn put_detail(cu: CurrentUser, State(st): State<AppState>, Json(req): Json<crate::modules::ustadz::dto::UstadzDetailReq>) -> Result<Response, AppError> {
    cu.require("ustadz.profile.self")?;
    svc::put_detail(&st.pool, cu.user_id, req).await?;
    Ok(ok(json!({ "updated": true }), StatusCode::OK))
}

pub async fn get_bank(cu: CurrentUser, State(st): State<AppState>) -> Result<Response, AppError> {
    cu.require("ustadz.profile.self")?;
    Ok(ok(svc::get_bank(&st.pool, cu.user_id).await?, StatusCode::OK))
}

pub async fn put_bank(cu: CurrentUser, State(st): State<AppState>, Json(req): Json<crate::modules::ustadz::dto::BankAccountReq>) -> Result<Response, AppError> {
    cu.require("ustadz.profile.self")?;
    svc::put_bank(&st.pool, cu.user_id, req).await?;
    Ok(ok(json!({ "updated": true }), StatusCode::OK))
}

pub async fn get_point(cu: CurrentUser, State(st): State<AppState>) -> Result<Response, AppError> {
    cu.require("ustadz.profile.self")?;
    Ok(ok(svc::get_point(&st.pool, cu.user_id).await?, StatusCode::OK))
}

pub async fn put_point(cu: CurrentUser, State(st): State<AppState>, Json(req): Json<crate::modules::ustadz::dto::UstadzPointReq>) -> Result<Response, AppError> {
    cu.require("ustadz.profile.self")?;
    svc::put_point(&st.pool, cu.user_id, req).await?;
    Ok(ok(json!({ "updated": true }), StatusCode::OK))
}
