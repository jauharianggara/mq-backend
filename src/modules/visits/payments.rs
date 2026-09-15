//! Payments — Xendit integration + lifecycle (Bagian V).
//!
//! Invariant (plan rev 5/6):
//!  * satu visit = satu ACTIVE payment attempt (DB UNIQUE active_marker);
//!  * webhook/worker idempotent — PAID tidak boleh reopen visit terminal
//!    (late-PAID setelah cancel/decline/expired => reconciliation refund otomatis);
//!  * refund gagal (API belum aktif / error) => REFUND_PENDING_MANUAL + notif admin.
use sqlx::MySqlPool;

use crate::modules::visits::service::{dberr, log_history, notify, setting_i64};
use crate::shared::error::AppError;
use crate::state::AppState;

/// Buat payment PENDING + invoice Xendit utk visit baru. Return invoice_url (None bila
/// invoice gagal dibuat — retry via /pay; payment PENDING tetap ada, bukan duplikat).
pub async fn create_payment_for_visit(state: &AppState, visit_id: i64, amount: i64) -> Result<Option<String>, AppError> {
    let external_id = format!("visit-{visit_id}-{}", &uuid::Uuid::new_v4().simple().to_string()[..8]);
    let duration = setting_i64(&state.pool, "visit_invoice_duration_sec", 7200).await;
    let expires: chrono::NaiveDateTime = sqlx::query_scalar(
            &format!("SELECT DATE_ADD(UTC_TIMESTAMP(), INTERVAL {duration} SECOND)")).fetch_one(&state.pool).await.map_err(dberr)?;
    sqlx::query(
        "INSERT INTO payments (provider, external_id, amount, subject_type, subject_id, status, expires_at) \
         VALUES ('xendit', ?, ?, 'ustadz_visit', ?, 'PENDING', ?)")
        .bind(&external_id).bind(amount).bind(visit_id).bind(expires)
        .execute(&state.pool).await.map_err(dberr)?;
    match issue_invoice(state, visit_id).await {
        Ok(url) => Ok(url),
        Err(e) => {
            tracing::warn!("invoice belum terbit utk visit {visit_id}: {e} — retry via POST /visits/{visit_id}/pay");
            Ok(None)
        }
    }
}

/// Terbitkan/ulang invoice utk visit (single active attempt). Return invoice_url.
/// Reuse payment PENDING yang ada; hanya buat baru bila attempt lama sudah non-active
/// (EXPIRED/FAILED) — dan atomically (pay_active_uq menjaga race).
pub async fn issue_invoice(state: &AppState, visit_id: i64) -> Result<Option<String>, AppError> {
    loop {
        // 1) reuse attempt aktif
        let active: Option<(i64, String, Option<String>, i64)> = sqlx::query_as(
            "SELECT id, external_id, xendit_invoice_id, amount FROM payments \
             WHERE subject_type = 'ustadz_visit' AND subject_id = ? AND status = 'PENDING'")
            .bind(visit_id).fetch_optional(&state.pool).await.map_err(dberr)?;
        if let Some((_id, external_id, invoice_id, amount)) = active {
            if invoice_id.is_some() && !state.payments.is_mock() {
                // invoice masih valid — regenerate URL via GET (idempotent read)
                if let Ok(inv) = state.payments.get_invoice(invoice_id.as_deref().unwrap()).await {
                    if inv.status == "PENDING" || inv.status == "PAID" {
                        return Ok(Some(inv.invoice_url));
                    }
                }
            }
            let url = create_and_store(state, visit_id, &external_id, amount).await?;
            return Ok(Some(url));
        }
        // 2) tidak ada attempt aktif — cek visit masih REQUESTED & buat baru
        let v = crate::modules::visits::service::fetch_visit(&state.pool, visit_id)
            .await?.ok_or_else(|| AppError::NotFound("pesanan tidak ada".into()))?;
        if v.status != "REQUESTED" {
            return Err(AppError::Unprocessable(format!("status {} — tidak perlu pembayaran", v.status)));
        }
        let external_id = format!("visit-{visit_id}-{}", &uuid::Uuid::new_v4().simple().to_string()[..8]);
        let duration = setting_i64(&state.pool, "visit_invoice_duration_sec", 7200).await;
        let expires: String = sqlx::query_scalar("SELECT DATE_FORMAT(DATE_ADD(UTC_TIMESTAMP(), INTERVAL ? SECOND), '%Y-%m-%dT%H:%i:%sZ')")
            .bind(duration).fetch_one(&state.pool).await.map_err(dberr)?;
        let ins = sqlx::query(
            "INSERT INTO payments (provider, external_id, amount, subject_type, subject_id, status, expires_at) \
             VALUES ('xendit', ?, ?, 'ustadz_visit', ?, 'PENDING', ?)")
            .bind(&external_id).bind(v.price).bind(visit_id).bind(expires)
            .execute(&state.pool).await;
        match ins {
            Ok(_) => continue, // loop -> masuk cabang reuse
            Err(e) => {
                let m = format!("{e}");
                if m.contains("pay_active_uq") {
                    continue; // race: attempt lain dibuat bersamaan — reuse
                }
                return Err(dberr(e));
            }
        }
    }
}

