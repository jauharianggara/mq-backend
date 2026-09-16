//! Payments v2 (Panggil Ustadz): Xendit invoices (kunjungan & top-up), webhook,
//! refund ke deposit santri, penghasilan ustadz, payout & worker jobs.
use sqlx::MySqlPool;

use crate::modules::visits::service::{dberr, log_history, notify, notify_admins, setting_i64};
use crate::shared::error::AppError;
use crate::state::AppState;

// ===================== kunjungan: payment + invoice =====================

pub async fn create_payment_pending(state: &AppState, visit_id: i64, amount: i64) -> Result<(i64, String), AppError> {
    let external_id = format!("visit-{visit_id}-{}", &uuid::Uuid::new_v4().simple().to_string()[..8]);
    let duration = setting_i64(&state.pool, "visit_invoice_duration_sec", 7200).await;
    let expires: String = sqlx::query_scalar(
        &format!("SELECT DATE_FORMAT(DATE_ADD(UTC_TIMESTAMP(), INTERVAL {duration} SECOND), '%Y-%m-%dT%H:%i:%sZ')"))
        .fetch_one(&state.pool).await.map_err(dberr)?;
    let ins = sqlx::query(
        "INSERT INTO payments (provider, external_id, amount, subject_type, subject_id, status, expires_at) \
         VALUES ('xendit', ?, ?, 'ustadz_visit', ?, 'PENDING', ?)")
        .bind(&external_id).bind(amount).bind(visit_id).bind(&expires)
        .execute(&state.pool).await.map_err(dberr)?;
    Ok((ins.last_insert_id() as i64, external_id))
}

/// Coba terbitkan invoice utk payment PENDING terbaru milik visit (tanpa duplikat).
/// Return invoice_url bila sukses.
pub async fn try_issue_invoice(state: &AppState, payment_id: i64, external_id: &str, amount: i64) -> Option<String> {
    let duration = setting_i64(&state.pool, "visit_invoice_duration_sec", 7200).await;
    let inv = state.payments.create_invoice(
        crate::infrastructure::xendit::CreateInvoice {
            external_id,
            amount,
            description: &format!("Panggil Ustadz #{external_id} — MQ Mujayarotul Faqih"),
            duration_sec: duration,
        }).await.ok()?;
    sqlx::query("UPDATE payments SET xendit_invoice_id = ? WHERE id = ?")
        .bind(&inv.id).bind(payment_id)
        .execute(&state.pool).await.ok()?;
    Some(inv.invoice_url)
}

/// Issue/reuse invoice utk visit (dipakai POST /visits/{id}/pay). Tanpa duplikat aktif.
pub async fn issue_invoice_for_visit(state: &AppState, visit_id: i64) -> Result<Option<String>, AppError> {
    let p = crate::modules::visits::service::fetch_payment(&state.pool, visit_id)
        .await?.ok_or_else(|| AppError::NotFound("payment tidak ada".into()))?;
    match p.status.as_str() {
        "PENDING" => {
            if let Some(inv_id) = &p.invoice_id {
                if !state.payments.is_mock() {
                    if let Ok(inv) = state.payments.get_invoice(inv_id).await {
                        if inv.status == "PAID" {
                            apply_visit_paid(state, &p.external_id, Some(inv_id), None, "{}").await?;
                            return Ok(Some(inv.invoice_url));
                        }
                        if inv.status == "PENDING" {
                            return Ok(Some(inv.invoice_url));
                        }
                    }
                } else if let Some(u) = mock_url(&p.external_id) {
                    return Ok(Some(u));
                }
            }
            let url = state.payments.create_invoice(
                crate::infrastructure::xendit::CreateInvoice {
                    external_id: &p.external_id,
                    amount: p.amount,
                    description: &format!("Panggil Ustadz #{visit_id} - MQ Mujayarotul Faqih"),
                    duration_sec: setting_i64(&state.pool, "visit_invoice_duration_sec", 7200).await,
                }).await;
            match url {
                Ok(inv) => {
                    sqlx::query("UPDATE payments SET xendit_invoice_id = ? WHERE id = ?")
                        .bind(&inv.id).bind(p.id)
                        .execute(&state.pool).await.map_err(dberr)?;
                    Ok(Some(inv.invoice_url))
                }
                Err(e) => {
                    tracing::warn!("invoice gagal utk visit {visit_id}: {e}");
                    Ok(None)
                }
            }
        }
        other => Err(AppError::Conflict(format!("payment berstatus {other}"))),
    }
}

