//! DTO modul visits (Bagian V — Pesan Ustadz).
use serde::{Deserialize, Serialize};

fn ts(d: chrono::NaiveDateTime) -> String {
    d.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

// ---------- layanan ----------
#[derive(Serialize)]
pub struct ServiceTypeOut {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
}

// ---------- nearby ----------
#[derive(Serialize, Clone)]
pub struct TarifOut {
    pub service_type_id: i64,
    pub service_type_name: String,
    pub price_amount: i64,
    pub duration_minutes: i64,
}

#[derive(Serialize)]
pub struct NearbyUstadz {
    pub ustadz_id: i64,
    pub full_name: String,
    pub distance_km: f64,
    pub rating_avg: Option<f64>,
    pub rating_count: i64,
    pub services: Vec<TarifOut>,
}

// ---------- booking ----------
#[derive(Deserialize)]
pub struct CreateVisitReq {
    pub ustadz_id: i64,
    pub service_type_id: i64,
    /// ISO UTC: 2026-09-20T14:00:00Z
    pub scheduled_at: String,
    pub lat: f64,
    pub lng: f64,
    pub accuracy_m: Option<i16>,
    pub address_label: String,
    pub note: Option<String>,
}

#[derive(Serialize)]
pub struct PaymentOut {
    pub id: i64,
    pub external_id: String,
    pub status: String,
    pub invoice_url: Option<String>,
    pub amount: i64,
    pub refunded_amount: i64,
    pub expires_at: Option<String>,
    pub paid_at: Option<String>,
}

#[derive(Serialize)]
pub struct PartyOut {
    pub user_id: i64,
    pub full_name: String,
    pub phone: Option<String>,
}

#[derive(Serialize)]
pub struct VisitOut {
    pub id: i64,
    pub status: String,
    pub service_type_id: i64,
    pub service_name: String,
    pub scheduled_at: String,
    pub duration_minutes: i64,
    pub address_label: String,
    pub note: Option<String>,
    pub price_amount: i64,
    /// pemilik lokasi (santri) — koordinat hanya utk peserta; dianonymize -> (0,0)
    pub lat: Option<f64>,
    pub lng: Option<f64>,
    pub anonymized: bool,
    pub ustadz: Option<PartyOut>,
    pub requester: Option<PartyOut>,
    pub payment: Option<PaymentOut>,
    pub created_at: String,
    pub paid_at: Option<String>,
    pub confirmed_at: Option<String>,
    pub completed_at: Option<String>,
    pub canceled_at: Option<String>,
    pub cancel_reason: Option<String>,
    pub decline_reason: Option<String>,
}

#[derive(Serialize)]
pub struct VisitCreatedOut {
    pub visit: VisitOut,
    pub invoice_url: Option<String>,
    /// true = idempotency replay (booking lama dikembalikan)
    pub replay: bool,
}

// ---------- settings & tarif ustadz ----------
#[derive(Serialize)]
pub struct VisitSettingsOut {
    pub is_accepting: bool,
    pub max_active_visits: i64,
}

#[derive(Deserialize)]
pub struct VisitSettingsReq {
    pub is_accepting: bool,
    pub max_active_visits: i64,
}

#[derive(Deserialize)]
pub struct TarifUpsertReq {
    pub service_type_id: i64,
    pub price_amount: i64,
    pub duration_minutes: i64,
    pub note: Option<String>,
}

// ---------- chat ----------
#[derive(Deserialize)]
pub struct SendMessageReq {
    pub body: String,
}

#[derive(Serialize)]
pub struct MessageOut {
    pub id: i64,
    pub sender_id: i64,
    pub body: String,
    pub created_at: String,
    pub read_at: Option<String>,
}

#[derive(Serialize)]
pub struct UnreadOut {
    pub unread: i64,
}

// ---------- review ----------
#[derive(Deserialize)]
pub struct ReviewReq {
    pub rating: i8,
    pub comment: Option<String>,
}

#[derive(Serialize)]
pub struct ReviewStatusOut {
    pub can_review: bool,
    pub my_rating: Option<i8>,
    pub my_comment: Option<String>,
    pub counterpart_submitted: bool,
    pub revealed: bool,
}

#[derive(Serialize)]
pub struct ReviewPublicOut {
    pub id: i64,
    pub reviewer_first_name: String,
    pub rating: i8,
    pub comment: Option<String>,
    pub created_at: String,
}

// ---------- lokasi ----------
#[derive(Deserialize)]
pub struct PutLocationReq {
    pub lat: f64,
    pub lng: f64,
    pub accuracy_m: Option<i16>,
}

// ---------- ustadz visits list ----------
#[derive(Serialize)]
pub struct RequesterOut {
    pub user_id: i64,
    pub full_name: String,
    pub rating_avg: Option<f64>,
    pub rating_count: i64,
}

#[derive(Serialize)]
pub struct IncomingVisitOut {
    pub id: i64,
    pub status: String,
    pub service_name: String,
    pub scheduled_at: String,
    pub duration_minutes: i64,
    pub price_amount: i64,
    pub note: Option<String>,
    pub requester: RequesterOut,
}

#[derive(Serialize)]
pub struct MyVisitsOut {
    pub incoming: Vec<IncomingVisitOut>,
    pub upcoming: Vec<VisitOut>,
}

// ---------- admin ----------
#[derive(Deserialize, Default)]
pub struct AdminVisitFilter {
    pub status: Option<String>,
    pub ustadz_id: Option<i64>,
    pub user_id: Option<i64>,
    pub cursor: Option<i64>,
}

#[derive(Deserialize)]
pub struct ForceCancelReq {
    pub reason: String,
    /// true = refund penuh meski di luar aturan (default false ikut aturan)
    #[serde(default)]
    pub force_refund: bool,
}
