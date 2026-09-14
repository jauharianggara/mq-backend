//! Middleware auth & RBAC (modul 1 + 3).
//! CurrentUser = extractor JWT (HS256, exp 15m) + status + permissions.
//! require_permission = helper eksplisit (DILARANG `if role ==` — audit kontrak).
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use std::collections::HashSet;

use crate::modules::auth::token::decode_access;
use crate::shared::error::AppError;
use crate::state::AppState;

#[derive(Debug, Clone)]
pub struct CurrentUser {
    pub user_id: i64,
    pub session_id: i64,
    pub status: String,
    pub permissions: HashSet<String>,
}

impl CurrentUser {
    /// Gate akun ACTIVE sebelum aksi interaksi (keputusan v3).
    pub fn require_active(&self) -> Result<(), AppError> {
        if self.status != "ACTIVE" {
            return Err(AppError::Forbidden(format!(
                "akun berstatus {} — verifikasi/aktivasi dulu",
                self.status
            )));
        }
        Ok(())
    }

    /// Permission gate — middleware `require_permission` (docs/permissions.md).
    pub fn require(&self, perm: &str) -> Result<(), AppError> {
        self.require_active()?;
        if self.permissions.contains(perm) {
            Ok(())
        } else {
            Err(AppError::Forbidden(format!("butuh permission {perm}")))
        }
    }
}

/// Extractor opsional utk endpoint publik yang ingin tahu siapa pemanggil (mis. learning).
/// Token hilang/invalid => None (anonymous); tidak pernah 401.
#[derive(Debug, Clone)]
pub struct OptionalUser(pub Option<CurrentUser>);

impl FromRequestParts<AppState> for OptionalUser {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "));
        let Some(token) = token else { return Ok(OptionalUser(None)) };
        let Ok(claims) = decode_access(&state.jwt_secret, token) else {
            return Ok(OptionalUser(None));
        };
        let cu = try_current_user(state, claims).await.ok();
        Ok(OptionalUser(cu))
    }
}

async fn try_current_user(state: &AppState, claims: crate::modules::auth::token::Claims) -> Result<CurrentUser, AppError> {
    let (status,): (String,) = sqlx::query_as("SELECT status FROM users WHERE id = ?")
        .bind(claims.sub)
        .fetch_one(&state.pool)
        .await
        .map_err(|_| AppError::Unauthorized("akun tidak ditemukan".into()))?;
    if status == "DELETED" {
        return Err(AppError::Unauthorized("akun sudah dihapus".into()));
    }
    let perms: Vec<(String,)> = sqlx::query_as(
        "SELECT DISTINCT p.code FROM user_roles ur          JOIN role_permissions rp ON rp.role_id = ur.role_id          JOIN permissions p ON p.id = rp.permission_id WHERE ur.user_id = ?")
        .bind(claims.sub)
        .fetch_all(&state.pool)
        .await
        .map_err(|e| { tracing::error!("rbac query: {e}"); AppError::Internal("rbac".into()) })?;
    Ok(CurrentUser {
        user_id: claims.sub, session_id: claims.sid, status,
        permissions: perms.into_iter().map(|p| p.0).collect(),
    })
}

impl FromRequestParts<AppState> for CurrentUser {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let header = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or_else(|| AppError::Unauthorized("token tidak ada (Bearer)".into()))?;

        let claims = decode_access(&state.jwt_secret, header)
            .map_err(|_| AppError::Unauthorized("token tidak valid / kadaluarsa".into()))?;

        // status + permissions (MVP: query per request — cache moka menyusul; catatan trade-off
        // di plan: revokesi sesi berjalan lewat exp 15m + refresh rotation)
        let (status,): (String,) = sqlx::query_as("SELECT status FROM users WHERE id = ?")
            .bind(claims.sub)
            .fetch_one(&state.pool)
            .await
            .map_err(|_| AppError::Unauthorized("akun tidak ditemukan".into()))?;
        if status == "DELETED" {
            return Err(AppError::Unauthorized("akun sudah dihapus".into()));
        }
        let perms: Vec<(String,)> = sqlx::query_as(
            "SELECT DISTINCT p.code FROM user_roles ur \
             JOIN role_permissions rp ON rp.role_id = ur.role_id \
             JOIN permissions p ON p.id = rp.permission_id WHERE ur.user_id = ?")
            .bind(claims.sub)
            .fetch_all(&state.pool)
            .await
            .map_err(|e| {
                tracing::error!("rbac query: {e}");
                AppError::Internal("rbac".into())
            })?;

        Ok(CurrentUser {
            user_id: claims.sub,
            session_id: claims.sid,
            status,
            permissions: perms.into_iter().map(|p| p.0).collect(),
        })
    }
}
