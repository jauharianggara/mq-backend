//! DTO modul memorization.
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct SubmitReq {
    pub surah_id: i64,
    pub ayah_start: i64,
    pub ayah_end: i64,
    pub audio_media_id: i64,
    pub note: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SubmissionOut {
    pub id: i64,
    pub user_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_name: Option<String>,
    pub surah_id: i64,
    pub surah_name: String,
    pub ayah_start: i64,
    pub ayah_end: i64,
    pub audio_media_id: i64,
    pub duration_ms: Option<i32>,
    pub note: Option<String>,
    pub status: String,
    pub ustadz_id: Option<i64>,
    pub submitted_at: String,
    pub reviewed_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SubmissionDetail {
    #[serde(flatten)]
    pub submission: SubmissionOut,
    pub audio_presigned_url: Option<String>,
    pub review: Option<ReviewOut>,
}

#[derive(Debug, Serialize)]
pub struct ReviewOut {
    pub reviewer_id: i64,
    pub verdict: String,
    pub notes: Option<String>,
    pub reply_audio_media_id: Option<i64>,
    pub reply_audio_presigned_url: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct ProgressRow {
    pub surah_id: i64,
    pub surah_name: String,
    pub last_passed_ayah: i64,
    pub passed_count: i64,
}

#[derive(Debug, Deserialize)]
pub struct ReviewReq {
    pub action: String, // CLAIM | SUBMIT
    pub verdict: Option<String>,
    pub notes: Option<String>,
    pub reply_audio_media_id: Option<i64>,
}
