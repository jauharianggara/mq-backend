//! DTO modul ustadz.
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct SpecializationOut {
    pub category_id: i64,
    pub slug: String,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct PutSpecializationsReq {
    pub category_ids: Vec<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AvailabilityOut {
    pub is_accepting_questions: bool,
    pub max_active_questions: i64,
    pub verified: bool,
}

#[derive(Debug, Deserialize)]
pub struct PutAvailabilityReq {
    pub is_accepting_questions: bool,
    pub max_active_questions: i64,
}

#[derive(Debug, Serialize)]
pub struct UstadzStats {
    pub total_reviewed: i64,
    pub queue_pending_global: i64,
    pub my_in_review: i64,
    pub avg_review_minutes: Option<f64>,
    pub questions_answered: i64,
    pub questions_pending: i64,
}

#[derive(Debug, Deserialize)]
pub struct UstadzDetailReq {
    pub pendidikan_terakhir: Option<String>,
    pub pengalaman_mengajar: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct UstadzDetailOut {
    pub pendidikan_terakhir: Option<String>,
    pub pengalaman_mengajar: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BankAccountReq {
    pub bank_name: String,
    pub bank_account_no: String,
    pub bank_account_name: String,
}

#[derive(Debug, Serialize)]
pub struct BankAccountOut {
    pub bank_name: Option<String>,
    pub bank_account_no: Option<String>,
    pub bank_account_name: Option<String>,
}