fn mock_url(external_id: &str) -> Option<String> {
    Some(format!("mock://invoice/{external_id}"))
}

// ===================== webhook & simulasi =====================

/// Bayar visit pakai deposit: tandai payment PENDING milik visit ini jadi PAID
/// (channel DEPOSIT) + transisi visit REQUESTED -> WAITING_CONFIRM + notif ustadz.
/// Dipanggil handler pay-deposit SETELAH wallet::debit sukses.
pub async fn mark_visit_paid_by_deposit(state: &AppState, visit_id: i64) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE payments SET status = 'PAID', paid_at = UTC_TIMESTAMP(), channel = 'DEPOSIT', \
         raw_callback = CAST('{}' AS JSON) \
         WHERE subject_type = 'ustadz_visit' AND subject_id = ? AND status = 'PENDING'")
        .bind(visit_id)
        .execute(&state.pool)
        .await.map_err(dberr)?;
    let v = crate::modules::visits::service::fetch_visit(&state.pool, visit_id)
        .await?.ok_or_else(|| AppError::Internal("visit hilang".into()))?;
    if v.status == "REQUESTED" {
        let mut tx = state.pool.begin().await.map_err(dberr)?;
        let n = sqlx::query("UPDATE ustadz_visits SET status = 'WAITING_CONFIRM', paid_at = UTC_TIMESTAMP() WHERE id = ? AND status = 'REQUESTED'")
            .bind(visit_id).execute(&mut *tx).await.map_err(dberr)?.rows_affected();
        if n > 0 {
            log_history(&mut *tx, visit_id, Some("REQUESTED"), "WAITING_CONFIRM", None, "dibayar via deposit").await?;
            notify(&mut *tx, v.ustadz_id, "VISIT_PAID_WAITING", "Permintaan kunjungan baru",
                &format!("Santri memesan kunjungan untuk {}. Buka menu Kunjungan utk menerima/menolak (batas 3 jam).", v.scheduled_at),
                &visit_id.to_string()).await?;
        }
        tx.commit().await.map_err(dberr)?;
    }
    Ok(())
}

pub async fn apply_visit_paid(state: &AppState, external_id: &str, invoice_id: Option<&str>, channel: Option<&str>, raw: &str) -> Result<String, AppError> {
    let n = sqlx::query(
        "UPDATE payments SET status = 'PAID', paid_at = UTC_TIMESTAMP(), channel = COALESCE(?, channel), \
         raw_callback = CAST(? AS JSON) \
         WHERE (external_id = ? OR (? IS NOT NULL AND xendit_invoice_id = ?)) AND status = 'PENDING'")
        .bind(channel).bind(raw).bind(external_id).bind(invoice_id).bind(invoice_id)
        .execute(&state.pool).await.map_err(dberr)?.rows_affected();
    let row: Option<(i64, i64, String)> = sqlx::query_as(
        "SELECT id, subject_id, status FROM payments \
         WHERE external_id = ? OR (? IS NOT NULL AND xendit_invoice_id = ?) ORDER BY id DESC LIMIT 1")
        .bind(external_id).bind(invoice_id).bind(invoice_id)
        .fetch_optional(&state.pool).await.map_err(dberr)?;
    let (payment_id, visit_id, pay_status) = row.ok_or_else(|| AppError::NotFound("payment tidak dikenal".into()))?;
    let _ = payment_id;
    if n == 0 && pay_status != "PAID" {
        return Err(AppError::Conflict(format!("payment berstatus {pay_status}")));
    }
    let v = crate::modules::visits::service::fetch_visit(&state.pool, visit_id)
        .await?.ok_or_else(|| AppError::Internal("visit hilang".into()))?;
    if v.status == "REQUESTED" {
        let mut tx = state.pool.begin().await.map_err(dberr)?;
        let n2 = sqlx::query("UPDATE ustadz_visits SET status = 'WAITING_CONFIRM', paid_at = UTC_TIMESTAMP() WHERE id = ? AND status = 'REQUESTED'")
            .bind(visit_id).execute(&mut *tx).await.map_err(dberr)?.rows_affected();
        if n2 > 0 {
            log_history(&mut *tx, visit_id, Some("REQUESTED"), "WAITING_CONFIRM", None, "pembayaran diterima").await?;
            notify(&mut *tx, v.ustadz_id, "VISIT_PAID_WAITING", "Permintaan kunjungan baru",
                &format!("Santri memesan {} untuk {}. Buka menu Kunjungan utk menerima/menolak (batas 3 jam).",
                    "Kunjungan", v.scheduled_at), &visit_id.to_string()).await?;
        }
        tx.commit().await.map_err(dberr)?;
        return Ok("paid".into());
    }
    if matches!(v.status.as_str(), "CANCELED" | "DECLINED" | "PAYMENT_EXPIRED") {
        // late-PAID: dana kembali ke saldo santri (tanpa Xendit refund API)
        crate::modules::wallet::service::credit(&state.pool, v.user_id, v.price_total, "REFUND", "ustadz_visit", visit_id).await?;
        sqlx::query("UPDATE payments SET status = 'REFUNDED', refunded_amount = ? WHERE external_id = ?")
            .bind(v.price_total).bind(external_id)
            .execute(&state.pool).await.map_err(dberr)?;
        return Ok("paid->refund-deposit".into());
    }
    Ok("paid(no-op)".into())
}

