// educlaw-server/src/auth.rs
//
// JWT authentication and Argon2id password helpers.
//
// JWT payload (Claims):
//   sub (student_id) + role + cohort_id + subject_ids + optional session_id

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

use crate::error::AppError;

// ── Auth config ───────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct AuthConfig {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
}

impl AuthConfig {
    pub fn new(secret: String) -> Self {
        Self {
            encoding_key: EncodingKey::from_secret(secret.as_bytes()),
            decoding_key: DecodingKey::from_secret(secret.as_bytes()),
        }
    }

    /// Short-lived admin token (24 h) for internal tooling.
    pub fn sign_admin_token(&self) -> anyhow::Result<String> {
        let claims = Claims {
            sub: "admin".to_string(),
            role: "admin".to_string(),
            cohort_id: String::new(),
            subject_ids: vec![],
            session_id: None,
            exp: (Utc::now() + Duration::hours(24)).timestamp() as usize,
        };
        Ok(encode(&Header::default(), &claims, &self.encoding_key)?)
    }

    /// Student login token (8 h) — carries role + cohort_id + enrolled subject_ids.
    pub fn sign_student_token(
        &self,
        student_id: &str,
        role: &str,
        cohort_id: &str,
        subject_ids: Vec<String>,
    ) -> anyhow::Result<String> {
        let claims = Claims {
            sub: student_id.to_string(),
            role: role.to_string(),
            cohort_id: cohort_id.to_string(),
            subject_ids,
            session_id: None,
            exp: (Utc::now() + Duration::hours(8)).timestamp() as usize,
        };
        Ok(encode(&Header::default(), &claims, &self.encoding_key)?)
    }

    /// Session-scoped token (8 h) for WebSocket connection auth.
    pub fn sign_session_token(
        &self,
        student_id: &str,
        role: &str,
        cohort_id: &str,
        subject_ids: Vec<String>,
        session_id: &str,
    ) -> anyhow::Result<String> {
        let claims = Claims {
            sub: student_id.to_string(),
            role: role.to_string(),
            cohort_id: cohort_id.to_string(),
            subject_ids,
            session_id: Some(session_id.to_string()),
            exp: (Utc::now() + Duration::hours(8)).timestamp() as usize,
        };
        Ok(encode(&Header::default(), &claims, &self.encoding_key)?)
    }

    pub fn verify(&self, token: &str) -> crate::error::Result<Claims> {
        decode::<Claims>(token, &self.decoding_key, &Validation::default())
            .map(|data| data.claims)
            .map_err(|_| AppError::Unauthorized)
    }
}

// ── JWT claims ────────────────────────────────────────────────────────────

/// JWT payload carried by every token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// student_id UUID or "admin"
    pub sub: String,
    /// "student" | "teacher" | "admin"
    pub role: String,
    /// Cohort the student belongs to; empty string for admin tokens.
    pub cohort_id: String,
    /// Subject UUIDs the cohort is enrolled in; empty for admin tokens.
    pub subject_ids: Vec<String>,
    /// Present only on session-scoped tokens.
    pub session_id: Option<String>,
    /// Unix timestamp expiry.
    pub exp: usize,
}

// ── Helpers ───────────────────────────────────────────────────────────────

/// Extract a Bearer token from the Authorization header.
pub fn bearer_token(headers: &axum::http::HeaderMap) -> Option<&str> {
    headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
}

// ── Argon2id ──────────────────────────────────────────────────────────────

/// Hash a plaintext password with Argon2id + random salt.
/// Returns a PHC string suitable for storage.
pub fn hash_password(password: &str) -> Result<String, AppError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| AppError::Internal(anyhow::anyhow!("password hash failed: {e}")))
}

/// Verify a plaintext password against a stored Argon2id PHC hash.
pub fn verify_password(password: &str, hash: &str) -> Result<bool, AppError> {
    let parsed = PasswordHash::new(hash)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("invalid hash: {e}")))?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}
