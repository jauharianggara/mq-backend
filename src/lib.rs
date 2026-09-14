//! Library crate MQ backend (dipakai bins + integration test).
pub mod config;
pub mod infrastructure;
pub mod middleware;
pub mod modules;
pub mod shared;
pub mod state;

pub fn config_for_smoke() -> crate::config::AppConfig {
    crate::config::AppConfig::from_env()
}
pub async fn storage_for_smoke(cfg: &crate::config::AppConfig) -> crate::infrastructure::storage::Storage {
    crate::infrastructure::storage::build(cfg.s3.as_ref().expect("S3_* wajib")).await
}
pub async fn pool_for_smoke(cfg: &crate::config::AppConfig) -> sqlx::MySqlPool {
    crate::infrastructure::database::build_pool(cfg.database_url.as_ref().expect("DATABASE_URL")).await
}
