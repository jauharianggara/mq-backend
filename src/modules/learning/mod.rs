//! Modul learning (Bagian I Phase 3 #6) — materi + progress via activity_events.
pub mod dto;
pub mod handler;
pub mod service;

use axum::routing::{get, put};
use axum::Router;

use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/learning/materials", get(handler::list_materials))
        .route("/learning/materials/{id}", get(handler::get_material))
        .route("/me/learning/{material_id}/progress", put(handler::put_progress))
}
