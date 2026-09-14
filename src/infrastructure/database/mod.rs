//! Database pool (MySQL 8) — UTC per koneksi (gotcha plan).
use sqlx::mysql::{MySqlConnectOptions, MySqlPool, MySqlPoolOptions};

pub async fn build_pool(database_url: &str) -> MySqlPool {
    let opts: MySqlConnectOptions = database_url
        .parse()
        .unwrap_or_else(|e| panic!("DATABASE_URL tidak valid: {e}"));
    let opts = opts.timezone(Some("+00:00".to_string()));
    MySqlPoolOptions::new()
        .max_connections(10)
        .connect_with(opts)
        .await
        .unwrap_or_else(|e| panic!("gagal connect DB: {e}"))
}
