//! DTO modul learning.
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct MaterialOut {
    pub id: i64,
    pub slug: String,
    pub title: String,
    pub tajwid_rule: Option<String>,
    pub content_md: Option<String>,
    pub status: String,
    pub published_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PutProgressReq {
    pub completed: Option<bool>,
    pub last_position: Option<String>,
}
