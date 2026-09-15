//! Worker scheduled_jobs (Bagian I Phase 3 — v11 reliability):
//! poll FOR UPDATE SKIP LOCKED -> tandai RUNNING di transaksi pendek -> eksekusi DI LUAR lock;
//! retry+backoff (3x -> DEAD); stale-RUNNING recovery; job ber-side-effect wajib dedupe_key.
//! Job: media.cleanup_orphan (UPLOADING > 24 jam -> FAILED + object dihapus best-effort).
use std::time::Duration;

use sqlx::mysql::{MySqlConnectOptions, MySqlPool, MySqlPoolOptions};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "mq_worker=info".into()))
        .init();

    let cfg = mq_backend_lib::config::AppConfig::from_env();
    let url = cfg.database_url.clone().expect("DATABASE_URL wajib");
    let opts: MySqlConnectOptions = url.parse().expect("DATABASE_URL tidak valid");
    let opts = opts.timezone(Some("+00:00".to_string()));
    let pool = MySqlPoolOptions::new().max_connections(2).connect_with(opts).await.expect("connect DB");
    let storage = match cfg.s3.as_ref() {
        Some(s) => Some(mq_backend_lib::infrastructure::storage::build(s).await),
        None => None,
    };
    // Bagian V: state ringan utk job visits (payment gateway + rate gates tak dipakai di sini)
    let visit_state = mq_backend_lib::state::AppState::new(pool.clone(), &cfg).await;

    tracing::info!("mq-worker: mulai (poll 30s)");
    let mut tick: u64 = 0;
    loop {
        tick += 1;
        ensure_cleanup_job(&pool).await;
        ensure_visit_jobs(&pool).await;
        ensure_khatmil_job(&pool).await;
        recover_stale(&pool).await;
        let mut done = 0;
        while let Some((id, job_type, attempts)) = claim_next(&pool).await {
            let r = execute(&pool, storage.as_ref(), &job_type, &visit_state).await;
            finish(&pool, id, attempts, r).await;
            done += 1;
            if done > 50 { break; } // safety
        }
        if done > 0 { tracing::info!("tick {tick}: {done} job dieksekusi"); }
        tokio::time::sleep(Duration::from_secs(30)).await;
    }
}

async fn ensure_cleanup_job(pool: &MySqlPool) {
    // INSERT IGNORE disengaja: duplikat dedupe_key per-jam DI-skip (semantik scheduler idempotent;
    // pengecualian sadar dari aturan ODKU — bukan jalur data-integrity).
    let _ = sqlx::query(
        "INSERT IGNORE INTO scheduled_jobs (job_type, payload, status, run_at, dedupe_key) \
         VALUES ('media.cleanup_orphan', CAST('{}' AS JSON), 'PENDING', \
                 DATE_ADD(UTC_TIMESTAMP(), INTERVAL 1 HOUR), \
                 CONCAT('media-cleanup-', DATE_FORMAT(UTC_TIMESTAMP(), '%Y%m%d%H')))")
        .execute(pool).await;
}

/// Bagian V: job periodik visits — poll & reminder tiap 15 mnt, timeout & review-window tiap jam
/// (dedupe_key periodik; INSERT IGNORE sadar utk scheduler idempotent).
async fn ensure_visit_jobs(pool: &MySqlPool) {
    for (job_type, dedupe, every) in [
        ("visit.payment_poll", "DATE_FORMAT(UTC_TIMESTAMP(), '%Y%m%d%H%i')", "15 MINUTE"),
        ("visit.reminder", "DATE_FORMAT(UTC_TIMESTAMP(), '%Y%m%d%H%i')", "15 MINUTE"),
        ("visit.confirm_timeout", "DATE_FORMAT(UTC_TIMESTAMP(), '%Y%m%d%H')", "1 HOUR"),
        ("visit.review_window", "DATE_FORMAT(UTC_TIMESTAMP(), '%Y%m%d%H')", "1 HOUR"),
    ] {
        let _ = sqlx::query(
            &format!(
                "INSERT IGNORE INTO scheduled_jobs (job_type, payload, status, run_at, dedupe_key) \
                 VALUES ('{job_type}', CAST('{{}}' AS JSON), 'PENDING', \
                         DATE_ADD(UTC_TIMESTAMP(), INTERVAL {every}), \
                         CONCAT('{job_type}-', {dedupe}))"
            ))
            .execute(pool).await;
    }
}

/// Khatmil: pengingat juz mangkrak harian 08:00 WIB (01:00 UTC) — dedupe_key per hari.
async fn ensure_khatmil_job(pool: &MySqlPool) {
    let _ = sqlx::query(
        "INSERT IGNORE INTO scheduled_jobs (job_type, payload, status, run_at, dedupe_key) \
         VALUES ('khatmil.stale_reminder', CAST('{}' AS JSON), 'PENDING', \
                 TIMESTAMP(DATE(UTC_TIMESTAMP() + INTERVAL 1 DAY), '01:00:00'), \
                 CONCAT('khatmil-stale-', DATE_FORMAT(UTC_TIMESTAMP(), '%Y%m%d')))")
        .execute(pool).await;
}

