//! Service auth — register/login/refresh/logout/verify/reset (modul 1 Phase 3).
//! Semua query runtime-bound (tanpa macro — .sqlx offline menyusul bersamaan modul berikut).
use sqlx::MySqlPool;
use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};

use crate::modules::auth::dto::*;
use crate::modules::auth::token::*;
use crate::shared::error::AppError;

fn hash_password(pw: &str) -> String {
    let salt = SaltString::encode_b64(uuid::Uuid::new_v4().as_bytes()).expect("salt");
    Argon2::default().hash_password(pw.as_bytes(), &salt).expect("argon2").to_string()
}

fn verify_password(hash: &str, pw: &str) -> bool {
    PasswordHash::new(hash)
        .map(|parsed| Argon2::default().verify_password(pw.as_bytes(), &parsed).is_ok())
        .unwrap_or(false)
}

fn dberr(e: sqlx::Error) -> AppError {
    tracing::error!("db: {e}");
    AppError::Internal(format!("db: {e}"))
}

async fn user_public(pool: &MySqlPool, user_id: i64) -> Result<UserPublic, AppError> {
    let (phone, email, account_type, status): (Option<String>, Option<String>, String, String) =
        sqlx::query_as("SELECT phone, email, account_type, status FROM users WHERE id = ?")
            .bind(user_id).fetch_one(pool).await.map_err(dberr)?;
    let roles: Vec<(String,)> =
        sqlx::query_as("SELECT r.code FROM user_roles ur JOIN roles r ON r.id = ur.role_id WHERE ur.user_id = ?")
            .bind(user_id).fetch_all(pool).await.map_err(dberr)?;
    let full_name: Option<(Option<String>,)> =
        sqlx::query_as("SELECT full_name FROM user_profiles WHERE user_id = ?")
            .bind(user_id).fetch_optional(pool).await.map_err(dberr)?;
    Ok(UserPublic {
        id: user_id, phone, email, account_type, status,
        roles: roles.into_iter().map(|r| r.0).collect(),
        full_name: full_name.and_then(|f| f.0),
        photo_url: None,
    })
}

pub async fn register(pool: &MySqlPool, dev_expose: bool, req: RegisterReq) -> Result<RegisterResp, AppError> {
    if !req.consent {
        return Err(AppError::Unprocessable("consent wajib true (UU PDP)".into()));
    }
    if req.password.len() < 8 {
        return Err(AppError::Unprocessable("password minimal 8 karakter".into()));
    }
    let email = req.email.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let phone = req.phone.as_deref().map(str::trim).filter(|s| !s.is_empty());
    if email.is_none() && phone.is_none() {
        return Err(AppError::Unprocessable("email atau phone wajib salah satu".into()));
    }

    // duplikat -> 409
    if let Some(e) = email {
        let n: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE email = ?").bind(e)
            .fetch_one(pool).await.map_err(dberr)?;
        if n.0 > 0 { return Err(AppError::Conflict("email sudah terdaftar".into())); }
    }
    if let Some(p) = phone {
        let n: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE phone = ?").bind(p)
            .fetch_one(pool).await.map_err(dberr)?;
        if n.0 > 0 { return Err(AppError::Conflict("nomor phone sudah terdaftar".into())); }
    }

    let hash = hash_password(&req.password);
    let result = sqlx::query(
        "INSERT INTO users (phone, email, password_hash, account_type, status) VALUES (?, ?, ?, 'SANTRI', 'PENDING_VERIFICATION')")
        .bind(phone).bind(email).bind(&hash)
        .execute(pool).await.map_err(dberr)?;
    let uid = result.last_insert_id() as i64;

    if let Some(name) = req.full_name.as_deref().filter(|s| !s.trim().is_empty()) {
        sqlx::query("INSERT IGNORE INTO user_profiles (user_id, full_name) VALUES (?, ?)")
            .bind(uid).bind(name.trim()).execute(pool).await.map_err(dberr)?;
    }
    // role default SANTRI
    sqlx::query("INSERT IGNORE INTO user_roles (user_id, role_id) SELECT ?, id FROM roles WHERE code = 'SANTRI'")
        .bind(uid).execute(pool).await.map_err(dberr)?;

    // consent -> activity_events (keputusan #12 PDP)
    sqlx::query("INSERT INTO activity_events (user_id, event_type, ref_type, ref_id, payload) VALUES (?, 'auth.registered', 'user', ?, CAST('{\"consent\":true,\"terms\":1}' AS JSON))")
        .bind(uid).bind(uid.to_string()).execute(pool).await.map_err(dberr)?;

    // token verifikasi email (resend invalidates prior)
    let raw = generate_opaque_token();
    create_action_token(pool, uid, "EMAIL_VERIFY", &raw, 24).await?;

    Ok(RegisterResp {
        user_id: uid,
        status: "PENDING_VERIFICATION".into(),
        dev_verification_token: dev_expose.then_some(raw),
    })
}

