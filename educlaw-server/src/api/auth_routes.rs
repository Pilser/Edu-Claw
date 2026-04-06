// educlaw-server/src/api/auth_routes.rs
//
// POST /auth/login   — student login with username + password → JWT
// POST /auth/token   — dev admin token (guarded by ADMIN_SECRET env var)

use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::post, Json, Router};
use serde::Deserialize;

use crate::auth;
use crate::db::{students, subjects};
use crate::error::{AppError, Result};

use super::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/auth/login", post(student_login))
        .route("/auth/token", post(dev_admin_token))
}

// ── Student login ─────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

async fn student_login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> Result<impl IntoResponse> {
    let student = students::get_student_by_username(&state.pool, &body.username)
        .await?
        .ok_or_else(|| {
            tracing::warn!(username = %body.username, "login: unknown username");
            AppError::Unauthorized
        })?;

    // Constant-time Argon2id verification
    if !auth::verify_password(&body.password, &student.password_hash)? {
        tracing::warn!(username = %body.username, student_id = %student.id, "login: incorrect password");
        return Err(AppError::Unauthorized);
    }

    // Resolve subject slugs for this cohort so they go into the JWT.
    let subject_ids: Vec<String> = if student.cohort_id.is_empty() {
        vec![]
    } else {
        subjects::list_subjects(&state.pool, &student.cohort_id)
            .await?
            .into_iter()
            .map(|s| s.slug)
            .collect()
    };

    let token = state.auth.sign_student_token(
        &student.id,
        &student.role,
        &student.cohort_id,
        subject_ids.clone(),
    )?;

    // Update last_seen (non-fatal)
    let _ = students::touch_last_seen(&state.pool, &student.id).await;

    tracing::info!(username = %body.username, student_id = %student.id, "student logged in");

    Ok(Json(serde_json::json!({
        "token":        token,
        "student_id":   student.id,
        "display_name": student.display_name,
        "role":         student.role,
        "cohort_id":    student.cohort_id,
        "subject_ids":  subject_ids,
    })))
}

// ── Dev admin token ───────────────────────────────────────────────────────

#[derive(Deserialize)]
struct DevTokenRequest {
    secret: String,
}

async fn dev_admin_token(
    State(state): State<AppState>,
    Json(body): Json<DevTokenRequest>,
) -> Result<impl IntoResponse> {
    let expected =
        std::env::var("ADMIN_SECRET").unwrap_or_else(|_| "dev-secret".to_string());
    if body.secret != expected {
        return Err(AppError::Unauthorized);
    }
    let token = state.auth.sign_admin_token()?;
    Ok((StatusCode::OK, Json(serde_json::json!({ "token": token }))))
}
