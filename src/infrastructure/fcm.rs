//! Infrastructure FCM — channel PUSH via Firebase Cloud Messaging (HTTP v1).
//!
//! Outbox pattern: modul lain tetap menulis notifikasi ke `user_notifications`
//! (channel IN_APP) di dalam transaksi bisnisnya masing-masing; pump mengambil
//! baris `pushed_at IS NULL` yang sudah COMMIT lalu mengirim push ke semua
//! device aktif user (`user_devices`), routing credential per `app_id`
//! (SANTRI_APP/USTADZ_APP). Token mati (unregistered) → device otomatis
//! dinonaktifkan. Kredensial = file service-account JSON per project Firebase,
//! path via env `FCM_SANTRI_SERVICE_ACCOUNT` / `FCM_USTADZ_SERVICE_ACCOUNT`.
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use sqlx::MySqlPool;

const OAUTH_URL: &str = "https://oauth2.googleapis.com/token";
const FCM_SEND_URL: &str = "https://fcm.googleapis.com/v1/projects";
const OAUTH_SCOPE: &str = "https://www.googleapis.com/auth/firebase.messaging";
/// Batas percobaan per baris utk error sementara sebelum ditinggal.
const MAX_ATTEMPTS: i16 = 5;
const PUMP_INTERVAL: Duration = Duration::from_secs(3);

// ============================================================
// Manager (per app_id)
// ============================================================

pub struct FcmManager {
    senders: HashMap<&'static str, Arc<FcmSender>>,
}

impl FcmManager {
    /// Muat kredensial dari env. Tidak ada env = manager kosong (PUSH nonaktif,
    /// IN_APP tetap jalan) — meniru pola opsional milik S3/Xendit.
    pub fn from_env() -> Self {
        let mut senders = HashMap::new();
        for (app_id, env) in [
            ("SANTRI_APP", "FCM_SANTRI_SERVICE_ACCOUNT"),
            ("USTADZ_APP", "FCM_USTADZ_SERVICE_ACCOUNT"),
        ] {
            let Ok(path) = std::env::var(env) else { continue };
            match FcmSender::from_service_account_file(&path) {
                Ok(s) => {
                    tracing::info!("FCM aktif utk {app_id} (project {})", s.project_id);
                    senders.insert(app_id, Arc::new(s));
                }
                Err(e) => tracing::error!(
                    "FCM {app_id}: gagal muat {env}={path}: {e} — push app ini NONAKTIF"
                ),
            }
        }
        Self { senders }
    }

    pub fn sender_for(&self, app_id: &str) -> Option<&Arc<FcmSender>> {
        self.senders.get(app_id)
    }

    pub fn is_empty(&self) -> bool {
        self.senders.is_empty()
    }
}

// ============================================================
// Sender (OAuth service account + FCM v1 send)
// ============================================================

#[derive(Debug)]
pub enum FcmError {
    /// Token device tidak berlaku lagi (unregistered/invalid) — nonaktifkan device.
    DeadToken,
    /// Gagal sementara (jaringan/429/5xx/auth) — boleh diulang.
    Transient(String),
}

pub struct FcmSender {
    pub project_id: String,
    client_email: String,
    key: jsonwebtoken::EncodingKey,
    http: reqwest::Client,
    cached: Mutex<Option<CachedToken>>,
}

struct CachedToken {
    value: String,
    expires_at: Instant,
}

#[derive(Serialize)]
struct JwtClaims<'a> {
    iss: &'a str,
    scope: &'a str,
    aud: &'a str,
    iat: i64,
    exp: i64,
}

impl FcmSender {
    fn from_service_account_file(path: &str) -> Result<Self, String> {
        let raw = std::fs::read_to_string(path).map_err(|e| format!("baca file: {e}"))?;
        #[derive(serde::Deserialize)]
        struct Sa {
            project_id: String,
            client_email: String,
            private_key: String,
        }
        let sa: Sa = serde_json::from_str(&raw).map_err(|e| format!("parse JSON service account: {e}"))?;
        let key = jsonwebtoken::EncodingKey::from_rsa_pem(sa.private_key.as_bytes())
            .map_err(|e| format!("private_key bukan RSA PEM valid: {e}"))?;
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(|e| format!("http client: {e}"))?;
        Ok(Self {
            project_id: sa.project_id,
            client_email: sa.client_email,
            key,
            http,
            cached: Mutex::new(None),
        })
    }