pub async fn apply_visit_expired(state: &AppState, external_id: &str, invoice_id: Option<&str>, raw: &str) -> Result<String, AppError> {
    let n = sqlx::query(
        "UPDATE payments SET status = 'EXPIRED', raw_callback = CAST(? AS JSON) \
         WHERE (external_id = ? OR (? IS NOT NULL AND xendit_invoice_id = ?)) AND status = 'PENDING'")
        .bind(raw).bind(external_id).bind(invoice_id).bind(invoice_id)
        .execute(&state.pool).await.map_err(dberr)?.rows_affected();
    if n == 0 { return Ok("expired(no-op)".into()); }
    let row: Option<(i64,)> = sqlx::query_as(
        "SELECT subject_id FROM payments WHERE external_id = ? OR (? IS NOT NULL AND xendit_invoice_id = ?) ORDER BY id DESC LIMIT 1")
        .bind(external_id).bind(invoice_id).bind(invoice_id)
        .fetch_optional(&state.pool).await.map_err(dberr)?;
    if let Some((visit_id,)) = row {
        let mut tx = state.pool.begin().await.map_err(dberr)?;
        let n2 = sqlx::query("UPDATE ustadz_visits SET status = 'PAYMENT_EXPIRED' WHERE id = ? AND status = 'REQUESTED'")
            .bind(visit_id).execute(&mut *tx).await.map_err(dberr)?.rows_affected();
        if n2 > 0 {
            crate::modules::visits::service::log_history(&mut *tx, visit_id, Some("REQUESTED"), "PAYMENT_EXPIRED", None, "invoice kedaluwarsa").await?;
        }
        tx.commit().await.map_err(dberr)?;
    }
    Ok("expired".into())
}

// ===================== refund & penghasilan (wallet) =====================

/// Semua dana kunjungan yang sudah dibayar dikembalikan ke DEPOSIT santri.
pub async fn refund_visit_to_deposit(state: &AppState, visit_id: i64, reason: &str) -> Result<(), AppError> {
    let rows: Vec<(i64, String, i64, i64)> = sqlx::query_as(
        "SELECT id, external_id, amount, refunded_amount FROM payments \
         WHERE subject_type = 'ustadz_visit' AND subject_id = ? AND status = 'PAID'")
        .bind(visit_id).fetch_all(&state.pool).await.map_err(dberr)?;
    for (pid, external_id, amount, refunded) in rows {
        let sisa = amount - refunded;
        if sisa <= 0 { continue; }
        let owner: (i64,) = sqlx::query_as("SELECT user_id FROM ustadz_visits WHERE id = ?")
            .bind(visit_id).fetch_one(&state.pool).await.map_err(dberr)?;
        crate::modules::wallet::service::credit(&state.pool, owner.0, sisa, "REFUND", "ustadz_visit", visit_id).await?;
        sqlx::query("UPDATE payments SET status = 'REFUNDED', refunded_amount = ? WHERE id = ?")
            .bind(sisa).bind(pid)
            .execute(&state.pool).await.map_err(dberr)?;
        tracing::info!("refund {sisa} ke saldo santri (visit {visit_id}) — {reason} [{external_id}]");
    }
    Ok(())
}