async fn create_and_store(state: &AppState, visit_id: i64, external_id: &str, amount: i64) -> Result<String, AppError> {
    let inv = state.payments.create_invoice(crate::infrastructure::xendit::CreateInvoice {
        external_id,
        amount,
        description: &format!("Pesan Ustadz #{visit_id} — MQ Mujayarotul Faqih"),
        duration_sec: setting_i64(&state.pool, "visit_invoice_duration_sec", 7200).await,
        success_url: std::env::var("MQ_VISIT_SUCCESS_URL").ok().map(|s| leak(&s)).as_deref(),
        failure_url: std::env::var("MQ_VISIT_FAILURE_URL").ok().map(|s| leak(&s)).as_deref(),
        payer_email: None,
    }).await.map_err(|e| AppError::Internal(format!("payment provider: {e}")))?;
    sqlx::query("UPDATE payments SET xendit_invoice_id = ? WHERE external_id = ?")
        .bind(&inv.id).bind(external_id)
        .execute(&state.pool).await.map_err(dberr)?;
    Ok(inv.invoice_url)
}
fn leak(s: &String) -> String { s.clone() }

/// Terapkan PAID (dipakai webhook, worker poll, dan dev-simulate). Idempotent + no-reopen.
pub async fn apply_paid(state: &AppState, external_id: &str, invoice_id: Option<&str>, channel: Option<&str>, raw: &str) -> Result<String, AppError> {
    // payment transition PENDING -> PAID (rows_affected jadi penanda idempotent)
    let n = sqlx::query(
        "UPDATE payments SET status = 'PAID', paid_at = UTC_TIMESTAMP(), channel = COALESCE(?, channel), \
         raw_callback = CAST(? AS JSON) \
         WHERE (external_id = ? OR (? IS NOT NULL AND xendit_invoice_id = ?)) AND status = 'PENDING'")
        .bind(channel).bind(raw).bind(external_id).bind(invoice_id).bind(invoice_id)
        .execute(&state.pool).await.map_err(dberr)?.rows_affected();
    let pid_row: Option<(i64, i64, String)> = sqlx::query_as(
        "SELECT id, subject_id, status FROM payments WHERE external_id = ? OR (? IS NOT NULL AND xendit_invoice_id = ?) ORDER BY id DESC LIMIT 1")
        .bind(external_id).bind(invoice_id).bind(invoice_id)
        .fetch_optional(&state.pool).await.map_err(dberr)?;
    let (payment_id, visit_id, pay_status) = pid_row.ok_or_else(|| AppError::NotFound("payment tidak dikenal".into()))?;
    if n == 0 && pay_status != "PAID" {
        return Err(AppError::Conflict(format!("payment status {pay_status} — tidak bisa PAID")));
    }
    // visit transition (hanya REQUESTED -> WAITING_CONFIRM; terminal => reconciliation refund)
    let v = crate::modules::visits::service::fetch_visit(&state.pool, visit_id)
        .await?.ok_or_else(|| AppError::Internal("visit hilang".into()))?;
    if v.status == "REQUESTED" {
        let mut tx = state.pool.begin().await.map_err(dberr)?;
        let n2 = sqlx::query("UPDATE ustadz_visits SET status = 'WAITING_CONFIRM', paid_at = UTC_TIMESTAMP() WHERE id = ? AND status = 'REQUESTED'")
            .bind(visit_id).execute(&mut *tx).await.map_err(dberr)?.rows_affected();
        if n2 > 0 {
            log_history(&mut *tx, visit_id, Some("REQUESTED"), "WAITING_CONFIRM", None, "payment PAID").await?;
            let timeout_h = setting_i64(&state.pool, "visit_confirm_timeout_hours", 3).await;
            notify(&mut *tx, v.ustadz_id, "VISIT_PAID_WAITING", "Permintaan kunjungan baru",
                &format!("Santri memesan {} utk {}. Konfirmasi/tolak dalam {timeout_h} jam.", v.service_name, v.scheduled_at), &visit_id.to_string()).await?;
        }
        tx.commit().await.map_err(dberr)?;
        return Ok("paid".into());
    }
    // PAID datang terlambat di visit terminal/lanjut => jangan reopen; refund otomatis
    if matches!(v.status.as_str(), "CANCELED" | "DECLINED" | "PAYMENT_EXPIRED") {
        attempt_refund(state, visit_id, "late-PAID reconciliation").await?;
        return Ok("paid->refunded(late)".into());
    }
    Ok("paid(no-op)".into())
}