    /// Access token OAuth2 (cache sampai 60 dtk sebelum expiry).
    async fn access_token(&self) -> Result<String, FcmError> {
        if let Some(t) = self.cached.lock().unwrap().as_ref() {
            if Instant::now() < t.expires_at {
                return Ok(t.value.clone());
            }
        }
        let now = chrono::Utc::now().timestamp();
        let claims = JwtClaims {
            iss: &self.client_email,
            scope: OAUTH_SCOPE,
            aud: OAUTH_URL,
            iat: now,
            exp: now + 3600,
        };
        let jwt = jsonwebtoken::encode(&jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256), &claims, &self.key)
            .map_err(|e| FcmError::Transient(format!("sign jwt: {e}")))?;
        let resp = self
            .http
            .post(OAUTH_URL)
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
                ("assertion", jwt.as_str()),
            ])
            .send()
            .await
            .map_err(|e| FcmError::Transient(format!("oauth request: {e}")))?;
        let status = resp.status();
        let body = resp
            .text()
            .await
            .map_err(|e| FcmError::Transient(format!("oauth body: {e}")))?;
        if !status.is_success() {
            // Kredensial salah juga dilaporkan Transient: pump cap MAX_ATTEMPTS lalu
            // tinggal — jangan sampai device dinonaktifkan gara-gara config server.
            let snippet: String = body.chars().take(200).collect();
            return Err(FcmError::Transient(format!("oauth HTTP {status}: {snippet}")));
        }
        #[derive(serde::Deserialize)]
        struct TokenResp {
            access_token: String,
            expires_in: u64,
        }
        let tr: TokenResp = serde_json::from_str(&body).map_err(|e| FcmError::Transient(format!("oauth parse: {e}")))?;
        let expires_at = Instant::now() + Duration::from_secs(tr.expires_in.saturating_sub(60));
        *self.cached.lock().unwrap() = Some(CachedToken { value: tr.access_token.clone(), expires_at });
        Ok(tr.access_token)
    }

    /// Kirim satu push ke satu device token.
    async fn send(&self, token: &str, title: &str, body: &str, data: &serde_json::Value) -> Result<(), FcmError> {
        let access = self.access_token().await?;
        let url = format!("{FCM_SEND_URL}/{}/messages:send", self.project_id);
        let resp = self
            .http
            .post(&url)
            .bearer_auth(&access)
            .json(&build_message(token, title, body, data))
            .send()
            .await
            .map_err(|e| FcmError::Transient(format!("send: {e}")))?;
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        if status.is_success() {
            return Ok(());
        }
        Err(classify(status, &body))
    }
}

/// 400/404 yang menunjuk registration token = token mati; sisanya sementara.
fn classify(status: reqwest::StatusCode, body: &str) -> FcmError {
    let msg = serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v.get("error")?.get("message")?.as_str().map(str::to_string))
        .unwrap_or_else(|| body.chars().take(200).collect());
    match status.as_u16() {
        // 404 "Requested entity was not found" = registration token tidak dikenal
        404 => FcmError::DeadToken,
        400 if msg.to_lowercase().contains("token") => FcmError::DeadToken,
        _ => FcmError::Transient(format!("HTTP {status}: {msg}")),
    }
}

/// Payload `data` FCM wajib map string→string: ambil hanya field skalar dari
/// JSON kolom `data` (deeplink, entity id, dst.).
fn flatten_data(data: &serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
    let mut out = serde_json::Map::new();
    if let Some(obj) = data.as_object() {
        for (k, v) in obj {
            let s = match v {
                serde_json::Value::String(s) => Some(s.clone()),
                serde_json::Value::Number(n) => Some(n.to_string()),
                serde_json::Value::Bool(b) => Some(b.to_string()),
                _ => None,
            };
            if let Some(s) = s {
                out.insert(k.clone(), serde_json::Value::String(s));
            }
        }
    }
    out
}

fn build_message(token: &str, title: &str, body: &str, data: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "message": {
            "token": token,
            "notification": { "title": title, "body": body },
            "data": flatten_data(data),
            "android": { "priority": "HIGH" },
        }
    })
}

// ============================================================
// Pump outbox
// ============================================================

/// Loop pump — JANGAN dijalankan di dua proses sekaligus (belum ada claim
/// antar-instance; dua pump = push dobel).
pub async fn run_pump(pool: MySqlPool, mgr: Arc<FcmManager>) {
    let mut tick = tokio::time::interval(PUMP_INTERVAL);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tick.tick().await;
        match pump_once(&pool, &mgr).await {
            Ok(0) => {}
            Ok(n) => tracing::info!("push pump: {n} notifikasi diproses"),
            Err(e) => tracing::warn!("push pump: {e}"),
        }
    }
}

