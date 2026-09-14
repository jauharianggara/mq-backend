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
