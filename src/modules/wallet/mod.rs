//! Modul wallet (deposit santri & penghasilan ustadz).
pub mod admin_service;
pub mod handler;
pub mod service;

use axum::routing::{get, post};
use axum::Router;

use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/wallet", get(handler::get_wallet))
        .route("/wallet/topup", post(handler::topup))
        .route("/wallet/transactions", get(handler::transactions))
        .route("/me/wallet-adjustments", get(handler::my_adjustments))
        .route("/me/wallet-adjustments/{id}/accept", post(handler::accept_adjustment))
        .route("/me/wallet-adjustments/{id}/reject", post(handler::reject_adjustment))
        // admin
        .route("/admin/wallet", get(handler::admin_list_balances))
        .route("/admin/wallet/{user_id}/transactions", get(handler::admin_transactions))
        .route("/admin/wallet/{user_id}/adjustment", post(handler::admin_propose_adjustment))
        .route("/admin/wallet-adjustments", get(handler::admin_list_adjustments))
        .route("/admin/payments", get(handler::admin_list_payments))
}
