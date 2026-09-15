//! Modul khatmil (Bagian I Phase 3 #8).
pub mod dto;
pub mod handler;
pub mod service;

use axum::routing::{get, post};
use axum::Router;

use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/khatmil/campaigns", get(handler::list_campaigns).post(handler::create_campaign))
        .route("/khatmil/campaigns/{id}", get(handler::campaign_detail).patch(handler::update_campaign))
        .route("/khatmil/campaigns/{id}/join", post(handler::join))
        .route("/khatmil/campaigns/{id}/juz/claim", post(handler::claim))
        .route("/khatmil/campaigns/{id}/participants", get(handler::participants))
        .route("/khatmil/campaigns/{id}/activity", get(handler::activity))
        .route("/khatmil/assignments/{id}/progress", post(handler::post_progress))
        .route("/me/khatmil/assignments", get(handler::my_assignments))
}