/// Penghasilan ustadz masuk saldo saat kunjungan COMPLETED.
pub async fn earn_visit_income(state: &AppState, visit_id: i64, ustadz_id: i64) -> Result<(), AppError> {
    let rows: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT id, amount FROM payments \
         WHERE subject_type = 'ustadz_visit' AND subject_id = ? AND status = 'PAID'")
        .bind(visit_id).fetch_all(&state.pool).await.map_err(dberr)?;
    for (pid, amount) in rows {
        let sudah: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM wallet_transactions \
             WHERE user_id = ? AND tx_type = 'EARNING' AND subject_type = 'ustadz_visit' AND subject_id = ?")
            .bind(ustadz_id).bind(visit_id).fetch_one(&state.pool).await.map_err(dberr)?;
        if sudah > 0 { continue; }
        crate::modules::wallet::service::credit(&state.pool, ustadz_id, amount, "EARNING", "ustadz_visit", visit_id).await?;
    }
    Ok(())
}

// ===================== worker jobs =====================

pub async fn job_confirm_timeout(state: &AppState) -> Result<u64, AppError> {
    let timeout_h = setting_i64(&state.pool, "visit_confirm_timeout_hours", 3).await;
    let rows: Vec<(i64, i64, i64)> = sqlx::query_as(
        "SELECT id, user_id, ustadz_id FROM ustadz_visits \
         WHERE status = 'WAITING_CONFIRM' AND paid_at < DATE_SUB(UTC_TIMESTAMP(), INTERVAL ? HOUR) LIMIT 100")
        .bind(timeout_h)
        .fetch_all(&state.pool).await.map_err(dberr)?;
    let mut n = 0;
    for (visit_id, _santri, _ustadz) in rows {
        let mut tx = state.pool.begin().await.map_err(dberr)?;
        let n2 = sqlx::query("UPDATE ustadz_visits SET status = 'DECLINED', declined_at = UTC_TIMESTAMP(), decline_reason = 'tidak dikonfirmasi dalam batas waktu' WHERE id = ? AND status = 'WAITING_CONFIRM'")
            .bind(visit_id).execute(&mut *tx).await.map_err(dberr)?.rows_affected();
        if n2 > 0 {
            crate::modules::visits::service::log_history(&mut *tx, visit_id, Some("WAITING_CONFIRM"), "DECLINED", None, "auto-timeout konfirmasi").await?;
            tx.commit().await.map_err(dberr)?;
            refund_visit_to_deposit(state, visit_id, "confirm-timeout").await?;
            n += 1;
        } else {
            tx.rollback().await.map_err(dberr)?;
        }
    }
    Ok(n)
}

pub async fn job_payment_poll(state: &AppState) -> Result<u64, AppError> {
    if state.payments.is_mock() { return Ok(0); }
    let rows: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT external_id, xendit_invoice_id FROM payments \
         WHERE status = 'PENDING' AND created_at < DATE_SUB(UTC_TIMESTAMP(), INTERVAL 45 MINUTE) LIMIT 50")
        .fetch_all(&state.pool).await.map_err(dberr)?;
    let mut n = 0;
    for (external_id, invoice_id) in rows {
        let inv = match state.payments.get_invoice(invoice_id.as_deref().unwrap_or(&external_id)).await {
            Ok(i) => i,
            Err(e) => { tracing::warn!("poll {external_id}: {e}"); continue; }
        };
        match inv.status.as_str() {
            "PAID" | "SETTLED" => { apply_visit_paid(state, &external_id, Some(&inv.id), None, "{}").await?; n += 1; }
            "EXPIRED" | "EXPIRING" => { apply_visit_expired(state, &external_id, Some(&inv.id), "{}").await?; n += 1; }
            _ => {}
        }
    }
    Ok(n)
}

