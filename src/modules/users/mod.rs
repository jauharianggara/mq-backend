//! Modul users (Bagian I Phase 3 #2) — self-service; admin CRUD menyusul (dashboard/admin scope).
pub mod dto;
pub mod handler;
pub mod service;

use axum::routing::{delete, get, post};
use axum::Router;

use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        // NOTE: GET /me sudah terdaftar di modul auth (interim) — tetap satu router nest
        .route("/me", axum::routing::patch(handler::patch_me).delete(handler::delete_me))
        .route("/me/profile", get(handler::my_profile))
        .route("/me/home-point", get(handler::get_home_point).put(handler::put_home_point))
        .route("/me/progress/summary", get(handler::progress_summary))
        .route("/me/devices", get(handler::list_devices).post(handler::register_device))
        .route("/me/devices/{id}", delete(handler::delete_device))
}
