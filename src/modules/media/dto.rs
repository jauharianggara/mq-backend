//! DTO modul media.
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct CreateUploadReq {
    pub kind: String, // AUDIO | IMAGE | DOCUMENT (VIDEO menyusul)
    pub mime_type: String,
    pub byte_size: i64,
    pub duration_ms: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct UploadOut {
    pub media_id: i64,
    pub upload_url: String,
    pub expires_at: String, // ISO UTC
}

#[derive(Debug, Serialize)]
pub struct MediaOut {
    pub media_id: i64,
    pub kind: String,
    pub status: String,
    pub mime_type: String,
    pub byte_size: i64,
    pub duration_ms: Option<i32>,
    pub presigned_url: String,
    pub expires_at: String,
}
