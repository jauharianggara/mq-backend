//! DTO modul visits v2 (Panggil Ustadz — alur baru: ketersediaan + deposit).
use serde::{Deserialize, Serialize};

pub fn ts(d: chrono::NaiveDateTime) -> String {
    d.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}
pub fn ts_o(d: &Option<chrono::NaiveDateTime>) -> Option<String> {
    d.as_ref().map(|x| ts(*x))
}

// ---------- nearby ----------
#[derive(Serialize)]
pub struct NearbyUstadz {
    pub ustadz_id: i64,
    pub full_name: String,
    pub distance_km: f64,
    pub rating_avg: Option<f64>,
    pub rating_count: i64,
    pub price_per_hour: i64,
}

// ---------- slots ----------
#[derive(Deserialize)]
pub struct SlotsReq {
    pub ustadz_id: i64,
    /// YYYY-MM-DD (WIB)
    pub date: String,
    pub hours: i64,
}

#[derive(Serialize)]
pub struct SlotMax {
    pub start: String,
    pub max_hours: i64,
}

#[derive(Serialize)]
pub struct SlotsOut {
    pub date: String,
    pub hours: i64,
    /// jam mulai yang bisa dipilih untuk durasi `hours` (WIB, "HH:MM")
    pub slots: Vec<String>,
    /// jam beruntun maksimal untuk SETIAP jam mulai yang terbuka ≥ 1 jam
    pub max_hours: Vec<SlotMax>,
}

// ---------- booking ----------
#[derive(Deserialize)]
pub struct CreateVisitReq {
    pub ustadz_id: i64,
    /// YYYY-MM-DD (WIB)
    pub date: String,
    /// "HH:MM" (WIB) — wajib dari daftar /visits/slots
    pub start_time: String,
    pub duration_hours: i64,
    /// kosong = pakai titik rumah santri yang tersimpan
    pub lat: Option<f64>,
    pub lng: Option<f64>,
    pub accuracy_m: Option<i16>,
    pub address_label: Option<String>,
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
    pub channel: Option<String>,
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
    pub scheduled_at: String,
    pub duration_hours: i64,
    pub price_per_hour: i64,
    pub price_total: i64,
    pub address_label: String,
    pub note: Option<String>,
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
    pub replay: bool,
}

// ---------- settings ustadz ----------
#[derive(Serialize)]
pub struct VisitSettingsOut {
    pub is_accepting: bool,
    pub max_active_visits: i64,
    pub price_per_hour: i64,
}

#[derive(Deserialize)]
pub struct VisitSettingsReq {
    pub is_accepting: bool,
    pub max_active_visits: i64,
    pub price_per_hour: i64,
}

// ---------- slots ketersediaan ----------
#[derive(Deserialize)]
pub struct SlotUpsertReq {
    pub weekday: i8, // 0=Minggu .. 6=Sabtu
    pub start_minute: i16,
    pub end_minute: i16,
}

#[derive(Serialize)]
pub struct SlotOut {
    pub id: i64,
    pub weekday: i8,
    pub start_minute: i64,
    pub end_minute: i64,
}

#[derive(Deserialize)]
pub struct BlackoutReq {
    pub off_date: String, // YYYY-MM-DD
    pub note: Option<String>,
}

#[derive(Serialize)]
pub struct BlackoutOut {
    pub off_date: String,
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

// ---------- ustadz permintaan masuk ----------
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
    pub scheduled_at: String,
    pub duration_hours: i64,
    pub price_total: i64,
    pub note: Option<String>,
    pub address_label: String,
    pub requester: RequesterOut,
}

#[derive(Serialize)]
pub struct MyVisitsOut {
    pub incoming: Vec<IncomingVisitOut>,
    pub upcoming: Vec<VisitOut>,
}

// ---------- wallet ----------
#[derive(Serialize)]
pub struct WalletOut {
    pub balance: i64,
}

#[derive(Deserialize)]
pub struct TopupReq {
    pub amount: i64,
}

#[derive(Serialize)]
pub struct WalletTxOut {
    pub id: i64,
    pub tx_type: String,
    pub amount: i64,
    pub balance_after: i64,
    pub subject_type: Option<String>,
    pub subject_id: Option<i64>,
    pub created_at: String,
}

// ---------- payout ----------
#[derive(Deserialize)]
pub struct PayoutCreateReq {
    pub bank_name: String,
    pub account_no: String,
    pub account_name: String,
    pub amount: i64,
}

#[derive(Serialize)]
pub struct PayoutOut {
    pub id: i64,
    pub amount: i64,
    pub fee: i64,
    pub bank_name: String,
    pub bank_account_no: String,
    pub bank_account_name: String,
    pub status: String,
    pub rejected_reason: Option<String>,
    pub created_at: String,
    pub processed_at: Option<String>,
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
    #[serde(default)]
    pub force_refund: bool,
}