async fn create_action_token(pool: &MySqlPool, user_id: i64, purpose: &str, raw: &str, hours: i64) -> Result<(), AppError> {
    sqlx::query("UPDATE auth_action_tokens SET used_at = UTC_TIMESTAMP() WHERE user_id = ? AND purpose = ? AND used_at IS NULL")
        .bind(user_id).bind(purpose).execute(pool).await.map_err(dberr)?;
    sqlx::query("INSERT INTO auth_action_tokens (user_id, purpose, token_hash, expires_at) VALUES (?, ?, ?, DATE_ADD(UTC_TIMESTAMP(), INTERVAL ? HOUR))")
        .bind(user_id).bind(purpose).bind(sha256_hex(raw)).bind(hours)
        .execute(pool).await.map_err(dberr)?;
    Ok(())
}

async fn consume_action_token(pool: &MySqlPool, raw: &str) -> Result<(i64, String), AppError> {
    let row: Option<(i64, String)> = sqlx::query_as(
        "SELECT user_id, purpose FROM auth_action_tokens WHERE token_hash = ? AND used_at IS NULL AND expires_at > UTC_TIMESTAMP()")
        .bind(sha256_hex(raw)).fetch_optional(pool).await.map_err(dberr)?;
    match row {
        Some((uid, purpose)) => {
            sqlx::query("UPDATE auth_action_tokens SET used_at = UTC_TIMESTAMP() WHERE token_hash = ?")
                .bind(sha256_hex(raw)).execute(pool).await.map_err(dberr)?;
            Ok((uid, purpose))
        }
        None => Err(AppError::Unprocessable("token tidak valid / kadaluarsa / sudah dipakai".into())),
    }
}

pub async fn verify_email(pool: &MySqlPool, token: &str) -> Result<(), AppError> {
    let (uid, purpose) = consume_action_token(pool, token).await?;
    if purpose != "EMAIL_VERIFY" {
        return Err(AppError::Unprocessable("token bukan untuk verifikasi email".into()));
    }
    // policy: registration_require_verification (default true) -> langsung ACTIVE setelah verifikasi
    sqlx::query("UPDATE users SET email_verified_at = UTC_TIMESTAMP(), status = 'ACTIVE' WHERE id = ? AND status = 'PENDING_VERIFICATION'")
        .bind(uid).execute(pool).await.map_err(dberr)?;
    sqlx::query("INSERT INTO activity_events (user_id, event_type, payload) VALUES (?, 'auth.email_verified', CAST('{}' AS JSON))")
        .bind(uid).execute(pool).await.map_err(dberr)?;
    Ok(())
}

