//! Modul auth (Bagian I Phase 3 #1).
pub mod dto;
pub mod handler;
pub mod service;
pub mod token;

use axum::routing::{get, post};
use axum::Router;

use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/auth/register", post(handler::register))
        .route("/auth/login", post(handler::login))
        .route("/auth/refresh", post(handler::refresh))
        .route("/auth/logout", post(handler::logout))
        .route("/auth/logout-all", post(handler::logout_all))
        .route("/auth/verify-email", post(handler::verify_email))
        .route("/auth/resend-verification", post(handler::resend_verification))
        .route("/auth/forgot-password", post(handler::forgot_password))
        .route("/auth/reset-password", post(handler::reset_password))
        .route("/auth/change-password", post(handler::change_password))
        // interim: /me (modul 2 users akan memperluas — PATCH/DELETE/summary)
        .route("/me", get(handler::me))
}
