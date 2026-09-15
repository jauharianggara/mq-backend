//! DTO modul quran.
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct SurahOut {
    pub id: i64,
    pub name_arabic: String,
    pub name_latin: String,
    pub name_id: String,
    pub ayah_count: i64,
    pub revelation: String,
}

#[derive(Debug, Serialize)]
pub struct AyahOut {
    pub id: i64,
    pub surah_id: i64,
    pub ayah_number: i64,
    pub text_uthmani: String,
    pub text_imlaei: Option<String>,
    pub juz: i64,
    pub hizb: Option<i64>,
    pub page: i64,
    pub sajda: bool,
    pub translation: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AudioOut {
    pub ayah_id: i64,
    pub reciter_code: String,
    pub audio_url: String,
    pub duration_ms: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct LastReadOut {
    pub ayah_id: i64,
    pub surah_id: i64,
    pub surah_name: String,
    pub ayah_number: i64,
    pub page: i64,
    pub juz: i64,
}

#[derive(Debug, Deserialize)]
pub struct PutLastReadReq {
    pub ayah_id: i64,
}

#[derive(Debug, Deserialize)]
pub struct AddBookmarkReq {
    pub ayah_id: i64,
    pub note: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct BookmarkOut {
    pub id: i64,
    pub ayah_id: i64,
    pub surah_id: i64,
    pub ayah_number: i64,
    pub page: i64,
    pub note: Option<String>,
}

// ===================== F0c: ayat per JUZ (khatmil reader) =====================

#[derive(Debug, Serialize)]
pub struct JuzAyahOut {
    pub id: i64,
    pub surah_id: i64,
    pub surah_name_latin: String,
    pub surah_name_arabic: String,
    pub ayah_number: i64,
    pub text_uthmani: String,
    pub text_imlaei: Option<String>,
    pub page: i64,
    pub juz: i64,
    pub translation: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct JuzAyahsOut {
    pub juz: i64,
    pub total_ayat: i64,
    pub ayahs: Vec<JuzAyahOut>,
}
