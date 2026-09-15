//! Modul visits (Bagian V — Pesan Ustadz: on-demand kunjungan + Xendit + chat + rating dua arah).
pub mod dto;
pub mod handler;
pub mod payments;
pub mod service;

use axum::routing::{get, post, put};
use axum::Router;

use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        // ---- santri ----
        .route("/visits/services", get(handler::list_services))
        .route("/visits/ustadz/nearby", get(handler::nearby))
        .route("/visits/ustadz/{id}/reviews", get(handler::ustadz_public_reviews))
        .route("/visits", get(handler::list_my_visits).post(handler::create_visit))
        .route("/visits/{id}", get(handler::visit_detail))
        .route("/visits/{id}/pay", post(handler::pay))
        .route("/visits/{id}/cancel", post(handler::cancel))
        .route("/visits/{id}/review", get(handler::review_status).post(handler::submit_review))
        // ---- chat (santri & ustadz — peserta visit) ----
        .route("/visits/{id}/messages", get(handler::messages_list).post(handler::messages_send))
        .route("/ustadz/visits/{id}/messages", get(handler::messages_list).post(handler::messages_send))
        // ---- lokasi ----
        .route("/me/location", put(handler::put_my_location))
        // ---- ustadz ----
        .route("/ustadz/visits/settings", get(handler::get_visit_settings).put(handler::put_visit_settings))
        .route("/ustadz/visit/services", get(handler::list_tarif).post(handler::upsert_tarif))
        .route("/ustadz/visit/services/{id}", axum::routing::delete(handler::delete_tarif))
        .route("/ustadz/visits", get(handler::my_visits))
        .route("/ustadz/visits/{id}/requester-reviews", get(handler::requester_reviews))
        .route("/ustadz/visits/{id}/confirm", post(handler::confirm_visit))
        .route("/ustadz/visits/{id}/decline", post(handler::decline_visit))
        .route("/ustadz/visits/{id}/complete", post(handler::complete_visit))
        .route("/ustadz/visits/{id}/review", post(handler::submit_review))
        // ---- payments: webhook publik + dev simulate ----
        .route("/payments/xendit/callback", post(handler::xendit_callback))
        .route("/payments/dev/simulate", post(handler::dev_simulate))
        // ---- admin ----
        .route("/admin/visits", get(handler::admin_list_visits))
        .route("/admin/visits/{id}", get(handler::admin_visit_detail))
        .route("/admin/visits/{id}/force-complete", post(handler::admin_force_complete))
        .route("/admin/visits/{id}/force-cancel", post(handler::admin_force_cancel))
        .route("/admin/visits/{id}/messages", get(handler::admin_messages))
        .route("/admin/payments", get(handler::admin_list_payments))
        .route("/admin/payments/{id}/mark-refunded", post(handler::admin_mark_refunded))
        .route("/admin/reviews", get(handler::admin_list_reviews))
        .route("/admin/reviews/{id}/hide", post(handler::admin_hide_review))
        .route("/admin/reviews/{id}/unhide", post(handler::admin_unhide_review))
}
