//! Modul CMS (Bagian I Phase 3 #11) — konten publik + admin CRUD.
pub mod service;

use axum::routing::{get, post};
use axum::Router;

use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/cms/banners", get(crate::modules::cms::service::pub_banners_h))
        .route("/cms/articles", get(crate::modules::cms::service::pub_articles_h))
        .route("/cms/articles/{slug}", get(crate::modules::cms::service::pub_article_h))
        .route("/cms/announcements", get(crate::modules::cms::service::pub_announcements_h))
        .route("/cms/faqs", get(crate::modules::cms::service::pub_faqs_h))
}
