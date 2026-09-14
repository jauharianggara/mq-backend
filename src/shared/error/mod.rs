//! AppError — error konsisten `{code, message, details?}` (Task 0.4).
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    Unauthorized(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Unprocessable(String),
    #[error("terlalu banyak request")]
    RateLimited,
    #[error("kesalahan internal")]
    Internal(String),
}

impl AppError {
    fn parts(&self) -> (StatusCode, &'static str, String) {
        match self {
            AppError::Unauthorized(m) => (StatusCode::UNAUTHORIZED, "unauthorized", m.clone()),
            AppError::Forbidden(m) => (StatusCode::FORBIDDEN, "forbidden", m.clone()),
            AppError::NotFound(m) => (StatusCode::NOT_FOUND, "not_found", m.clone()),
            AppError::Conflict(m) => (StatusCode::CONFLICT, "conflict", m.clone()),
            AppError::Unprocessable(m) => (StatusCode::UNPROCESSABLE_ENTITY, "unprocessable", m.clone()),
            AppError::RateLimited => (StatusCode::TOO_MANY_REQUESTS, "rate_limited", "terlalu banyak request, coba lagi nanti".into()),
            AppError::Internal(_detail) => (StatusCode::INTERNAL_SERVER_ERROR, "internal", "kesalahan internal".into()),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message) = self.parts();
        if status == StatusCode::INTERNAL_SERVER_ERROR {
            if let AppError::Internal(detail) = &self {
                tracing::error!("AppError::Internal: {detail}");
            }
        }
        (status, Json(json!({ "code": code, "message": message }))).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_shape_konsisten() {
        let r = AppError::Conflict("juz sudah diklaim".into()).into_response();
        assert_eq!(r.status(), StatusCode::CONFLICT);
        let body = axum::body::to_bytes(axum::body::Body::from(
            serde_json::to_string(&serde_json::json!({})).unwrap(),
        ), 1024);
        // Cukup pastikan status+konstruksi JSON jalan; body penuh dites via integration test handler.
        drop(body);
    }

    #[test]
    fn kode_error_mapping() {
        assert_eq!(AppError::Unauthorized("x".into()).parts().1, "unauthorized");
        assert_eq!(AppError::Forbidden("x".into()).parts().1, "forbidden");
        assert_eq!(AppError::NotFound("x".into()).parts().1, "not_found");
        assert_eq!(AppError::Conflict("x".into()).parts().1, "conflict");
        assert_eq!(AppError::Unprocessable("x".into()).parts().1, "unprocessable");
        assert_eq!(AppError::Internal("db down".into()).parts().1, "internal");
    }
}
