//! DTO modul khatmil.
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct CampaignUpsertReq {
    pub slug: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default = "default_target")]
    pub target_khataman: i64,
    #[serde(default)]
    pub period_start: Option<String>,
    #[serde(default)]
    pub period_end: Option<String>,
    #[serde(default = "default_min_minutes")]
    pub min_minutes_per_juz: i64,
    #[serde(default)]
    pub require_manual_verification: bool,
    #[serde(default)]
    pub max_participants: Option<i64>,
    #[serde(default = "default_status")]
    pub status: String,
}
fn default_mode() -> String { "PARALLEL".into() }
fn default_target() -> i64 { 1 }
fn default_min_minutes() -> i64 { 30 }
fn default_status() -> String { "DRAFT".into() }

#[derive(Debug, Serialize)]
pub struct CampaignOut {
    pub id: i64,
    pub slug: String,
    pub name: String,
    pub mode: String,
    pub status: String,
    pub target_khataman: i64,
    pub min_minutes_per_juz: i64,
    pub require_manual_verification: bool,
    pub participants: i64,
    pub juz_completed: i64,
    pub progress_pct: f64,
}

#[derive(Debug, Serialize)]
pub struct JuzSlot {
    pub juz: i64,
    pub status: Option<String>,       // None = kosong
    pub owner_name: Option<String>,   // None utk kosong / anonim? khatmil tidak anonim
    pub pages_read: Option<i64>,
    pub minutes_read: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct CampaignDetail {
    #[serde(flatten)]
    pub campaign: CampaignOut,
    pub juz_map: Vec<JuzSlot>,
    pub period_start: Option<String>,
    pub period_end: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ClaimReq {
    #[serde(default)]
    pub juz: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct AssignmentOut {
    pub id: i64,
    pub campaign_id: i64,
    pub campaign_name: String,
    pub juz: i64,
    pub status: String,
    pub due_at: Option<String>,
    pub pages_read: i64,
    pub minutes_read: i64,
    pub verification: String,
}

#[derive(Debug, Deserialize)]
pub struct ProgressReq {
    pub pages_read: i64,
    pub minutes_read: i64,
    #[serde(default)]
    pub note: Option<String>,
}
