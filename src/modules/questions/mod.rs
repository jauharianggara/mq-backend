//! Modul questions (Bagian I Phase 3 #9) — tanya ustadz + moderation machine (v11).
pub mod dto;
pub mod handler;
pub mod service;

use axum::routing::{get, post};
use axum::Router;

use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/question-categories", get(handler::categories))
        .route("/questions", post(handler::create))
        .route("/me/questions", get(handler::my_questions))
        .route("/questions/archive", get(handler::archive))
        .route("/questions/moderation", get(handler::moderation_queue))
        .route("/questions/{id}", get(handler::detail))
        .route("/questions/{id}/messages", post(handler::send_message))
        .route("/questions/{id}/close", post(handler::close))
        .route("/questions/{id}/answer", post(handler::answer))
        .route("/questions/{id}/publish-request", post(handler::publish_request))
        .route("/questions/{id}/publish", post(handler::publish))
        .route("/questions/{id}/reject-publish", post(handler::reject_publish))
        .route("/ustadz/questions/inbox", get(handler::inbox))
}
