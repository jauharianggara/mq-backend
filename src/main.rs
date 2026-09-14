mod config;
mod shared;

use axum::{routing::get, Router};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "mq_backend=debug,tower_http=info".into()),
        )
        .init();

    let cfg = config::AppConfig::from_env();
    let addr = cfg.listen_addr;
    tracing::info!("MQ backend starting on http://{addr}");

    let app = Router::new().route("/healthz", get(healthz));

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| panic!("gagal bind {addr}: {e}"));
    axum::serve(listener, app)
        .await
        .expect("server error");
}

async fn healthz() -> axum::Json<serde_json::Value> {
    shared::response::ok(serde_json::json!({ "status": "ok" }))
}