pub async fn job_reminder(state: &AppState) -> Result<u64, AppError> {
    let rows: Vec<(i64, i64, i64)> = sqlx::query_as(
        "SELECT id, user_id, ustadz_id FROM ustadz_visits \
         WHERE status = 'CONFIRMED' \
           AND scheduled_at BETWEEN UTC_TIMESTAMP() AND DATE_ADD(UTC_TIMESTAMP(), INTERVAL 2 HOUR) LIMIT 100")
        .fetch_all(&state.pool).await.map_err(dberr)?;
    let mut n = 0;
    for (visit_id, santri, ustadz) in rows {
        let sent: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM user_notifications WHERE template_code = 'VISIT_REMINDER' AND data LIKE ?")
            .bind(format!("%visit:{visit_id}%"))
            .fetch_one(&state.pool).await.map_err(dberr)?;
        if sent > 0 { continue; }
        let sched: String = sqlx::query_scalar(
            "SELECT DATE_FORMAT(scheduled_at, '%Y-%m-%dT%H:%i:%sZ') FROM ustadz_visits WHERE id = ?")
            .bind(visit_id).fetch_one(&state.pool).await.map_err(dberr)?;
        notify(&state.pool, santri, "VISIT_REMINDER", "Pengingat kunjungan",
            &format!("Kunjungan kurang 2 jam lagi ({sched}). Siapkan tempat & Al-Qur'an."), &visit_id.to_string()).await?;
        notify(&state.pool, ustadz, "VISIT_REMINDER", "Pengingat kunjungan",
            &format!("Kunjungan kurang 2 jam lagi ({sched})."), &visit_id.to_string()).await?;
        n += 1;
    }
    Ok(n)
}

pub async fn job_review_window(state: &AppState) -> Result<u64, AppError> {
    let days = setting_i64(&state.pool, "visit_review_window_days", 7).await;
    let rows: Vec<(i64,)> = sqlx::query_as(
        "SELECT id FROM ustadz_visits WHERE status = 'COMPLETED' AND completed_at < DATE_SUB(UTC_TIMESTAMP(), INTERVAL ? DAY) LIMIT 100")
        .bind(days)
        .fetch_all(&state.pool).await.map_err(dberr)?;
    let mut n = 0;
    for (visit_id,) in rows {
        let mut tx = state.pool.begin().await.map_err(dberr)?;
        sqlx::query("UPDATE visit_reviews SET revealed_at = UTC_TIMESTAMP() WHERE visit_id = ? AND revealed_at IS NULL")
            .bind(visit_id).execute(&mut *tx).await.map_err(dberr)?;
        let n2 = sqlx::query("UPDATE ustadz_visits SET status = 'REVIEWED', reviewed_at = UTC_TIMESTAMP() WHERE id = ? AND status = 'COMPLETED'")
            .bind(visit_id).execute(&mut *tx).await.map_err(dberr)?.rows_affected();
        if n2 > 0 {
            crate::modules::visits::service::log_history(&mut *tx, visit_id, Some("COMPLETED"), "REVIEWED", None, "window review lewat").await?;
        }
        tx.commit().await.map_err(dberr)?;
        n += 1;
    }
    Ok(n)
}

// ===================== payout ustadz =====================

pub async fn create_payout(state: &AppState, ustadz_id: i64, bank: &str, no: &str, an: &str, amount: i64) -> Result<i64, AppError> {
    let fee = setting_i64(&state.pool, "payout_fee_amount", 6500).await;
    let min = setting_i64(&state.pool, "payout_min_amount", 50000).await;
    if amount < min {
        return Err(AppError::Unprocessable(format!("minimum penarikan Rp {min}")));
    }
    // diterima = amount - fee; saldo didebit sebesar amount (anti tarik dua kali)
    crate::modules::wallet::service::debit(&state.pool, ustadz_id, amount, "PAYOUT", "payout_request", 0).await?;
    let ins = sqlx::query(
        "INSERT INTO payout_requests (ustadz_id, amount, fee, bank_name, bank_account_no, bank_account_name) \
         VALUES (?, ?, ?, ?, ?, ?)")
        .bind(ustadz_id).bind(amount - fee).bind(fee).bind(bank).bind(no).bind(an)
        .execute(&state.pool).await.map_err(dberr)?;
    let id = ins.last_insert_id() as i64;
    notify_admins(&state.pool, "Pengajuan penarikan dana baru",
        &format!("Ustadz mengajukan penarikan Rp {} (diterima Rp {}) ke {bank} {no}. Buka menu Penarikan utk ACC/tolak.", fmt_rp(amount), fmt_rp(amount - fee))).await;
    Ok(id)
}

pub fn fmt_rp(n: i64) -> String {
    n.to_string().chars().rev().collect::<Vec<_>>().chunks(3)
        .map(|c| c.iter().collect::<String>())
        .collect::<Vec<_>>().join(".").chars().rev().collect()
}