pub async fn login(pool: &MySqlPool, secret: &str, req: LoginReq) -> Result<TokenPair, AppError> {
    let row: Option<(i64, String, String, Option<String>)> = sqlx::query_as(
        "SELECT id, password_hash, status, email FROM users WHERE (email = ? AND ? IS NOT NULL) OR (phone = ? AND ? IS NOT NULL) LIMIT 1")
        .bind(req.email.as_deref()).bind(req.email.as_deref())
        .bind(req.phone.as_deref()).bind(req.phone.as_deref())
        .fetch_optional(pool).await.map_err(dberr)?;
    let (uid, hash, status, email) = match row {
        Some(r) => r,
        None => return Err(AppError::Unauthorized("email/phone atau password salah".into())),
    };
    if !verify_password(&hash, &req.password) {
        return Err(AppError::Unauthorized("email/phone atau password salah".into()));
    }
    if status != "ACTIVE" {
        return Err(AppError::Forbidden(format!("akun berstatus {status} — tidak bisa login")));
    }
    let refresh = generate_opaque_token();
    let ins = sqlx::query("INSERT INTO user_sessions (user_id, refresh_token_hash, expires_at) VALUES (?, ?, ?)")
        .bind(uid).bind(sha256_hex(&refresh)).bind(refresh_expiry())
        .execute(pool).await.map_err(dberr)?;
    let sid = ins.last_insert_id() as i64;
    sqlx::query("UPDATE users SET last_login_at = UTC_TIMESTAMP() WHERE id = ?").bind(uid)
        .execute(pool).await.map_err(dberr)?;
    let user = user_public(pool, uid).await?;
    let _ = email;
    Ok(TokenPair { access_token: encode_access(secret, uid, sid), refresh_token: refresh, user })
}

pub async fn refresh(pool: &MySqlPool, secret: &str, raw_refresh: &str) -> Result<TokenPair, AppError> {
    let row: Option<(i64, i64)> = sqlx::query_as(
        "SELECT id, user_id FROM user_sessions WHERE refresh_token_hash = ? AND revoked_at IS NULL AND expires_at > UTC_TIMESTAMP()")
        .bind(sha256_hex(raw_refresh)).fetch_optional(pool).await.map_err(dberr)?;
    let (sid, uid) = row.ok_or_else(|| AppError::Unauthorized("refresh token tidak valid".into()))?;
    // rotasi
    sqlx::query("UPDATE user_sessions SET revoked_at = UTC_TIMESTAMP() WHERE id = ?").bind(sid)
        .execute(pool).await.map_err(dberr)?;
    let new_refresh = generate_opaque_token();
    let ins = sqlx::query("INSERT INTO user_sessions (user_id, refresh_token_hash, expires_at) VALUES (?, ?, ?)")
        .bind(uid).bind(sha256_hex(&new_refresh)).bind(refresh_expiry())
        .execute(pool).await.map_err(dberr)?;
    let new_sid = ins.last_insert_id() as i64;
    let user = user_public(pool, uid).await?;
    Ok(TokenPair { access_token: encode_access(secret, uid, new_sid), refresh_token: new_refresh, user })
}

pub async fn logout(pool: &MySqlPool, session_id: i64) -> Result<(), AppError> {
    // revoke sesi + nonaktifkan device terkait (kontrak /auth/logout)
    sqlx::query("UPDATE user_sessions SET revoked_at = UTC_TIMESTAMP() WHERE id = ? AND revoked_at IS NULL")
        .bind(session_id).execute(pool).await.map_err(dberr)?;
    sqlx::query("UPDATE user_devices SET is_active = 0 WHERE id = (SELECT device_id FROM user_sessions WHERE id = ?)")
        .bind(session_id).execute(pool).await.map_err(dberr)?;
    Ok(())
}

pub async fn logout_all(pool: &MySqlPool, user_id: i64) -> Result<(), AppError> {
    sqlx::query("UPDATE user_sessions SET revoked_at = UTC_TIMESTAMP() WHERE user_id = ? AND revoked_at IS NULL")
        .bind(user_id).execute(pool).await.map_err(dberr)?;
    sqlx::query("UPDATE user_devices SET is_active = 0 WHERE user_id = ?").bind(user_id)
        .execute(pool).await.map_err(dberr)?;
    Ok(())
}

