//! Service wallet admin: list saldo santri, riwayat, penyesuaian dgn ACC santri.
use sqlx::MySqlPool;

use crate::shared::error::AppError;

pub fn dberr(e: sqlx::Error) -> AppError {
    tracing::error!("wallet-admin db: {e}");
    AppError::Internal(format!("db: {e}"))
}

/// Daftar saldo per role (SANTRI / USTADZ) — nama + saldo.
pub async fn list_balances(pool: &MySqlPool, role: &str, q: Option<&str>, limit: i64, cursor: Option<i64>) -> Result<(Vec<serde_json::Value>, Option<String>), AppError> {
    let rows: Vec<(i64, String, i64)> = sqlx::query_as(
        "SELECT u.id, COALESCE(NULLIF(up.full_name,''),'(tanpa nama)'), COALESCE(w.balance, 0) \
         FROM users u \
         JOIN user_roles ur ON ur.user_id = u.id \
         JOIN roles r ON r.id = ur.role_id AND r.code = ? \
         LEFT JOIN user_profiles up ON up.user_id = u.id \
         LEFT JOIN wallets w ON w.user_id = u.id \
         WHERE u.status != 'DELETED' AND (? IS NULL OR up.full_name LIKE ?) \
           AND (? IS NULL OR u.id < ?) \
         ORDER BY u.id DESC LIMIT ?")
        .bind(role)
        .bind(q).bind(q.map(|s| format!("%{s}%")))
        .bind(cursor).bind(cursor).bind(limit + 1)
        .fetch_all(pool).await.map_err(dberr)?;
    let mut out = Vec::new();
    let mut next = None;
    for (i, r) in rows.into_iter().enumerate() {
        if (i as i64) == limit { next = Some(r.0.to_string()); break; }
        out.push(serde_json::json!({ "user_id": r.0, "full_name": r.1, "balance": r.2 }));
    }
    Ok((out, next))
}

/// Riwayat mutasi wallet seorang user.
pub async fn transactions(pool: &MySqlPool, user_id: i64, limit: i64, cursor: Option<i64>) -> Result<(Vec<serde_json::Value>, Option<String>), AppError> {
    let rows: Vec<(i64, String, i64, i64, Option<String>, Option<i64>, String)> = sqlx::query_as(
        "SELECT id, tx_type, amount, balance_after, subject_type, subject_id, \
         DATE_FORMAT(created_at, '%Y-%m-%dT%H:%i:%sZ') \
         FROM wallet_transactions WHERE user_id = ? AND (? IS NULL OR id < ?) ORDER BY id DESC LIMIT ?")
        .bind(user_id).bind(cursor).bind(cursor).bind(limit + 1)
        .fetch_all(pool).await.map_err(dberr)?;
    let mut out = Vec::new();
    let mut next = None;
    for (i, r) in rows.into_iter().enumerate() {
        if (i as i64) == limit { next = Some(r.0.to_string()); break; }
        out.push(serde_json::json!({
            "id": r.0, "tx_type": r.1, "amount": r.2, "balance_after": r.3,
            "subject_type": r.4, "subject_id": r.5, "created_at": r.6,
        }));
    }
    Ok((out, next))
}

/// Admin mengajukan penyesuaian saldo (dua langkah — menunggu ACC santri).
/// amount bisa negatif. TIDAK mengubah saldo.
pub async fn propose_adjustment(
    pool: &MySqlPool,
    admin_id: i64,
    user_id: i64,
    amount: i64,
    reason: &str,
) -> Result<i64, AppError> {
    if amount == 0 {
        return Err(AppError::Unprocessable("nominal tidak boleh nol".into()));
    }
    let reason = reason.trim();
    if reason.len() < 5 {
        return Err(AppError::Unprocessable("alasan wajib diisi (min 5 karakter)".into()));
    }
    let ins = sqlx::query(
        "INSERT INTO admin_wallet_adjustments (user_id, admin_id, amount, reason) VALUES (?, ?, ?, ?)")
        .bind(user_id).bind(admin_id).bind(amount).bind(reason)
        .execute(pool).await.map_err(dberr)?;
    let adj_id = ins.last_insert_id() as i64;
    // notif santri
    let _ = sqlx::query(
        "INSERT INTO user_notifications (user_id, template_code, title, body, data, channel) \
         VALUES (?, 'SALDO_ADJUST_REQUEST', 'Penyesuaian saldo dari admin', \
         'Admin mengajukan penyesuaian saldo Anda. Buka halaman Deposit untuk menyetujui/menolak.', \
         CAST('{\"deeplink\":\"wallet\"}' AS JSON), 'IN_APP')")
        .bind(user_id)
        .execute(pool).await;
    Ok(adj_id)
}

