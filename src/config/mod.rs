//! Konfigurasi aplikasi — semua via env (.env), tidak ada secret hardcoded.
use std::net::SocketAddr;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub listen_addr: SocketAddr,
    pub database_url: Option<String>,
    pub jwt_secret: Option<String>,
    pub cache_driver: String,
    pub s3: Option<S3Config>,
}

#[derive(Debug, Clone)]
pub struct S3Config {
    pub endpoint: String,
    /// Endpoint internal utk runtime ops (HEAD/GET range) — default = endpoint.
    /// Presign SELALU pakai `endpoint` (host publik utk client device).
    pub endpoint_internal: Option<String>,
    pub bucket: String,
    pub access_key: String,
    pub secret_key: String,
}

impl AppConfig {
    pub fn from_env() -> Self {
        let listen_addr = std::env::var("LISTEN_ADDR")
            .unwrap_or_else(|_| "127.0.0.1:8290".to_string())
            .parse()
            .expect("LISTEN_ADDR tidak valid (contoh: 127.0.0.1:8290)");

        let database_url = std::env::var("DATABASE_URL").ok();
        if database_url.is_none() {
            tracing::warn!("DATABASE_URL belum diset (dev OK; WAJIB saat modul DB Phase 3)");
        }

        let jwt_secret = std::env::var("JWT_SECRET").ok();
        if let Some(s) = &jwt_secret {
            assert!(s.len() >= 32, "JWT_SECRET minimal 32 byte (keputusan plan gotcha)");
        } else {
            tracing::warn!("JWT_SECRET belum diset (dev OK; WAJIB + >=32 byte saat modul auth Phase 3 — panic saat kurang)");
        }

        let cache_driver = std::env::var("CACHE_DRIVER").unwrap_or_else(|_| "memory".into());

        let s3 = match (
            std::env::var("S3_ENDPOINT").ok(),
            std::env::var("S3_BUCKET").ok(),
            std::env::var("S3_ACCESS_KEY").ok(),
            std::env::var("S3_SECRET_KEY").ok(),
        ) {
            (Some(endpoint), Some(bucket), Some(access_key), Some(secret_key)) => {
                let endpoint_internal = std::env::var("S3_ENDPOINT_INTERNAL").ok();
                tracing::info!("S3 object storage aktif: {endpoint} bucket={bucket} (internal: {})", endpoint_internal.as_deref().unwrap_or("= endpoint"));
                Some(S3Config { endpoint, endpoint_internal, bucket, access_key, secret_key })
            }
            _ => {
                tracing::warn!("S3_* belum lengkap (dev OK; WAJIB saat modul media 14)");
                None
            }
        };

        Self { listen_addr, database_url, jwt_secret, cache_driver, s3 }
    }
}
