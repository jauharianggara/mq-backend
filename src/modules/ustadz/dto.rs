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

#[derive(Debug, Deserialize)]
pub struct UstadzPointReq {
    pub lat: f64,
    pub lng: f64,
    pub label: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct UstadzPointOut {
    pub lat: Option<f64>,
    pub lng: Option<f64>,
    pub label: Option<String>,
}