pub async fn ustadz_payouts(pool: &MySqlPool, ustadz_id: i64) -> Result<Vec<crate::modules::visits::dto::PayoutOut>, AppError> {
    let rows: Vec<(i64, i64, i64, String, String, String, String, Option<String>, String, Option<String>)> = sqlx::query_as(
        "SELECT id, amount, fee, bank_name, bank_account_no, bank_account_name, status, rejected_reason, \
         DATE_FORMAT(created_at, '%Y-%m-%dT%H:%i:%sZ'), DATE_FORMAT(processed_at, '%Y-%m-%dT%H:%i:%sZ') \
         FROM payout_requests WHERE ustadz_id = ? ORDER BY id DESC LIMIT 100")
        .bind(ustadz_id).fetch_all(pool).await.map_err(dberr)?;
    Ok(rows.into_iter().map(|r| crate::modules::visits::dto::PayoutOut {
        id: r.0, amount: r.1, fee: r.2, bank_name: r.3, bank_account_no: r.4,
        bank_account_name: r.5, status: r.6, rejected_reason: r.7, created_at: r.8, processed_at: r.9,
    }).collect())
}

pub async fn admin_list_payouts(pool: &MySqlPool, status: Option<&str>, limit: i64, cursor: Option<i64>) -> Result<(Vec<serde_json::Value>, Option<String>), AppError> {
    let rows: Vec<(i64, i64, i64, i64, String, String, String, String, String, String)> = sqlx::query_as(
        "SELECT pr.id, pr.ustadz_id, pr.amount, pr.fee, pr.bank_name, pr.bank_account_no, pr.bank_account_name, pr.status, \
         COALESCE(NULLIF(up.full_name, ''), '(tanpa nama)'), \
         DATE_FORMAT(pr.created_at, '%Y-%m-%dT%H:%i:%sZ') \
         FROM payout_requests pr \
         LEFT JOIN user_profiles up ON up.user_id = pr.ustadz_id \
         WHERE (? IS NULL OR pr.status = ?) AND (? IS NULL OR pr.id < ?) ORDER BY pr.id DESC LIMIT ?")
        .bind(status).bind(status).bind(cursor).bind(cursor).bind(limit + 1)
        .fetch_all(pool).await.map_err(dberr)?;
    let mut out = Vec::new();
    let mut next = None;
    for (i, r) in rows.into_iter().enumerate() {
        if (i as i64) == limit { next = Some(r.0.to_string()); break; }
        out.push(serde_json::json!({
            "id": r.0, "ustadz_id": r.1, "amount": r.2, "fee": r.3, "bank_name": r.4,
            "account_no": r.5, "account_name": r.6, "status": r.7, "ustadz_name": r.8, "created_at": r.9,
        }));
    }
    Ok((out, next))
}

pub async fn admin_approve(pool: &MySqlPool, admin_id: i64, payout_id: i64) -> Result<(), AppError> {
    let n = sqlx::query(
        "UPDATE payout_requests SET status = 'APPROVED', processed_by = ?, processed_at = UTC_TIMESTAMP() \
         WHERE id = ? AND status = 'PENDING'")
        .bind(admin_id).bind(payout_id)
        .execute(pool).await.map_err(dberr)?.rows_affected();
    if n == 0 { return Err(AppError::Conflict("permintaan tidak dalam status PENDING".into())); }
    Ok(())
}

pub async fn admin_reject(pool: &MySqlPool, admin_id: i64, payout_id: i64, reason: &str) -> Result<(), AppError> {
    let row: Option<(i64, i64)> = sqlx::query_as(
        "SELECT ustadz_id, amount FROM payout_requests WHERE id = ? AND status IN ('PENDING','APPROVED')")
        .bind(payout_id).fetch_optional(pool).await.map_err(dberr)?;
    let (ustadz_id, amount) = row.ok_or_else(|| AppError::Conflict("permintaan tidak bisa ditolak".into()))?;
    let mut tx = pool.begin().await.map_err(dberr)?;
    sqlx::query("UPDATE payout_requests SET status = 'REJECTED', rejected_reason = ?, processed_by = ?, processed_at = UTC_TIMESTAMP() WHERE id = ?")
        .bind(reason.trim()).bind(admin_id).bind(payout_id)
        .execute(&mut *tx).await.map_err(dberr)?;
    // dana kembali ke saldo ustadz
    sqlx::query("UPDATE wallets SET balance = balance + ? WHERE user_id = ?")
        .bind(amount).bind(ustadz_id)
        .execute(&mut *tx).await.map_err(dberr)?;
    sqlx::query(
        "INSERT INTO wallet_transactions (user_id, tx_type, amount, balance_after, subject_type, subject_id) \
         SELECT ?, 'REFUND', ?, balance, 'payout_request', ? FROM wallets WHERE user_id = ?")
        .bind(ustadz_id).bind(amount).bind(payout_id).bind(ustadz_id)
        .execute(&mut *tx).await.map_err(dberr)?;
    tx.commit().await.map_err(dberr)?;
    Ok(())
}