/// Daftar penyesuaian milik santri (utk kartu persetujuan di app).
pub async fn my_adjustments(pool: &MySqlPool, user_id: i64) -> Result<Vec<serde_json::Value>, AppError> {
    let rows: Vec<(i64, i64, i64, String, String, Option<String>)> = sqlx::query_as(
        "SELECT a.id, a.amount, a.admin_id, a.reason, a.status, \
         DATE_FORMAT(a.created_at, '%Y-%m-%dT%H:%i:%sZ') \
         FROM admin_wallet_adjustments a WHERE a.user_id = ? ORDER BY a.id DESC LIMIT 20")
        .bind(user_id).fetch_all(pool).await.map_err(dberr)?;
    Ok(rows.iter().map(|r| serde_json::json!({
        "id": r.0, "amount": r.1, "admin_id": r.2, "reason": r.3,
        "status": r.4, "created_at": r.5,
    })).collect())
}

/// Santri ACC penyesuaian -> saldo berubah + wallet_transactions ADJUST + notif admin.
pub async fn accept_adjustment(pool: &MySqlPool, user_id: i64, adjustment_id: i64) -> Result<(), AppError> {
    let row: Option<(i64, i64, i64, Option<i64>)> = sqlx::query_as(
        "SELECT id, amount, user_id, admin_id FROM admin_wallet_adjustments \
         WHERE id = ? AND user_id = ? AND status = 'PENDING'")
        .bind(adjustment_id).bind(user_id).fetch_optional(pool).await.map_err(dberr)?;
    let (adj_id, amount, _uid, admin_id) = row.ok_or_else(|| AppError::NotFound("penyesuaian tidak ditemukan".into()))?;
    // pastikan wallet ada (balance 0 bila belum pernah ada aktivitas)
    sqlx::query("INSERT IGNORE INTO wallets (user_id, balance) VALUES (?, 0)")
        .bind(user_id)
        .execute(pool).await.map_err(dberr)?;
    let mut tx = pool.begin().await.map_err(dberr)?;
    if amount > 0 {
        sqlx::query("UPDATE wallets SET balance = balance + ? WHERE user_id = ?")
            .bind(amount).bind(user_id)
            .execute(&mut *tx).await.map_err(dberr)?;
    } else {
        sqlx::query("UPDATE wallets SET balance = balance - ? WHERE user_id = ? AND balance >= ?")
            .bind(-amount).bind(user_id).bind(-amount)
            .execute(&mut *tx).await.map_err(dberr)?;
    }
    let after: i64 = sqlx::query_scalar("SELECT balance FROM wallets WHERE user_id = ?")
        .bind(user_id).fetch_one(&mut *tx).await.map_err(dberr)?;
    sqlx::query(
        "INSERT INTO wallet_transactions (user_id, tx_type, amount, balance_after, subject_type, subject_id) \
         VALUES (?, 'ADJUST', ?, ?, 'wallet_adjustment', ?)")
        .bind(user_id).bind(amount.abs()).bind(after).bind(adj_id)
        .execute(&mut *tx).await.map_err(dberr)?;
    sqlx::query("UPDATE admin_wallet_adjustments SET status = 'ACCEPTED', handled_at = UTC_TIMESTAMP() WHERE id = ?")
        .bind(adj_id).execute(&mut *tx).await.map_err(dberr)?;
    tx.commit().await.map_err(dberr)?;
    notify_adjustment_result(pool, adj_id, admin_id, user_id, true).await;
    Ok(())
}