pub async fn forgot_password(pool: &MySqlPool, dev_expose: bool, req: ForgotReq) -> Result<Option<String>, AppError> {
    let row: Option<(i64,)> = sqlx::query_as(
        "SELECT id FROM users WHERE (email = ? AND ? IS NOT NULL) OR (phone = ? AND ? IS NOT NULL) LIMIT 1")
        .bind(req.email.as_deref()).bind(req.email.as_deref())
        .bind(req.phone.as_deref()).bind(req.phone.as_deref())
        .fetch_optional(pool).await.map_err(dberr)?;
    // anti-enumeration: selalu sukses
    if let Some((uid,)) = row {
        let raw = generate_opaque_token();
        create_action_token(pool, uid, "PASSWORD_RESET", &raw, 1).await?;
        tracing::info!(user_id = uid, "PASSWORD_RESET token dibuat (pengirim email = stub)");
        return Ok(dev_expose.then_some(raw));
    }
    Ok(None)
}

pub async fn reset_password(pool: &MySqlPool, req: ResetReq) -> Result<(), AppError> {
    if req.new_password.len() < 8 {
        return Err(AppError::Unprocessable("password minimal 8 karakter".into()));
    }
    let (uid, purpose) = consume_action_token(pool, &req.token).await?;
    if purpose != "PASSWORD_RESET" {
        return Err(AppError::Unprocessable("token bukan untuk reset password".into()));
    }
    sqlx::query("UPDATE users SET password_hash = ? WHERE id = ?")
        .bind(hash_password(&req.new_password)).bind(uid)
        .execute(pool).await.map_err(dberr)?;
    logout_all(pool, uid).await?;
    Ok(())
}

pub async fn change_password(pool: &MySqlPool, user_id: i64, keep_session: i64, req: ChangePasswordReq) -> Result<(), AppError> {
    if req.new_password.len() < 8 {
        return Err(AppError::Unprocessable("password minimal 8 karakter".into()));
    }
    let (hash,): (String,) = sqlx::query_as("SELECT password_hash FROM users WHERE id = ?")
        .bind(user_id).fetch_one(pool).await.map_err(dberr)?;
    if !verify_password(&hash, &req.old_password) {
        return Err(AppError::Unprocessable("password lama salah".into()));
    }
    sqlx::query("UPDATE users SET password_hash = ? WHERE id = ?")
        .bind(hash_password(&req.new_password)).bind(user_id)
        .execute(pool).await.map_err(dberr)?;
    // revoke sesi lain, keep sesi aktif
    sqlx::query("UPDATE user_sessions SET revoked_at = UTC_TIMESTAMP() WHERE user_id = ? AND id != ? AND revoked_at IS NULL")
        .bind(user_id).bind(keep_session)
        .execute(pool).await.map_err(dberr)?;
    Ok(())
}

pub async fn resend_verification(pool: &MySqlPool, dev_expose: bool, user_id: i64) -> Result<Option<String>, AppError> {
    let (status, email): (String, Option<String>) = sqlx::query_as("SELECT status, email FROM users WHERE id = ?")
        .bind(user_id).fetch_one(pool).await.map_err(dberr)?;
    if status == "ACTIVE" {
        return Err(AppError::Unprocessable("akun sudah terverifikasi".into()));
    }
    if email.is_none() {
        return Err(AppError::Unprocessable("akun tidak memiliki email".into()));
    }
    let raw = generate_opaque_token();
    create_action_token(pool, user_id, "EMAIL_VERIFY", &raw, 24).await?;
    Ok(dev_expose.then_some(raw))
}

pub async fn get_me(state: &crate::state::AppState, user_id: i64) -> Result<UserPublic, AppError> {
    let mut u = user_public(&state.pool, user_id).await?;
    if let Some(url) = crate::modules::media::service::avatar_urls_for(state, &[user_id]).await.get(&user_id) {
        u.photo_url = Some(url.clone());
    }
    Ok(u)
}
