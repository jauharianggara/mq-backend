//! Modul wallet — deposit santri & penghasilan ustadz (Panggil Ustadz v2).
//!
//! Aturan: saldo HANYA berubah lewat credit()/debit() di modul ini (selalu
//! dalam transaksi + catat wallet_transactions). Refund apapun = kredit deposit;
//! Xendit Refund API tidak dipakai di v2.
use sqlx::MySqlPool;

use crate::shared::error::AppError;

pub fn dberr(e: sqlx::Error) -> AppError {
    tracing::error!("wallet db: {e}");
    AppError::Internal(format!("db: {e}"))
}

/// Pastikan baris wallet user ada.
pub async fn ensure(pool: &MySqlPool, user_id: i64) -> Result<(), AppError> {
    sqlx::query("INSERT IGNORE INTO wallets (user_id, balance) VALUES (?, 0)")
        .bind(user_id)
        .execute(pool)
        .await
        .map_err(dberr)?;
    Ok(())
}

pub async fn balance(pool: &MySqlPool, user_id: i64) -> Result<i64, AppError> {
    ensure(pool, user_id).await?;
    let b: i64 = sqlx::query_scalar("SELECT balance FROM wallets WHERE user_id = ?")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .map_err(dberr)?;
    Ok(b)
}

/// Kredit (uang masuk): top-up, refund, penghasilan ustadz.
pub async fn credit(
    pool: &MySqlPool,
    user_id: i64,
    amount: i64,
    tx_type: &str, // TOPUP | REFUND | EARNING | ADJUST
    subject_type: &str,
    subject_id: i64,
) -> Result<i64, AppError> {
    ensure(pool, user_id).await?;
    let mut tx = pool.begin().await.map_err(dberr)?;
    sqlx::query("UPDATE wallets SET balance = balance + ? WHERE user_id = ?")
        .bind(amount)
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .map_err(dberr)?;
    let after: i64 = sqlx::query_scalar("SELECT balance FROM wallets WHERE user_id = ?")
        .bind(user_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(dberr)?;
    sqlx::query(
        "INSERT INTO wallet_transactions (user_id, tx_type, amount, balance_after, subject_type, subject_id) \
         VALUES (?, ?, ?, ?, ?, ?)")
        .bind(user_id).bind(tx_type).bind(amount).bind(after).bind(subject_type).bind(subject_id)
        .execute(&mut *tx)
        .await
        .map_err(dberr)?;
    tx.commit().await.map_err(dberr)?;
    Ok(after)
}

/// Debit (uang keluar): bayar kunjungan dgn deposit, penarikan ustadz.
/// Gagal bila saldo kurang (422 saldo_tidak_cukup).
pub async fn debit(
    pool: &MySqlPool,
    user_id: i64,
    amount: i64,
    tx_type: &str, // PAYMENT | PAYOUT
    subject_type: &str,
    subject_id: i64,
) -> Result<i64, AppError> {
    ensure(pool, user_id).await?;
    let mut tx = pool.begin().await.map_err(dberr)?;
    let n = sqlx::query("UPDATE wallets SET balance = balance - ? WHERE user_id = ? AND balance >= ?")
        .bind(amount)
        .bind(user_id)
        .bind(amount)
        .execute(&mut *tx)
        .await
        .map_err(dberr)?
        .rows_affected();
    if n == 0 {
        tx.rollback().await.map_err(dberr)?;
        return Err(AppError::Unprocessable("saldo_tidak_cukup".into()));
    }
    let after: i64 = sqlx::query_scalar("SELECT balance FROM wallets WHERE user_id = ?")
        .bind(user_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(dberr)?;
    sqlx::query(
        "INSERT INTO wallet_transactions (user_id, tx_type, amount, balance_after, subject_type, subject_id) \
         VALUES (?, ?, ?, ?, ?, ?)")
        .bind(user_id).bind(tx_type).bind(amount).bind(after).bind(subject_type).bind(subject_id)
        .execute(&mut *tx)
        .await
        .map_err(dberr)?;
    tx.commit().await.map_err(dberr)?;
    Ok(after)
}