/// Santri menolak penyesuaian.
pub async fn reject_adjustment(pool: &MySqlPool, user_id: i64, adjustment_id: i64) -> Result<(), AppError> {
    let row: Option<(Option<i64>,)> = sqlx::query_as(
        "SELECT admin_id FROM admin_wallet_adjustments WHERE id = ? AND user_id = ? AND status = 'PENDING'")
        .bind(adjustment_id).bind(user_id).fetch_optional(pool).await.map_err(dberr)?;
    let (admin_id,) = row.ok_or_else(|| AppError::NotFound("penyesuaian tidak ditemukan / sudah diproses".into()))?;
    let n = sqlx::query(
        "UPDATE admin_wallet_adjustments SET status = 'REJECTED', handled_at = UTC_TIMESTAMP() \
         WHERE id = ? AND user_id = ? AND status = 'PENDING'")
        .bind(adjustment_id).bind(user_id)
        .execute(pool).await.map_err(dberr)?.rows_affected();
    if n == 0 {
        return Err(AppError::NotFound("penyesuaian tidak ditemukan / sudah diproses".into()));
    }
    notify_adjustment_result(pool, adjustment_id, admin_id, user_id, false).await;
    Ok(())
}

/// Notifikasi hasil penyesuaian ke admin pengaju (SALDO_ADJUST_RESULT).
async fn notify_adjustment_result(pool: &MySqlPool, adj_id: i64, admin_id: Option<i64>, user_id: i64, accepted: bool) {
    let Some(admin_id) = admin_id else { return };
    let name: String = sqlx::query_scalar(
        "SELECT COALESCE(NULLIF(up.full_name,''), '(tanpa nama)') FROM users u \
         LEFT JOIN user_profiles up ON up.user_id = u.id WHERE u.id = ?")
        .bind(user_id).fetch_one(pool).await.unwrap_or_else(|_| "(tanpa nama)".into());
    let _ = sqlx::query(
        "INSERT INTO user_notifications (user_id, template_code, title, body, data, channel) \
         VALUES (?, 'SALDO_ADJUST_RESULT', ?, ?, CAST('{\"deeplink\":\"wallet-adjustments\"}' AS JSON), 'IN_APP')")
        .bind(admin_id)
        .bind(if accepted { "Penyesuaian saldo disetujui" } else { "Penyesuaian saldo ditolak" })
        .bind(format!(
            "{name} {} penyesuaian #{adj_id}. Buka Saldo > tab Penyesuaian utk detail.",
            if accepted { "menyetujui" } else { "menolak" }
        ))
        .execute(pool).await;
}

/// Monitoring semua penyesuaian (utk halaman admin Saldo) — nama user + nama admin.
pub async fn list_adjustments(
    pool: &MySqlPool,
    status: Option<&str>,
    limit: i64,
    cursor: Option<i64>,
) -> Result<(Vec<serde_json::Value>, Option<String>), AppError> {
    let rows: Vec<(i64, i64, String, Option<String>, i64, String, String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT a.id, a.user_id, COALESCE(NULLIF(up.full_name,''),'(tanpa nama)'), \
         aa.email, a.amount, a.reason, a.status, \
         DATE_FORMAT(a.created_at, '%Y-%m-%dT%H:%i:%sZ'), DATE_FORMAT(a.handled_at, '%Y-%m-%dT%H:%i:%sZ') \
         FROM admin_wallet_adjustments a \
         LEFT JOIN user_profiles up ON up.user_id = a.user_id \
         LEFT JOIN users aa ON aa.id = a.admin_id \
         WHERE (? IS NULL OR a.status = ?) AND (? IS NULL OR a.id < ?) \
         ORDER BY a.id DESC LIMIT ?")
        .bind(status).bind(status)
        .bind(cursor).bind(cursor).bind(limit + 1)
        .fetch_all(pool).await.map_err(dberr)?;
    let mut out = Vec::new();
    let mut next = None;
    for (i, r) in rows.into_iter().enumerate() {
        if i as i64 == limit { next = Some(r.0.to_string()); break; }
        out.push(serde_json::json!({
            "id": r.0, "user_id": r.1, "user_name": r.2, "admin_email": r.3,
            "amount": r.4, "reason": r.5, "status": r.6,
            "created_at": r.7, "handled_at": r.8,
        }));
    }
    Ok((out, next))
}
