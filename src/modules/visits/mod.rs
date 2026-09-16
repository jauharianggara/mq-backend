//! Modul visits v2 (Panggil Ustadz — ketersediaan mingguan, deposit, penarikan).
pub mod dto;
pub mod handler;
pub mod payments;
pub mod service;

use axum::routing::{delete, get, post, put};
use axum::Router;

use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        // ---- santri ----
        .route("/visits/ustadz/nearby", get(handler::nearby))
        .route("/visits/slots", get(handler::slots))
        .route("/visits", get(handler::list_my_visits).post(handler::create_visit))
        .route("/visits/{id}", get(handler::visit_detail))
        .route("/visits/{id}/pay", post(handler::pay))
        .route("/visits/{id}/pay-deposit", post(handler::pay_deposit))
        .route("/visits/{id}/cancel", post(handler::cancel))
        .route("/visits/{id}/review", get(handler::review_status).post(handler::submit_review))
        .route("/visits/{id}/messages", get(handler::messages_list).post(handler::messages_send))
        .route("/visits/ustadz/{id}/reviews", get(handler::ustadz_public_reviews))
        .route("/me/location", put(handler::put_my_location))
        // ---- ustadz ----
        .route("/ustadz/visits/settings", get(handler::get_visit_settings).put(handler::put_visit_settings))
        .route("/ustadz/visit/availability", get(handler::get_availability))
        .route("/ustadz/visit/availability/slots", post(handler::add_slot))
        .route("/ustadz/visit/availability/slots/{id}", delete(handler::delete_slot))
        .route("/ustadz/visit/availability/blackouts", post(handler::add_blackout))
        .route("/ustadz/visit/availability/blackouts/{date}", delete(handler::delete_blackout))
        .route("/ustadz/visits", get(handler::my_visits))
        .route("/ustadz/visits/{id}/confirm", post(handler::confirm_visit))
        .route("/ustadz/visits/{id}/decline", post(handler::decline_visit))
        .route("/ustadz/visits/{id}/complete", post(handler::complete_visit))
        .route("/ustadz/payouts", get(handler::my_payouts).post(handler::request_payout))
        // ---- pembayaran (webhook publik + simulasi dev) ----
        .route("/payments/xendit/callback", post(handler::xendit_callback))
        .route("/payments/xendit/simulate", post(handler::xendit_simulate))
        // ---- admin ----
        .route("/admin/visits", get(handler::admin_list_visits))
        .route("/admin/visits/{id}", get(handler::admin_visit_detail))
        .route("/admin/visits/{id}/force-complete", post(handler::admin_force_complete))
        .route("/admin/visits/{id}/force-cancel", post(handler::admin_force_cancel))
        .route("/admin/visits/{id}/messages", get(handler::admin_messages))
        .route("/admin/payouts", get(handler::admin_list_payouts))
        .route("/admin/payouts/{id}/approve", post(handler::admin_approve_payout))
        .route("/admin/payouts/{id}/reject", post(handler::admin_reject_payout))
        .route("/admin/payouts/mark-transferred", post(handler::admin_mark_transferred))
}
