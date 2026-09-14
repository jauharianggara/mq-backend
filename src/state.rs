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
            cache_str: Cache::builder()
                .time_to_live(std::time::Duration::from_secs(24 * 3600))
                .build(),
        }
    }

    pub fn dev_expose_tokens(&self) -> bool {
        self.dev_expose_tokens
    }

    /// true = boleh kirim (belum pernah dalam 60s); false = rate-limited
    pub fn resend_gate(&self, user_id: &i64) -> bool {
        if self.resend_gate_cache.contains_key(user_id) {
            return false;
        }
        self.resend_gate_cache.insert(*user_id, ());
        true
    }
}
