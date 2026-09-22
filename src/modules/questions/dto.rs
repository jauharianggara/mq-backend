//! DTO modul questions.
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct CategoryOut {
    pub id: i64,
    pub slug: String,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateQuestionReq {
    pub category_id: i64,
    pub title: String,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub is_anonymous: bool,
}

#[derive(Debug, Serialize, Clone)]
pub struct QuestionOut {
    pub id: i64,
    pub category_id: i64,
    pub category_name: String,
    pub is_anonymous: bool,
    /// None bila penanya anonim DAN viewer bukan pemilik/moderator
    pub asker_name: Option<String>,
    pub title: String,
    pub body: Option<String>,
    pub status: String,
    pub assigned_ustadz_name: Option<String>,
    pub created_at: String,
    pub answered_at: Option<String>,
    pub published_at: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct MessageOut {
    pub id: i64,
    pub sender_id: i64,
    pub sender_name: Option<String>,
    pub is_ustadz: bool,
    pub type_: String,
    pub content: Option<String>,
    pub media_id: Option<i64>,
    pub media_presigned_url: Option<String>,
    pub duration_ms: Option<i32>,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct QuestionThread {
    #[serde(flatten)]
    pub question: QuestionOut,
    pub messages: Vec<MessageOut>,
    /// foto penanya — ikut kebijakan anonimitas asker_name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asker_photo_url: Option<String>,
    /// foto ustadz penjawab (wajah publik utk ustadz)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ustadz_photo_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SendMessageReq {
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub media_id: Option<i64>,
    #[serde(default)]
    pub duration_ms: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct RejectReq {
    #[serde(default)]
    pub reason: Option<String>,
}
