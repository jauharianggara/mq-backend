//! DTO modul khatmil — rev 3.3 (completion v2 POSISI-ONLY).
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct CampaignUpsertReq {
    pub slug: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    /// Cover campaign (media IMAGE READY — upload via modul media).
    #[serde(default)]
    pub cover_media_id: Option<i64>,
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default = "default_target")]
    pub target_khataman: i64,
    /// Jumlah kelompok per campaign (khatmil v2). None = tak diubah (PATCH kompatibel lama); create default 1.
    #[serde(default)]
    pub group_count: Option<i64>,
    #[serde(default)]
    pub period_start: Option<String>,
    #[serde(default)]
    pub period_end: Option<String>,
    #[serde(default = "default_min_minutes")]
    pub min_minutes_per_juz: i64, // legacy (tidak dipakai utk completion v2) — kolom DB tetap
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
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover_url: Option<String>,
    pub max_participants: Option<i64>,
    pub mode: String,
    pub status: String,
    pub target_khataman: i64,
    pub group_count: i64,
    pub min_minutes_per_juz: i64,
    pub require_manual_verification: bool,
    pub participants: i64,
    pub juz_completed: i64,
    pub progress_pct: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub period_start: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub period_end: Option<String>,
    /// Rincian progres per kelompok (utk list multi-kelompok — rev 3);
    /// kosong = campaign 1 kelompok / detail (detail sudah punya groups + juz_map_groups).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups_progress: Vec<GroupProgressLite>,
}

#[derive(Debug, Serialize)]
pub struct GroupProgressLite {
    pub group_no: i64,
    pub member_count: i64,   // assignment aktif (active_marker IS NOT NULL)
    pub completed_juz: i64,  // juz yg assignment TERBARU-nya COMPLETED (pola MAX(id) per juz)
}

#[derive(Debug, Serialize)]
pub struct JuzSlot {
    pub juz: i64,
    pub status: Option<String>,       // None = kosong; assignment TERBARU per juz (aktif / COMPLETED)
    pub owner_name: Option<String>,
    pub pages_read: Option<i64>,      // telemetri legacy
    pub minutes_read: Option<i64>,    // telemetri legacy
    pub current_surah: Option<i64>,   // posisi bacaan pemegang aktif
    pub current_ayah: Option<i64>,
    pub progress_pct: Option<f64>,    // posisi/juz_total_ayat (null bila belum lapor posisi)
    pub completed_at: Option<String>, // dari progress.verified_at (juz COMPLETED)
}

#[derive(Debug, Serialize)]
pub struct CampaignDetail {
    #[serde(flatten)]
    pub campaign: CampaignOut,
    pub juz_map: Vec<JuzSlot>,
    #[serde(default)]
    pub groups: Vec<GroupLite>,
    /// Peta juz PER KELOMPOK (assignment terbaru per juz di kelompok tsb;
    /// assignment legacy group_id NULL dihitung ke kelompok 1).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub juz_map_groups: Vec<GroupJuzMap>,
    pub period_start: Option<String>,
    pub period_end: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct GroupJuzMap {
    pub group_no: i64,
    pub group_id: i64,
    pub slots: Vec<JuzSlot>,
}

#[derive(Debug, Deserialize)]
pub struct ClaimReq {
    #[serde(default)]
    pub juz: Option<i64>,
    #[serde(default)]
    pub group_id: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct AssignmentOut {
    pub id: i64,
    pub campaign_id: i64,
    pub campaign_name: String,
    pub juz: i64,
    pub group_id: i64,
    pub status: String,
    pub due_at: Option<String>,
    pub pages_read: i64,
    pub minutes_read: i64,
    pub verification: String,
    // rev 3.3: posisi bacaan (null = belum pernah lapor posisi)
    pub current_surah: Option<i64>,
    pub current_ayah: Option<i64>,
    pub read_ayat: Option<i64>,
    pub juz_total_ayat: i64,
    pub progress_pct: Option<f64>,
}

/// ProgressReq v2 (rev 3.3 — KEPUTUSAN OPSI B):
/// - current_surah + current_ayah WAJIB (completion = posisi mencapai ayat terakhir juz)
/// - pages_read / minutes_read opsional telemetri legacy (default 0; tidak dipakai utk completion)
#[derive(Debug, Deserialize)]
pub struct ProgressReq {
    #[serde(default)]
    pub pages_read: Option<i64>,
    #[serde(default)]
    pub minutes_read: Option<i64>,
    pub current_surah: Option<i64>,
    pub current_ayah: Option<i64>,
    #[serde(default)]
    pub note: Option<String>,
}

/// Participants v2 (rev 3.2/3.3) — per peserta + metrik posisi/tugas/kontribusi.
#[derive(Debug, Serialize)]
pub struct JuzActiveOut {
    pub juz: i64,
    pub status: String,
    pub current_surah: Option<i64>,
    pub current_ayah: Option<i64>,
    pub read_ayat: Option<i64>,
    pub juz_total_ayat: i64,
    pub progress_pct: Option<f64>,
    pub last_reported_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct JuzDoneOut {
    pub juz: i64,
    pub completed_at: Option<String>, // verified_at
}

#[derive(Debug, Serialize)]
pub struct ParticipantOut {
    pub user_id: i64,
    pub full_name: String,
    pub joined_at: String,
    pub juz_active: Vec<JuzActiveOut>,
    pub juz_done: Vec<JuzDoneOut>,
    pub juz_active_count: i64,
    pub juz_done_count: i64,
    pub minutes_total: i64, // telemetri legacy (bukan metrik ranking)
    pub task_progress_pct: Option<f64>, // done/(done+active); None bila belum pegang juz
    pub contribution_pct: Option<f64>,  // done/(30*target); None bila target 0
    pub last_reported_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ActivityEventOut {
    pub full_name: String,
    pub juz: i64,
    pub current_surah: Option<i64>,
    pub current_ayah: Option<i64>,
    pub note: Option<String>,
    pub completed: bool, // laporan yang posisinya = ayat terakhir juz
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct GroupLite {
    pub id: i64,
    pub group_no: i64,
    pub member_count: i64,
    pub pembina: Option<String>,
    /// Nama ustadz yang menunggu ACC penugasan (badge "Menunggu ACC" di admin).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_pembina: Option<String>,
}
