//! Modul ustadz (Bagian I Phase 3 #4) — profile self-service + stats.
pub mod dto;
pub mod handler;
pub mod service;

use axum::routing::get;
use axum::Router;

use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/me/ustadz/specializations", get(handler::get_specializations).put(handler::put_specializations))
        .route("/me/ustadz/availability", get(handler::get_availability).put(handler::put_availability))
        .route("/ustadz/me/stats", get(handler::stats))
        .route("/me/ustadz/detail", get(handler::get_detail).put(handler::put_detail))
        .route("/me/ustadz/bank-account", get(handler::get_bank).put(handler::put_bank))
}
