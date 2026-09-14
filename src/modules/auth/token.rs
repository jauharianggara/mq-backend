//! JWT access token + refresh token (rotasi) — modul auth.
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: i64, // user_id
    pub sid: i64, // user_sessions.id
    pub exp: i64,
    pub iat: i64,
}

pub const ACCESS_TOKEN_TTL_SECS: i64 = 15 * 60; // 15 menit
pub const REFRESH_TOKEN_TTL_DAYS: i64 = 30;

pub fn encode_access(secret: &str, user_id: i64, session_id: i64) -> String {
    let now = Utc::now();
    let claims = Claims {
        sub: user_id,
        sid: session_id,
        iat: now.timestamp(),
        exp: (now + Duration::seconds(ACCESS_TOKEN_TTL_SECS)).timestamp(),
    };
    encode(&Header::default(), &claims, &EncodingKey::from_secret(secret.as_bytes()))
        .expect("jwt encode")
}

pub fn decode_access(secret: &str, token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    decode::<Claims>(token, &DecodingKey::from_secret(secret.as_bytes()), &Validation::default())
        .map(|d| d.claims)
}

/// Refresh token mentah (dikirim ke klien) — 64 hex char.
pub fn generate_opaque_token() -> String {
    let a = uuid::Uuid::new_v4().simple().to_string();
    let b = uuid::Uuid::new_v4().simple().to_string();
    format!("{a}{b}")
}

/// SHA-256 hex — untuk storage (refresh & auth_action_tokens). Token mentah TIDAK pernah disimpan.
pub fn sha256_hex(input: &str) -> String {
    let mut h = Sha256::new();
    h.update(input.as_bytes());
    let out = h.finalize();
    let mut s = String::with_capacity(64);
    for b in out {
        use std::fmt::Write;
        let _ = write!(s, "{b:02x}");
    }
    s
}

pub fn refresh_expiry() -> chrono::NaiveDateTime {
    (Utc::now() + Duration::days(REFRESH_TOKEN_TTL_DAYS)).naive_utc()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn access_token_roundtrip() {
        let secret = "0123456789abcdef0123456789abcdef0123456789";
        let t = encode_access(secret, 42, 7);
        let c = decode_access(secret, &t).expect("decode");
        assert_eq!(c.sub, 42);
        assert_eq!(c.sid, 7);
        assert!(c.exp > c.iat);
    }

    #[test]
    fn decode_tolak_secret_salah() {
        let t = encode_access("secret-satu-aaaaaaaaaaaaaaaaaaaaaaaaaa", 1, 1);
        assert!(decode_access("secret-dua-bbbbbbbbbbbbbbbbbbbbbbbbbb", &t).is_err());
    }

    #[test]
    fn sha256_stabil() {
        assert_eq!(sha256_hex("abc").len(), 64);
        assert_eq!(sha256_hex("abc"), sha256_hex("abc"));
    }
}
