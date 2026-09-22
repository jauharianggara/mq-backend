//! Handler auth — tipis, semua logika di service.
use axum::extract::State;
use axum::Json;
use serde_json::json;

use crate::middleware::auth::CurrentUser;
use crate::modules::auth::dto::*;
use crate::modules::auth::service as svc;
use crate::shared::error::AppError;
use crate::state::AppState;

fn ok<T: serde::Serialize>(data: T, status: axum::http::StatusCode) -> axum::response::Response {
    (status, Json(json!({ "data": data, "meta": {} }))).into_response()
}

use axum::response::IntoResponse;

pub async fn register(State(st): State<AppState>, Json(req): Json<RegisterReq>) -> Result<axum::response::Response, AppError> {
    let dev = st.dev_expose_tokens();
    let r = svc::register(&st.pool, dev, req).await?;
    Ok(ok(r, axum::http::StatusCode::CREATED))
}

pub async fn login(State(st): State<AppState>, Json(req): Json<LoginReq>) -> Result<axum::response::Response, AppError> {
    let r = svc::login(&st.pool, &st.jwt_secret, req).await?;
    Ok(ok(r, axum::http::StatusCode::OK))
}

pub async fn refresh(State(st): State<AppState>, Json(req): Json<RefreshReq>) -> Result<axum::response::Response, AppError> {
    let r = svc::refresh(&st.pool, &st.jwt_secret, &req.refresh_token).await?;
    Ok(ok(r, axum::http::StatusCode::OK))
}

pub async fn logout(cu: CurrentUser, State(st): State<AppState>) -> Result<axum::response::Response, AppError> {
    svc::logout(&st.pool, cu.session_id).await?;
    Ok(ok(json!({ "logged_out": true }), axum::http::StatusCode::OK))
}

pub async fn logout_all(cu: CurrentUser, State(st): State<AppState>) -> Result<axum::response::Response, AppError> {
    svc::logout_all(&st.pool, cu.user_id).await?;
    Ok(ok(json!({ "logged_out_all": true }), axum::http::StatusCode::OK))
}

pub async fn verify_email(State(st): State<AppState>, Json(req): Json<VerifyEmailReq>) -> Result<axum::response::Response, AppError> {
    svc::verify_email(&st.pool, &req.token).await?;
    Ok(ok(json!({ "verified": true }), axum::http::StatusCode::OK))
}

pub async fn resend_verification(cu: CurrentUser, State(st): State<AppState>) -> Result<axum::response::Response, AppError> {
    // rate-limit resend: 60 detik per user (in-process moka — keputusan #13)
    if !st.resend_gate(&cu.user_id) {
        return Err(AppError::RateLimited);
    }
    let dev = st.dev_expose_tokens();
    let token = svc::resend_verification(&st.pool, dev, cu.user_id).await?;
    Ok(ok(json!({ "sent": true, "dev_verification_token": token }), axum::http::StatusCode::OK))
}

pub async fn forgot_password(State(st): State<AppState>, Json(req): Json<ForgotReq>) -> Result<axum::response::Response, AppError> {
    let dev = st.dev_expose_tokens();
    let token = svc::forgot_password(&st.pool, dev, req).await?;
    Ok(ok(json!({ "sent": true, "dev_reset_token": token }), axum::http::StatusCode::OK))
}

pub async fn reset_password(State(st): State<AppState>, Json(req): Json<ResetReq>) -> Result<axum::response::Response, AppError> {
    svc::reset_password(&st.pool, req).await?;
    Ok(ok(json!({ "reset": true }), axum::http::StatusCode::OK))
}

pub async fn change_password(cu: CurrentUser, State(st): State<AppState>, Json(req): Json<ChangePasswordReq>) -> Result<axum::response::Response, AppError> {
    svc::change_password(&st.pool, cu.user_id, cu.session_id, req).await?;
    Ok(ok(json!({ "changed": true }), axum::http::StatusCode::OK))
}

pub async fn me(cu: CurrentUser, State(st): State<AppState>) -> Result<axum::response::Response, AppError> {
    let u = svc::get_me(&st, cu.user_id).await?;
    Ok(ok(u, axum::http::StatusCode::OK))
}