pub async fn admin_mark_transferred(pool: &MySqlPool, admin_id: i64, payout_id: i64) -> Result<(), AppError> {
    let n = sqlx::query(
        "UPDATE payout_requests SET status = 'TRANSFERRED', processed_by = ?, processed_at = UTC_TIMESTAMP() \
         WHERE id = ? AND status = 'APPROVED'")
        .bind(admin_id).bind(payout_id)
        .execute(pool).await.map_err(dberr)?.rows_affected();
    if n == 0 {
        return Err(AppError::Conflict("permintaan tidak dalam status APPROVED".into()));
    }
    Ok(())
}


// ===================== payout admin helpers =====================

pub async fn admin_payout_approve(pool: &MySqlPool, admin_id: i64, payout_id: i64) -> Result<(), AppError> {
    let n = sqlx::query(
        "UPDATE payout_requests SET status = 'APPROVED', processed_by = ?, processed_at = UTC_TIMESTAMP()          WHERE id = ? AND status = 'PENDING'")
        .bind(admin_id).bind(payout_id)
        .execute(pool).await.map_err(dberr)?.rows_affected();
    if n == 0 { return Err(AppError::Conflict("permintaan tidak dalam status PENDING".into())); }
    let row: Option<(i64, i64, String, String)> = sqlx::query_as(
        "SELECT ustadz_id, amount, bank_name, bank_account_no FROM payout_requests WHERE id = ?")
        .bind(payout_id).fetch_optional(pool).await.map_err(dberr)?;
    if let Some((uid, amount, bank, no)) = row {
        notify(pool, uid, "PAYOUT_APPROVED", "Penarikan disetujui",
            &format!("Pengajuan penarikan Rp {} disetujui — menunggu transfer ke {bank} {no}.", fmt_rp(amount)), "wallet").await?;
    }
    Ok(())
}

pub async fn admin_payout_reject(pool: &MySqlPool, admin_id: i64, payout_id: i64, reason: &str) -> Result<(), AppError> {
    // amount = diterima (setelah fee) — yang didebit saat pengajuan = amount + fee,
    // tolak harus mengembalikan SEMUA yang didebit (fee ikut kembali).
    let row: Option<(i64, i64, i64)> = sqlx::query_as(
        "SELECT ustadz_id, amount, fee FROM payout_requests WHERE id = ? AND status IN ('PENDING','APPROVED')")
        .bind(payout_id).fetch_optional(pool).await.map_err(dberr)?;
    let (ustadz_id, amount, fee) = row.ok_or_else(|| AppError::Conflict("permintaan tidak bisa ditolak".into()))?;
    let refunded = amount + fee;
    let mut tx = pool.begin().await.map_err(dberr)?;
    sqlx::query("UPDATE payout_requests SET status = 'REJECTED', rejected_reason = ?, processed_by = ?, processed_at = UTC_TIMESTAMP() WHERE id = ?")
        .bind(reason.trim()).bind(admin_id).bind(payout_id)
        .execute(&mut *tx).await.map_err(dberr)?;
    sqlx::query("UPDATE wallets SET balance = balance + ? WHERE user_id = ?")
        .bind(refunded).bind(ustadz_id)
        .execute(&mut *tx).await.map_err(dberr)?;
    sqlx::query(
        "INSERT INTO wallet_transactions (user_id, tx_type, amount, balance_after, subject_type, subject_id)          SELECT ?, 'REFUND', ?, balance, 'payout_request', ? FROM wallets WHERE user_id = ?")
        .bind(ustadz_id).bind(refunded).bind(payout_id).bind(ustadz_id)
        .execute(&mut *tx).await.map_err(dberr)?;
    tx.commit().await.map_err(dberr)?;
    notify(pool, ustadz_id, "PAYOUT_REJECTED", "Penarikan ditolak",
        &format!("Pengajuan penarikan Rp {} ditolak — dana kembali ke saldo Anda. Alasan: {}", fmt_rp(refunded), reason.trim()), "wallet").await?;
    Ok(())
}

