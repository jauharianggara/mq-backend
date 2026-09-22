//! DTO modul auth (kontrak: docs/api/openapi.yaml #/paths/auth).
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct RegisterReq {
    pub phone: Option<String>,
    pub email: Option<String>,
    pub password: String,
    pub full_name: Option<String>,
    pub consent: bool,
}

#[derive(Debug, Deserialize)]
pub struct LoginReq {
    pub email: Option<String>,
    pub phone: Option<String>,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct RefreshReq {
    pub refresh_token: String,
}

#[derive(Debug, Serialize)]
pub struct UserPublic {
    pub id: i64,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub account_type: String,
    pub status: String,
    pub roles: Vec<String>,
    pub full_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub photo_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
    pub user: UserPublic,
}

#[derive(Debug, Serialize)]
pub struct RegisterResp {
    pub user_id: i64,
    pub status: String,
    /// Hanya terisi bila MQ_DEV_EXPOSE_TOKENS=true dan pengirim email belum dikonfigurasi (dev).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dev_verification_token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct VerifyEmailReq {
    pub token: String,
}

#[derive(Debug, Deserialize)]
pub struct ForgotReq {
    pub email: Option<String>,
    pub phone: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ResetReq {
    pub token: String,
    pub new_password: String,
}

#[derive(Debug, Deserialize)]
pub struct ChangePasswordReq {
    pub old_password: String,
    pub new_password: String,
}
