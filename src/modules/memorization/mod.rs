//! Modul memorization (Bagian I Phase 3 #7) — setoran hafalan & review.
pub mod dto;
pub mod handler;
pub mod service;

use axum::routing::{get, post};
use axum::Router;

use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/memorization/submissions", post(handler::submit))
        .route("/me/submissions", get(handler::my_submissions))
        .route("/me/submissions/{id}", get(handler::my_submission_detail))
        .route("/me/memorization/progress", get(handler::my_progress))
        .route("/ustadz/memorization/queue", get(handler::queue))
        .route("/memorization/submissions/{id}/review", post(handler::review))
}
