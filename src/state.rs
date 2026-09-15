//! AppState — shared ke seluruh handler.
use std::sync::Arc;

use moka::sync::Cache;
use sqlx::MySqlPool;

use crate::config::AppConfig;
use crate::infrastructure::storage::{self, Storage};

#[derive(Clone)]
pub struct AppState {
    pub pool: MySqlPool,
    pub jwt_secret: Arc<String>,
    pub storage: Option<Arc<Storage>>,
    dev_expose_tokens: bool,
    /// cache string generik in-process (mis. quran:surahs 24h)
    pub cache_str: Cache<&'static str, std::sync::Arc<String>>,
    /// gate resend-verification 60s per user (in-process — keputusan #13)
    resend_gate_cache: Cache<i64, ()>,
    /// gate klaim juz 5s per user
    khatmil_claim_cache: Cache<i64, ()>,
    /// gate progress khatmil 30s per user (keputusan #13 — rate-limit di service, bukan DB)
    khatmil_progress_cache: Cache<i64, ()>,
    /// Bagian V gates: booking 30s / nearby 5s / lokasi 10s / chat 2s per user
    visit_create_cache: Cache<i64, ()>,
    visit_nearby_cache: Cache<i64, ()>,
    visit_location_cache: Cache<i64, ()>,
    visit_chat_cache: Cache<i64, ()>,
    /// Bagian V: payment gateway (Xendit / mock saat secret kosong)
    pub payments: std::sync::Arc<crate::infrastructure::xendit::PaymentGateway>,
}

impl AppState {
    pub async fn new(pool: MySqlPool, cfg: &AppConfig) -> Self {
        let jwt_secret = cfg.jwt_secret.clone().unwrap_or_else(|| {
            tracing::warn!("JWT_SECRET tidak diset — pakai dev-default (JANGAN di produksi)");
            "dev-only-secret-change-me-0123456789abcdef".to_string()
        });
        let s3 = match &cfg.s3 {
            Some(s3c) => Some(Arc::new(storage::build(s3c).await)),
            None => {
                tracing::warn!("S3 belum dikonfigurasi — modul media tidak aktif");
                None
            }
        };
        Self {
            pool,
            jwt_secret: Arc::new(jwt_secret),
            storage: s3,
            dev_expose_tokens: std::env::var("MQ_DEV_EXPOSE_TOKENS")
                .map(|v| v == "true")
                .unwrap_or(false),
            resend_gate_cache: Cache::builder()
                .time_to_live(std::time::Duration::from_secs(60))
                .build(),
            khatmil_claim_cache: Cache::builder()
                .time_to_live(std::time::Duration::from_secs(5))
                .build(),
            khatmil_progress_cache: Cache::builder()
                .time_to_live(std::time::Duration::from_secs(30))
                .build(),
            visit_create_cache: Cache::builder()
                .time_to_live(std::time::Duration::from_secs(30))
                .build(),
            visit_nearby_cache: Cache::builder()
                .time_to_live(std::time::Duration::from_secs(5))
                .build(),
            visit_location_cache: Cache::builder()
                .time_to_live(std::time::Duration::from_secs(10))
                .build(),
            visit_chat_cache: Cache::builder()
                .time_to_live(std::time::Duration::from_secs(2))
                .build(),
            payments: std::sync::Arc::new(crate::infrastructure::xendit::PaymentGateway::from_env()),
            cache_str: Cache::builder()
                .time_to_live(std::time::Duration::from_secs(24 * 3600))
                .build(),
        }
    }

    pub fn dev_expose_tokens(&self) -> bool {
        self.dev_expose_tokens
    }

    /// true = boleh kirim (belum pernah dalam 60s); false = rate-limited
    fn gate(cache: &Cache<i64, ()>, user_id: &i64) -> bool {
        if cache.contains_key(user_id) {
            return false;
        }
        cache.insert(*user_id, ());
        true
    }
    pub fn resend_gate(&self, user_id: &i64) -> bool {
        Self::gate(&self.resend_gate_cache, user_id)
    }
    pub fn khatmil_claim_gate(&self, user_id: &i64) -> bool {
        Self::gate(&self.khatmil_claim_cache, user_id)
    }
    pub fn khatmil_progress_gate(&self, user_id: &i64) -> bool {
        Self::gate(&self.khatmil_progress_cache, user_id)
    }
    pub fn visit_create_gate(&self, user_id: &i64) -> bool {
        Self::gate(&self.visit_create_cache, user_id)
    }
    pub fn visit_nearby_gate(&self, user_id: &i64) -> bool {
        Self::gate(&self.visit_nearby_cache, user_id)
    }
    pub fn visit_location_gate(&self, user_id: &i64) -> bool {
        Self::gate(&self.visit_location_cache, user_id)
    }
    pub fn visit_chat_gate(&self, user_id: &i64) -> bool {
        Self::gate(&self.visit_chat_cache, user_id)
    }
}
