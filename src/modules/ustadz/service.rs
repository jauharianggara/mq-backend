//! Service modul ustadz.
use sqlx::MySqlPool;

use crate::modules::ustadz::dto::*;
use crate::shared::error::AppError;

fn dberr(e: sqlx::Error) -> AppError {
    tracing::error!("db: {e}");
    AppError::Internal(format!("db: {e}"))
}

/// Pastikan user punya baris ustadz_profiles (buat default bila belum).
async fn ensure_profile(pool: &MySqlPool, user_id: i64) -> Result<(), AppError> {
    sqlx::query("INSERT IGNORE INTO ustadz_profiles (user_id) VALUES (?)")
        .bind(user_id).execute(pool).await.map_err(dberr)?;
    Ok(())
}

/// Gate ustadz (Bagian III C.4): verified_at IS NOT NULL.
pub async fn require_verified(pool: &MySqlPool, user_id: i64) -> Result<(), AppError> {
    let row: Option<(Option<String>,)> = sqlx::query_as(
        "SELECT DATE_FORMAT(verified_at, '%Y-%m-%dT%H:%i:%sZ') FROM ustadz_profiles WHERE user_id = ?")
        .bind(user_id).fetch_optional(pool).await.map_err(dberr)?;
    match row {
        Some((Some(_),)) => Ok(()),
        Some((None,)) => Err(AppError::Forbidden("ustadz belum terverifikasi (verified_at NULL)".into())),
        None => Err(AppError::Forbidden("bukan ustadz".into())),
    }
}

pub async fn get_specializations(pool: &MySqlPool, user_id: i64) -> Result<Vec<SpecializationOut>, AppError> {
    ensure_profile(pool, user_id).await?;
    let rows: Vec<(i64, String, String)> = sqlx::query_as(
        "SELECT c.id, c.slug, c.name FROM ustadz_specializations us \
         JOIN question_categories c ON c.id = us.category_id WHERE us.ustadz_id = ? ORDER BY c.sort_order")
        .bind(user_id).fetch_all(pool).await.map_err(dberr)?;
    Ok(rows.into_iter().map(|r| SpecializationOut { category_id: r.0, slug: r.1, name: r.2 }).collect())
}

pub async fn put_specializations(pool: &MySqlPool, user_id: i64, req: PutSpecializationsReq) -> Result<(), AppError> {
    ensure_profile(pool, user_id).await?;
    if req.category_ids.len() > 20 {
        return Err(AppError::Unprocessable("maksimal 20 kategori".into()));
    }
    let mut tx = pool.begin().await.map_err(dberr)?;
    sqlx::query("DELETE FROM ustadz_specializations WHERE ustadz_id = ?")
        .bind(user_id).execute(&mut *tx).await.map_err(dberr)?;
    for cid in &req.category_ids {
        let n = sqlx::query("INSERT INTO ustadz_specializations (ustadz_id, category_id) \
                             SELECT ?, id FROM question_categories WHERE id = ?")
            .bind(user_id).bind(cid).execute(&mut *tx).await.map_err(dberr)?.rows_affected();
        if n == 0 {
            return Err(AppError::Unprocessable(format!("category_id {cid} tidak ada")));
        }
    }
    tx.commit().await.map_err(dberr)?;
    Ok(())
}

pub async fn get_availability(pool: &MySqlPool, user_id: i64) -> Result<AvailabilityOut, AppError> {
    ensure_profile(pool, user_id).await?;
    let (accepting, maxq, verified): (i8, i64, i8) = sqlx::query_as(
        "SELECT is_accepting_questions, max_active_questions, verified_at IS NOT NULL FROM ustadz_profiles WHERE user_id = ?")
        .bind(user_id).fetch_one(pool).await.map_err(dberr)?;
    Ok(AvailabilityOut { is_accepting_questions: accepting != 0, max_active_questions: maxq, verified: verified != 0 })
}

pub async fn put_availability(pool: &MySqlPool, user_id: i64, req: PutAvailabilityReq) -> Result<(), AppError> {
    ensure_profile(pool, user_id).await?;
    if !(1..=100).contains(&req.max_active_questions) {
        return Err(AppError::Unprocessable("max_active_questions 1-100".into()));
    }
    sqlx::query("UPDATE ustadz_profiles SET is_accepting_questions = ?, max_active_questions = ? WHERE user_id = ?")
        .bind(req.is_accepting_questions).bind(req.max_active_questions).bind(user_id)
        .execute(pool).await.map_err(dberr)?;
    Ok(())
}

