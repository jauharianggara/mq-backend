use axum::routing::get;
use axum::Json;
use mq_backend_lib::modules::{auth, media, users};
use mq_backend_lib::state::AppState;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "mq_backend=debug,tower_http=info".into()),
        )
        .init();

    let cfg = mq_backend_lib::config::AppConfig::from_env();
    let addr = cfg.listen_addr;
    let database_url = cfg.database_url.clone().unwrap_or_else(|| panic!("DATABASE_URL wajib"));
    let pool = mq_backend_lib::infrastructure::database::build_pool(&database_url).await;
    let state = AppState::new(pool, &cfg).await;

    tracing::info!("MQ backend starting on http://{addr}");

    let app = axum::Router::new()
        .route("/healthz", get(healthz))
        .nest("/api/v1", auth::routes().merge(users::routes()).merge(media::routes()))
        .fallback(not_found)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| panic!("gagal bind {addr}: {e}"));
    axum::serve(listener, app).await.expect("server error");
}

async fn healthz() -> Json<serde_json::Value> {
    mq_backend_lib::shared::response::ok(serde_json::json!({ "status": "ok" }))
}

async fn not_found() -> (axum::http::StatusCode, Json<serde_json::Value>) {
    (
        axum::http::StatusCode::NOT_FOUND,
        Json(serde_json::json!({ "code": "not_found", "message": "endpoint tidak ada" })),
    )
}
