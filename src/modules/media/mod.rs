//! Modul media (Bagian I Phase 3 #14) — presign 3-langkah + serve presigned.
pub mod dto;
pub mod handler;
pub mod service;

use axum::routing::{get, post};
use axum::Router;

use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/media/uploads", post(handler::create_upload))
        .route("/media/uploads/{id}/complete", post(handler::complete_upload))
        .route("/media/{id}", get(handler::get_media))
}
