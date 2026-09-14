use axum::middleware as axmw;
use axum::routing::get;
use axum::Json;
use mq_backend_lib::modules::{admin, auth, cms, home, khatmil, learning, media, memorization, notifications, questions, quran, ustadz, users};
use mq_backend_lib::state::AppState;
use tower_http::cors::CorsLayer;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::trace::TraceLayer;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "mq_backend=info,tower_http=info".into()),
        )
        .init();

    let cfg = mq_backend_lib::config::AppConfig::from_env();
    let addr = cfg.listen_addr;
    let database_url = cfg.database_url.clone().unwrap_or_else(|| panic!("DATABASE_URL wajib"));
    let pool = mq_backend_lib::infrastructure::database::build_pool(&database_url).await;
    let state = AppState::new(pool, &cfg).await;

    tracing::info!("MQ backend starting on http://{addr}");

    // CORS ketat: hanya origin yang diizinkan (Phase 5.1)
    let cors = CorsLayer::new()
        .allow_origin("https://mq-admin.jagodigital.online".parse::<axum::http::HeaderValue>().unwrap())
        .allow_origin("http://localhost:3210".parse::<axum::http::HeaderValue>().unwrap())
        .allow_origin("http://127.0.0.1:3210".parse::<axum::http::HeaderValue>().unwrap())
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::PUT,
            axum::http::Method::PATCH,
            axum::http::Method::DELETE,
        ])
        .allow_headers([
            axum::http::header::AUTHORIZATION,
            axum::http::header::CONTENT_TYPE,
            axum::http::header::HeaderName::from_static("idempotency-key"),
        ]);

    let app = axum::Router::new()
        .route("/healthz", get(healthz))
        .nest("/api/v1",
            auth::routes()
            .merge(users::routes())
            .merge(media::routes())
            .merge(quran::routes())
            .merge(learning::routes())
            .merge(ustadz::routes())
            .merge(memorization::routes())
            .merge(khatmil::routes())
            .merge(questions::routes())
            .merge(notifications::routes())
            .merge(cms::routes())
            .merge(cms::service::admin_routes())
            .merge(admin::routes()).merge(home::routes())
        )
        .fallback(not_found)
        .layer(axmw::from_fn_with_state(state.clone(), audit_mw))
        .layer(RequestBodyLimitLayer::new(50 * 1024 * 1024))
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| panic!("gagal bind {addr}: {e}"));
    axum::serve(listener, app).await.expect("server error");
}

async fn audit_mw(
    axum::extract::State(st): axum::extract::State<AppState>,
    req: axum::extract::Request,
    next: axmw::Next,
) -> axum::response::Response {
    let method = req.method().as_str().to_owned();
    let path = req.uri().path().to_string();
    let resp = next.run(req).await;
    if method != "GET" && !path.starts_with("/healthz") {
        let pool = st.pool.clone();
        let action = method;
        let module = path.split('/').nth(3).unwrap_or("unknown").to_string();
        let status = resp.status().as_u16().to_string();
        tokio::spawn(async move {
            let _ = sqlx::query(
                "INSERT INTO audit_logs (action, module, entity_type, entity_id, new_value) \
                 VALUES (?, ?, 'http_request', ?, CAST(? AS JSON))")
                .bind(&action).bind(&module).bind(&path)
                .bind(format!("{{\"status\":{status}}}"))
                .execute(&pool).await;
        });
    }
    resp
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