async fn recover_stale(pool: &MySqlPool) {
    let n = sqlx::query(
        "UPDATE scheduled_jobs SET status = 'FAILED', last_error = 'stale RUNNING (recovered)' \
         WHERE status = 'RUNNING' AND locked_at < DATE_SUB(UTC_TIMESTAMP(), INTERVAL 10 MINUTE)")
        .execute(pool).await.map(|r| r.rows_affected()).unwrap_or(0);
    if n > 0 { tracing::warn!("stale RUNNING direcovery: {n}"); }
}

/// RUNNING ditandai di transaksi pendek; eksekusi di luar (v11).
async fn claim_next(pool: &MySqlPool) -> Option<(i64, String, i64)> {
    let mut tx = pool.begin().await.ok()?;
    let row: Option<(i64, String, i64)> = sqlx::query_as(
        "SELECT id, job_type, attempts FROM scheduled_jobs \
         WHERE status = 'PENDING' AND run_at <= UTC_TIMESTAMP() \
         ORDER BY run_at LIMIT 1 FOR UPDATE SKIP LOCKED")
        .fetch_optional(&mut *tx).await.ok()?;
    let (id, job_type, attempts) = row?;
    let _ = sqlx::query("UPDATE scheduled_jobs SET status = 'RUNNING', started_at = UTC_TIMESTAMP(), locked_at = UTC_TIMESTAMP() WHERE id = ?")
        .bind(id).execute(&mut *tx).await;
    let _ = tx.commit().await;
    Some((id, job_type, attempts))
}

async fn execute(pool: &MySqlPool, storage: Option<&mq_backend_lib::infrastructure::storage::Storage>, job_type: &str, visit_state: &mq_backend_lib::state::AppState) -> Result<(), String> {
    match job_type {
        "media.cleanup_orphan" => {
            let rows: Vec<(i64, String)> = sqlx::query_as(
                "SELECT id, storage_key FROM media WHERE status = 'UPLOADING' \
                 AND created_at < DATE_SUB(UTC_TIMESTAMP(), INTERVAL 24 HOUR) LIMIT 100")
                .fetch_all(pool).await.map_err(|e| e.to_string())?;
            let mut cleaned = 0;
            for (id, key) in rows {
                if let Some(s) = storage {
                    // best-effort hapus object
                    let _ = s.client.delete_object().bucket(&s.bucket).key(&key).send().await;
                }
                let _ = sqlx::query("UPDATE media SET status = 'FAILED' WHERE id = ?").bind(id).execute(pool).await;
                cleaned += 1;
            }
            tracing::info!("media.cleanup_orphan: {cleaned} row dibersihkan");
            Ok(())
        }
        // ---- Bagian V: jobs visits ----
        "visit.payment_poll" => {
            let n = mq_backend_lib::modules::visits::payments::job_payment_poll(visit_state).await.map_err(|e| e.to_string())?;
            tracing::info!("visit.payment_poll: {n} payment ditutup");
            Ok(())
        }
        "visit.reminder" => {
            let n = mq_backend_lib::modules::visits::payments::job_reminder(visit_state).await.map_err(|e| e.to_string())?;
            if n > 0 { tracing::info!("visit.reminder: {n} kunjungan diingatkan"); }
            Ok(())
        }
        "visit.confirm_timeout" => {
            let n = mq_backend_lib::modules::visits::payments::job_confirm_timeout(visit_state).await.map_err(|e| e.to_string())?;
            if n > 0 { tracing::info!("visit.confirm_timeout: {n} auto-decline + refund"); }
            Ok(())
        }
        "visit.review_window" => {
            let n = mq_backend_lib::modules::visits::payments::job_review_window(visit_state).await.map_err(|e| e.to_string())?;
            if n > 0 { tracing::info!("visit.review_window: {n} review di-reveal"); }
            Ok(())
        }
        // ---- Khatmil: pengingat juz mangkrak (plan admin rev 3.3 F1.8) ----
        "khatmil.stale_reminder" => {
            let n = mq_backend_lib::modules::khatmil::service::job_stale_reminder(pool).await.map_err(|e| e.to_string())?;
            if n > 0 { tracing::info!("khatmil.stale_reminder: {n} pengingat terkirim"); }
            Ok(())
        }
        other => Err(format!("job_type tidak dikenal: {other}")),
    }
}

async fn finish(pool: &MySqlPool, id: i64, attempts: i64, result: Result<(), String>) {
    match result {
        Ok(()) => {
            let _ = sqlx::query("UPDATE scheduled_jobs SET status = 'DONE', finished_at = UTC_TIMESTAMP(), last_error = NULL WHERE id = ?")
                .bind(id).execute(pool).await;
        }
        Err(e) => {
            let attempts = attempts + 1;
            let (status, backoff) = if attempts >= 3 {
                ("DEAD", 0i64)
            } else {
                ("FAILED", 5 * attempts) // 5m, 10m
            };
            let _ = sqlx::query(
                "UPDATE scheduled_jobs SET status = ?, attempts = ?, last_error = ?, \
                 run_at = DATE_ADD(UTC_TIMESTAMP(), INTERVAL ? MINUTE) WHERE id = ?")
                .bind(status).bind(attempts).bind(&e).bind(backoff).bind(id)
                .execute(pool).await;
            tracing::error!("job {id} gagal (attempt {attempts}): {e}");
        }
    }
}