/// Satu batch: max 100 notifikasi pending, kirim per device aktif.
/// Baris selesai bila ada yang terkirim / semua tidak-retry (skip, token mati,
/// tanpa device). Sukses parsial TIDAK di-retry (anti push dobel); semua-gagal-
/// sementara → push_attempts + 1 (dicoba tick berikutnya s.d. MAX_ATTEMPTS).
pub async fn pump_once(pool: &MySqlPool, mgr: &FcmManager) -> Result<usize, sqlx::Error> {
    let rows = sqlx::query_as::<_, (i64, i64, String, String, String)>(
        "SELECT id, user_id, title, body, CAST(data AS CHAR) FROM user_notifications \
         WHERE pushed_at IS NULL AND push_attempts < ? ORDER BY id LIMIT 100",
    )
    .bind(MAX_ATTEMPTS)
    .fetch_all(pool)
    .await?;
    let total = rows.len();

    for (id, user_id, title, body_text, data_raw) in rows {
        let data: serde_json::Value = serde_json::from_str(&data_raw).unwrap_or(serde_json::json!({}));
        let devices = sqlx::query_as::<_, (i64, String, String)>(
            "SELECT id, push_token, app_id FROM user_devices \
             WHERE user_id = ? AND is_active = 1 AND push_token IS NOT NULL AND push_token <> ''",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?;

        let mut delivered = 0usize;
        let mut transient = 0usize;
        let mut dead_devices: Vec<i64> = Vec::new();
        for (dev_id, token, app_id) in &devices {
            let Some(sender) = mgr.sender_for(app_id) else { continue }; // ADMIN_WEB / tanpa kredensial
            match sender.send(token, &title, &body_text, &data).await {
                Ok(()) => delivered += 1,
                Err(FcmError::DeadToken) => {
                    tracing::info!("push notif {id}: token device {dev_id} mati — nonaktifkan");
                    dead_devices.push(*dev_id);
                }
                Err(FcmError::Transient(e)) => {
                    tracing::warn!("push notif {id} device {dev_id} gagal sementara: {e}");
                    transient += 1;
                }
            }
        }
        for dev_id in dead_devices {
            let _ = sqlx::query("UPDATE user_devices SET is_active = 0 WHERE id = ?")
                .bind(dev_id)
                .execute(pool)
                .await;
        }
        if delivered == 0 && transient > 0 {
            sqlx::query("UPDATE user_notifications SET push_attempts = push_attempts + 1 WHERE id = ?")
                .bind(id)
                .execute(pool)
                .await?;
        } else {
            sqlx::query(
                "UPDATE user_notifications SET pushed_at = UTC_TIMESTAMP(), push_attempts = push_attempts + 1 WHERE id = ?",
            )
            .bind(id)
            .execute(pool)
            .await?;
        }
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flatten_data_hanya_skalar_jadi_string() {
        let d = serde_json::json!({"deeplink":"question:5","assignment_id":7,"b":true,"nested":{"x":1},"arr":[1]});
        let out = flatten_data(&d);
        assert_eq!(out.get("deeplink").unwrap(), "question:5");
        assert_eq!(out.get("assignment_id").unwrap(), "7");
        assert_eq!(out.get("b").unwrap(), "true");
        assert!(out.get("nested").is_none() && out.get("arr").is_none());
    }

    #[test]
    fn build_message_bentuk_fcm_v1() {
        let m = build_message("tok", "Judul", "Isi", &serde_json::json!({"deeplink":"wallet"}));
        assert_eq!(m["message"]["token"], "tok");
        assert_eq!(m["message"]["notification"]["title"], "Judul");
        assert_eq!(m["message"]["data"]["deeplink"], "wallet");
        assert_eq!(m["message"]["android"]["priority"], "HIGH");
    }

    #[test]
    fn classify_pisahkan_dead_vs_transient() {
        assert!(matches!(
            classify(reqwest::StatusCode::NOT_FOUND, "{\"error\":{\"message\":\"Requested entity was not found.\"}}"),
            FcmError::DeadToken
        ));
        assert!(matches!(
            classify(
                reqwest::StatusCode::BAD_REQUEST,
                "{\"error\":{\"message\":\"The registration token is not a valid FCM registration token\"}}"
            ),
            FcmError::DeadToken
        ));
        assert!(matches!(classify(reqwest::StatusCode::INTERNAL_SERVER_ERROR, "boom"), FcmError::Transient(_)));
        assert!(matches!(classify(reqwest::StatusCode::TOO_MANY_REQUESTS, "{}"), FcmError::Transient(_)));
    }

    #[test]
    fn manager_kosong_tanpa_env() {
        // dotenvy hanya jalan di bin main, unit test tidak memuat .env.
        std::env::remove_var("FCM_SANTRI_SERVICE_ACCOUNT");
        std::env::remove_var("FCM_USTADZ_SERVICE_ACCOUNT");
        assert!(FcmManager::from_env().is_empty());
    }
}