/// Terapkan EXPIRED dari Xendit (webhook/poll). Idempotent.
pub async fn apply_expired(state: &AppState, external_id: &str, invoice_id: Option<&str>, raw: &str) -> Result<String, AppError> {
    let n = sqlx::query(
        "UPDATE payments SET status = 'EXPIRED', raw_callback = CAST(? AS JSON) \
         WHERE (external_id = ? OR (? IS NOT NULL AND xendit_invoice_id = ?)) AND status = 'PENDING'")
        .bind(raw).bind(external_id).bind(invoice_id).bind(invoice_id)
        .execute(&state.pool).await.map_err(dberr)?.rows_affected();
    if n == 0 {
        return Ok("expired(no-op)".into());
    }
    let row: Option<(i64,)> = sqlx::query_as(
        "SELECT subject_id FROM payments WHERE external_id = ? OR (? IS NOT NULL AND xendit_invoice_id = ?) ORDER BY id DESC LIMIT 1")
        .bind(external_id).bind(invoice_id).bind(invoice_id)
        .fetch_optional(&state.pool).await.map_err(dberr)?;
    if let Some((visit_id,)) = row {
        let mut tx = state.pool.begin().await.map_err(dberr)?;
        let n2 = sqlx::query("UPDATE ustadz_visits SET status = 'PAYMENT_EXPIRED' WHERE id = ? AND status = 'REQUESTED'")
            .bind(visit_id).execute(&mut *tx).await.map_err(dberr)?.rows_affected();
        if n2 > 0 {
            log_history(&mut *tx, visit_id, Some("REQUESTED"), "PAYMENT_EXPIRED", None, "invoice kedaluwarsa").await?;
        }
        tx.commit().await.map_err(dberr)?;
    }
    Ok("expired".into())
}

/// Refund attempt utk visit (semua payment PAID milik visit). Sukses => REFUNDED;
/// gagal => REFUND_PENDING_MANUAL + notif admin (jalur manual — plan V).
pub async fn attempt_refund(state: &AppState, visit_id: i64, reason: &str) -> Result<(), AppError> {
    let rows: Vec<(i64, String, Option<String>, i64, i64)> = sqlx::query_as(
        "SELECT id, external_id, xendit_invoice_id, amount, refunded_amount FROM payments \
         WHERE subject_type = 'ustadz_visit' AND subject_id = ? AND status IN ('PAID','REFUND_REQUESTED')")
        .bind(visit_id).fetch_all(&state.pool).await.map_err(dberr)?;
    for (pid, external_id, invoice_id, amount, refunded) in rows {
        let sisa = amount - refunded;
        if sisa <= 0 {
            continue;
        }
        let inv_ref = invoice_id.clone().unwrap_or_else(|| external_id.clone());
        match state.payments.create_refund(&inv_ref, sisa, reason).await {
            Ok(rf) => {
                sqlx::query("UPDATE payments SET status = 'REFUNDED', refund_id = ?, refunded_amount = refunded_amount + ? WHERE id = ?")
                    .bind(&rf.id).bind(sisa).bind(pid)
                    .execute(&state.pool).await.map_err(dberr)?;
                tracing::info!("refund OK payment {pid} visit {visit_id}: {sisa}");
            }
            Err(e) => {
                sqlx::query("UPDATE payments SET status = 'REFUND_PENDING_MANUAL' WHERE id = ? AND status IN ('PAID','REFUND_REQUESTED')")
                    .bind(pid)
                    .execute(&state.pool).await.map_err(dberr)?;
                tracing::error!("refund GAGAL payment {pid} visit {visit_id}: {e}");
                crate::modules::visits::service::notify_admins(&state.pool, "VISIT_REFUND_PENDING_MANUAL",
                    "Refund manual diperlukan",
                    &format!("Payment {external_id} (visit {visit_id}) gagal refund otomatis: {reason}. Proses manual dari dashboard Xendit lalu tandai dikembalikan."),
                    &visit_id.to_string()).await;
            }
        }
    }
    Ok(())
}

// ===================== admin payments =====================