pub async fn stats(pool: &MySqlPool, user_id: i64) -> Result<UstadzStats, AppError> {
    ensure_profile(pool, user_id).await?;
    let (total_reviewed, queue_pending_global, my_in_review, avg_min): (i64, i64, i64, Option<f64>) = sqlx::query_as(
        "SELECT \
           (SELECT COUNT(*) FROM memorization_submissions WHERE ustadz_id = ? AND status IN ('PASSED','REVISION','REJECTED')), \
           (SELECT COUNT(*) FROM memorization_submissions WHERE status = 'PENDING' AND ustadz_id IS NULL), \
           (SELECT COUNT(*) FROM memorization_submissions WHERE status = 'IN_REVIEW' AND ustadz_id = ?), \
           (SELECT CAST(AVG(TIMESTAMPDIFF(MINUTE, submitted_at, reviewed_at)) AS DOUBLE) FROM memorization_submissions WHERE ustadz_id = ? AND reviewed_at IS NOT NULL)")
        .bind(user_id).bind(user_id).bind(user_id)
        .fetch_one(pool).await.map_err(dberr)?;
    // tanya-ustadz (modul 9 menyusul) — placeholder agregat 0
    Ok(UstadzStats {
        total_reviewed, queue_pending_global, my_in_review, avg_review_minutes: avg_min,
        questions_answered: 0, questions_pending: 0,
    })
}

pub async fn get_detail(pool: &MySqlPool, user_id: i64) -> Result<crate::modules::ustadz::dto::UstadzDetailOut, AppError> {
    let r: (Option<String>, Option<String>) = sqlx::query_as(
        "SELECT pendidikan_terakhir, pengalaman_mengajar FROM ustadz_profiles WHERE user_id = ?")
        .bind(user_id).fetch_one(pool).await.map_err(dberr)?;
    Ok(crate::modules::ustadz::dto::UstadzDetailOut {
        pendidikan_terakhir: r.0, pengalaman_mengajar: r.1,
    })
}

pub async fn put_detail(pool: &MySqlPool, user_id: i64, req: crate::modules::ustadz::dto::UstadzDetailReq) -> Result<(), AppError> {
    let pend = req.pendidikan_terakhir.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let peng = req.pengalaman_mengajar.as_deref().map(str::trim).filter(|s| !s.is_empty());
    sqlx::query(
        "UPDATE ustadz_profiles SET          pendidikan_terakhir = COALESCE(?, pendidikan_terakhir),          pengalaman_mengajar = COALESCE(?, pengalaman_mengajar) WHERE user_id = ?")
        .bind(pend).bind(peng).bind(user_id)
        .execute(pool).await.map_err(dberr)?;
    Ok(())
}

pub async fn get_bank(pool: &MySqlPool, user_id: i64) -> Result<crate::modules::ustadz::dto::BankAccountOut, AppError> {
    let r: Option<(String, String, String)> = sqlx::query_as(
        "SELECT bank_name, bank_account_no, bank_account_name FROM ustadz_bank_accounts WHERE user_id = ?")
        .bind(user_id).fetch_optional(pool).await.map_err(dberr)?;
    Ok(match r {
        Some((b, n, an)) => crate::modules::ustadz::dto::BankAccountOut {
            bank_name: Some(b), bank_account_no: Some(n), bank_account_name: Some(an) },
        None => crate::modules::ustadz::dto::BankAccountOut {
            bank_name: None, bank_account_no: None, bank_account_name: None },
    })
}

pub async fn put_bank(pool: &MySqlPool, user_id: i64, req: crate::modules::ustadz::dto::BankAccountReq) -> Result<(), AppError> {
    let b = req.bank_name.trim();
    let n = req.bank_account_no.trim();
    let an = req.bank_account_name.trim();
    if b.is_empty() || n.is_empty() || an.is_empty() {
        return Err(AppError::Unprocessable("bank, nomor rekening, dan atas nama wajib diisi".into()));
    }
    sqlx::query(
        "INSERT INTO ustadz_bank_accounts (user_id, bank_name, bank_account_no, bank_account_name) VALUES (?, ?, ?, ?)          ON DUPLICATE KEY UPDATE bank_name = VALUES(bank_name), bank_account_no = VALUES(bank_account_no), bank_account_name = VALUES(bank_account_name)")
        .bind(user_id).bind(b).bind(n).bind(an)
        .execute(pool).await.map_err(dberr)?;
    Ok(())
}
