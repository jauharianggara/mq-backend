//! DTO modul users.
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct PatchMeReq {
    pub full_name: Option<String>,
    pub gender: Option<String>,
    pub birth_date: Option<String>, // YYYY-MM-DD
    pub address_text: Option<String>,
    pub city: Option<String>,
    pub province: Option<String>,
    pub photo_media_id: Option<i64>,
    pub bio: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DeviceReq {
    pub push_token: String,
    pub platform: String,   // ANDROID | IOS | WEB
    pub app_id: String,     // SANTRI_APP | USTADZ_APP | ADMIN_WEB
    pub device_name: Option<String>,
    pub app_version: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DeviceOut {
    pub id: i64,
    pub platform: String,
    pub app_id: String,
    pub device_name: Option<String>,
    pub app_version: Option<String>,
    pub is_active: bool,
    pub last_seen_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ProgressSummary {
    pub ayat_passed: i64,
    pub surah_started: i64,
    pub total_setoran: i64,
    pub setoran_pending: i64,
    pub last_read: Option<LastRead>,
}

#[derive(Debug, Serialize)]
pub struct LastRead {
    pub surah_id: i64,
    pub surah_name: String,
    pub ayah_number: i64,
    pub page: i64,
}