pub async fn admin_list_payments(pool: &MySqlPool, status: Option<&str>, limit: i64, cursor: Option<i64>) -> Result<(Vec<serde_json::Value>, Option<String>), AppError> {
    let rows: Vec<(i64, String, Option<String>, i64, String, Option<String>, i64, i64, String)> = sqlx::query_as(
        "SELECT id, external_id, xendit_invoice_id, amount, status, channel, refunded_amount, subject_id, DATE_FORMAT(created_at, '%Y-%m-%dT%H:%i:%sZ') \
         FROM payments WHERE (? IS NULL OR status = ?) AND (? IS NULL OR id < ?) ORDER BY id DESC LIMIT ?")
        .bind(status).bind(status).bind(cursor).bind(cursor).bind(limit + 1)
        .fetch_all(pool).await.map_err(dberr)?;
    let mut out = Vec::new();
    let mut next = None;
    for (i, r) in rows.into_iter().enumerate() {
        if (i as i64) == limit {
            next = Some(r.0.to_string());
            break;
        }
        out.push(serde_json::json!({
            "id": r.0, "external_id": r.1, "xendit_invoice_id": r.2, "amount": r.3,
            "status": r.4, "channel": r.5, "refunded_amount": r.6, "visit_id": r.7, "created_at": r.8,
        }));
    }
    Ok((out, next))
}

pub async fn admin_mark_refunded(pool: &MySqlPool, admin_id: i64, payment_id: i64) -> Result<(), AppError> {
    let row: Option<(i64, i64, i64)> = sqlx::query_as(
        "SELECT amount, subject_id, refunded_amount FROM payments WHERE id = ? AND subject_type = 'ustadz_visit'")
        .bind(payment_id).fetch_optional(pool).await.map_err(dberr)?;
    let (amount, visit_id, refunded) = row.ok_or_else(|| AppError::NotFound("payment tidak ada".into()))?;
    let n = sqlx::query(
        "UPDATE payments SET status = 'REFUNDED', refunded_amount = ?, refund_id = COALESCE(refund_id, 'manual') WHERE id = ? AND status IN ('REFUND_PENDING_MANUAL','PAID')")
        .bind(amount).bind(payment_id)
        .execute(pool).await.map_err(dberr)?.rows_affected();
    if n == 0 {
        return Err(AppError::Conflict("payment tidak dalam status refundable".into()));
    }
    let _ = refunded;
    log_history(pool, visit_id, None, &format!("PAYMENT-{payment_id}-REFUNDED"), Some(admin_id), "admin tandai dikembalikan (manual)").await?;
    Ok(())
}

// ===================== worker jobs (Bagian V) =====================

pub async fn job_confirm_timeout(state: &AppState) -> Result<usize, AppError> {
    let timeout_h = setting_i64(&state.pool, "visit_confirm_timeout_hours", 3).await;
    let rows: Vec<(i64, i64, i64)> = sqlx::query_as(
        "SELECT id, user_id, ustadz_id FROM ustadz_visits \
         WHERE status = 'WAITING_CONFIRM' AND paid_at < DATE_SUB(UTC_TIMESTAMP(), INTERVAL ? HOUR) LIMIT 100")
        .bind(timeout_h)
        .fetch_all(&state.pool).await.map_err(dberr)?;
    let mut n = 0;
    for (visit_id, santri_id, ustadz_id) in rows {
        let mut tx = state.pool.begin().await.map_err(dberr)?;
        let n2 = sqlx::query("UPDATE ustadz_visits SET status = 'DECLINED', declined_at = UTC_TIMESTAMP(), decline_reason = 'tidak dikonfirmasi dalam batas waktu' WHERE id = ? AND status = 'WAITING_CONFIRM'")
            .bind(visit_id).execute(&mut *tx).await.map_err(dberr)?.rows_affected();
        if n2 > 0 {
            log_history(&mut *tx, visit_id, Some("WAITING_CONFIRM"), "DECLINED", None, "auto-timeout konfirmasi — refund penuh").await?;
            notify(&mut *tx, santri_id, "VISIT_DECLINED_REFUNDED", "Konfirmasi kedaluwarsa",
                "Ustadz tidak merespons dalam batas waktu. Dana PENUH dikembalikan.", &visit_id.to_string()).await?;
            let _ = ustadz_id;
            tx.commit().await.map_err(dberr)?;
            attempt_refund(state, visit_id, "confirm-timeout").await?;
            n += 1;
        } else {
            tx.rollback().await.map_err(dberr)?;
        }
    }
    Ok(n)
}