pub async fn admin_payout_mark_transferred(pool: &MySqlPool, admin_id: i64, payout_id: i64) -> Result<(), AppError> {
    let n = sqlx::query(
        "UPDATE payout_requests SET status = 'TRANSFERRED', processed_by = ?, processed_at = UTC_TIMESTAMP()          WHERE id = ? AND status = 'APPROVED'")
        .bind(admin_id).bind(payout_id)
        .execute(pool).await.map_err(dberr)?.rows_affected();
    if n == 0 {
        return Err(AppError::Conflict("permintaan tidak dalam status APPROVED".into()));
    }
    let row: Option<(i64, i64, String, String)> = sqlx::query_as(
        "SELECT ustadz_id, amount, bank_name, bank_account_no FROM payout_requests WHERE id = ?")
        .bind(payout_id).fetch_optional(pool).await.map_err(dberr)?;
    if let Some((uid, amount, bank, no)) = row {
        notify(pool, uid, "PAYOUT_TRANSFERRED", "Dana telah ditransfer",
            &format!("Rp {} telah ditransfer ke {bank} {no}.", fmt_rp(amount)), "wallet").await?;
    }
    Ok(())
}

// ===================== topup deposit =====================

pub async fn create_topup_invoice(state: &AppState, user_id: i64, amount: i64) -> Result<(i64, Option<String>), AppError> {
    let external_id = format!("topup-{user_id}-{}", &uuid::Uuid::new_v4().simple().to_string()[..8]);
    sqlx::query(
        "INSERT INTO payments (provider, external_id, amount, subject_type, subject_id, status)          VALUES ('xendit', ?, ?, 'wallet_topup', ?, 'PENDING')")
        .bind(&external_id).bind(amount).bind(user_id)
        .execute(&state.pool).await.map_err(dberr)?;
    let inv = state.payments.create_invoice(crate::infrastructure::xendit::CreateInvoice {
        external_id: &external_id,
        amount,
        description: "Top-up deposit santri - MQ Mujayarotul Faqih",
        duration_sec: 86400,
    }).await.map_err(|e| AppError::Internal(format!("payment provider: {e}")))?;
    sqlx::query("UPDATE payments SET xendit_invoice_id = ? WHERE external_id = ?")
        .bind(&inv.id).bind(&external_id)
        .execute(&state.pool).await.map_err(dberr)?;
    Ok((0, Some(inv.invoice_url)))
}

pub async fn apply_topup_paid(state: &AppState, external_id: &str, channel: Option<&str>, raw: &str) -> Result<String, AppError> {
    let n = sqlx::query(
        "UPDATE payments SET status = 'PAID', paid_at = UTC_TIMESTAMP(), channel = COALESCE(?, channel),          raw_callback = CAST(? AS JSON)          WHERE external_id = ? AND status = 'PENDING' AND subject_type = 'wallet_topup'")
        .bind(channel).bind(raw).bind(external_id)
        .execute(&state.pool).await.map_err(dberr)?.rows_affected();
    if n == 0 { return Ok("topup(no-op)".into()); }
    let row: Option<(i64, i64)> = sqlx::query_as(
        "SELECT amount, subject_id FROM payments WHERE external_id = ?")
        .bind(external_id).fetch_optional(&state.pool).await.map_err(dberr)?;
    let (amount, user_id) = row.ok_or_else(|| AppError::NotFound("topup tidak dikenal".into()))?;
    crate::modules::wallet::service::credit(&state.pool, user_id, amount, "TOPUP", "xendit_topup", 0).await?;
    Ok("topup-paid".into())
}

pub async fn apply_topup_expired(state: &AppState, external_id: &str) -> Result<String, AppError> {
    let n = sqlx::query(
        "UPDATE payments SET status = 'EXPIRED' WHERE external_id = ? AND status = 'PENDING' AND subject_type = 'wallet_topup'")
        .bind(external_id)
        .execute(&state.pool).await.map_err(dberr)?.rows_affected();
    Ok(if n > 0 { "topup-expired".into() } else { "topup-expired(no-op)".into() })
}
