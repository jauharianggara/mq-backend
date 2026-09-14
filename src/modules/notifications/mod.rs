//! Modul notifications (Bagian I Phase 3 #10) — inbox (data ditulis modul 7-9).
pub mod handler;

use axum::routing::{get, post};
use axum::Router;

use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/me/notifications", get(handler::list))
        .route("/me/notifications/{id}/read", post(handler::read_one))
        .route("/me/notifications/read-all", post(handler::read_all))
}