pub async fn job_payment_poll(state: &AppState) -> Result<usize, AppError> {
    if state.payments.is_mock() {
        return Ok(0); // mock: tidak ada sumber eksternal utk dipoll
    }
    let rows: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT external_id, xendit_invoice_id FROM payments \
         WHERE status = 'PENDING' AND created_at < DATE_SUB(UTC_TIMESTAMP(), INTERVAL 45 MINUTE) LIMIT 50")
        .fetch_all(&state.pool).await.map_err(dberr)?;
    let mut n = 0;
    for (external_id, invoice_id) in rows {
        let inv = match state.payments.get_invoice(invoice_id.as_deref().unwrap_or(&external_id)).await {
            Ok(i) => i,
            Err(e) => {
                tracing::warn!("poll {external_id}: {e}");
                continue;
            }
        };
        match inv.status.as_str() {
            "PAID" | "SETTLED" => {
                apply_paid(state, &external_id, Some(&inv.id), None, "{}").await?;
                n += 1;
            }
            "EXPIRED" | "EXPIRING" => {
                apply_expired(state, &external_id, Some(&inv.id), "{}").await?;
                n += 1;
            }
            _ => {}
        }
    }
    Ok(n)
}

pub async fn job_reminder(state: &AppState) -> Result<usize, AppError> {
    let rows: Vec<(i64, i64, i64, String)> = sqlx::query_as(
        "SELECT v.id, v.user_id, v.ustadz_id, vst.name FROM ustadz_visits v \
         JOIN visit_service_types vst ON vst.id = v.service_type_id \
         WHERE v.status = 'CONFIRMED' \
           AND v.scheduled_at BETWEEN UTC_TIMESTAMP() AND DATE_ADD(UTC_TIMESTAMP(), INTERVAL 2 HOUR) LIMIT 100")
        .fetch_all(&state.pool).await.map_err(dberr)?;
    let mut n = 0;
    for (visit_id, santri_id, ustadz_id, service) in rows {
        // dedupe: skip bila notif reminder sudah pernah terkirim utk visit ini
        let sent: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM user_notifications WHERE template_code = 'VISIT_REMINDER' AND data LIKE ?")
            .bind(format!("%visit:{visit_id}%"))
            .fetch_one(&state.pool).await.map_err(dberr)?;
        if sent > 0 {
            continue;
        }
        let sched: String = sqlx::query_scalar("SELECT DATE_FORMAT(scheduled_at, '%Y-%m-%dT%H:%i:%sZ') FROM ustadz_visits WHERE id = ?")
            .bind(visit_id).fetch_one(&state.pool).await.map_err(dberr)?;
        let mins: i64 = sqlx::query_scalar("SELECT GREATEST(0, TIMESTAMPDIFF(MINUTE, UTC_TIMESTAMP(), ?))")
            .bind(sched).fetch_one(&state.pool).await.map_err(dberr)?;
        notify(&state.pool, santri_id, "VISIT_REMINDER", "Pengingat kunjungan",
            &format!("Kunjungan {service} kurang {mins} menit lagi. Siapkan tempat & Al-Qur'an."), &visit_id.to_string()).await?;
        notify(&state.pool, ustadz_id, "VISIT_REMINDER", "Pengingat kunjungan",
            &format!("Kunjungan {service} kurang {mins} menit lagi."), &visit_id.to_string()).await?;
        n += 1;
    }
    Ok(n)
}

pub async fn job_review_window(state: &AppState) -> Result<usize, AppError> {
    let days = setting_i64(&state.pool, "visit_review_window_days", 7).await;
    let rows: Vec<(i64,)> = sqlx::query_as(
        &format!("SELECT id FROM ustadz_visits WHERE status = 'COMPLETED' AND completed_at < DATE_SUB(UTC_TIMESTAMP(), INTERVAL {days} DAY) LIMIT 100"))
        .fetch_all(&state.pool).await.map_err(dberr)?;
    let mut n = 0;
    for (visit_id,) in rows {
        let mut tx = state.pool.begin().await.map_err(dberr)?;
        sqlx::query("UPDATE visit_reviews SET revealed_at = UTC_TIMESTAMP() WHERE visit_id = ? AND revealed_at IS NULL")
            .bind(visit_id).execute(&mut *tx).await.map_err(dberr)?;
        let n2 = sqlx::query("UPDATE ustadz_visits SET status = 'REVIEWED', reviewed_at = UTC_TIMESTAMP() WHERE id = ? AND status = 'COMPLETED'")
            .bind(visit_id).execute(&mut *tx).await.map_err(dberr)?.rows_affected();
        if n2 > 0 {
            log_history(&mut *tx, visit_id, Some("COMPLETED"), "REVIEWED", None, "window review lewat — reveal otomatis").await?;
        }
        tx.commit().await.map_err(dberr)?;
        n += 1;
    }
    Ok(n)
}
