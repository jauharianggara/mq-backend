//! Modul quran read (Bagian I Phase 3 #5) — data read-only + progresi baca user.
pub mod dto;
pub mod handler;
pub mod service;

use axum::routing::{get, put};
use axum::Router;

use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/quran/surahs", get(handler::surahs))
        .route("/quran/surahs/{id}/ayahs", get(handler::ayahs))
        .route("/quran/audio", get(handler::audio))
        .route("/me/reading/last-read", get(handler::get_last_read).put(handler::put_last_read))
        .route("/me/bookmarks", get(handler::list_bookmarks).post(handler::add_bookmark))
        .route("/me/bookmarks/{ayah_id}", axum::routing::delete(handler::delete_bookmark))
}
