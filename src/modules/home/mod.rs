//! Modul home (Bagian III M2.5 + M1.2) — agregat Home mobile + force-update.
pub mod handler;

use axum::routing::get;
use axum::Router;

use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/me/home", get(handler::santri_home))
        .route("/ustadz/me/home", get(handler::ustadz_home))
        .route("/app/version", get(handler::app_version)) // TANPA auth (dicek di splash)
}
