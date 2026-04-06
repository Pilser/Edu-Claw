// educlaw-server/src/api/students.rs
//
// POST /students        — create student (admin)
// GET  /students        — list all students (admin)
// GET  /students/:id    — get one student by id (admin)

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::auth;
use crate::db::students::{self, Student};
use crate::error::{AppError, Result};

use super::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/students", post(create_student))
        .route("/students", get(list_students))
        .route("/students/:id", get(get_student))
}

// ── Create student ────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct CreateStudentRequest {
    username: String,
    password: String,
    display_name: String,
    /// "student" | "teacher" | "admin" — defaults to "student"
    role: Option<String>,
    /// Cohort this student belongs to
    cohort_id: Option<String>,
}

async fn create_student(
    State(state): State<AppState>,
    Json(body): Json<CreateStudentRequest>,
) -> Result<impl IntoResponse> {
    let password_hash = auth::hash_password(&body.password)?;

    let student = Student {
        id: Uuid::new_v4().to_string(),
        username: body.username,
        password_hash,
        display_name: body.display_name,
        role: body.role.unwrap_or_else(|| "student".to_string()),
        cohort_id: body.cohort_id.unwrap_or_default(),
        uid_slot: None,
        created_at: String::new(), // set by DB DEFAULT
        last_seen: None,
    };

    students::insert_student(&state.pool, &student).await?;

    tracing::info!(
        student_id = %student.id,
        username   = %student.username,
        "student created"
    );

    // password_hash is skipped via #[serde(skip_serializing)]
    Ok((StatusCode::CREATED, Json(student)))
}

// ── List students ─────────────────────────────────────────────────────────

async fn list_students(State(state): State<AppState>) -> Result<impl IntoResponse> {
    let all = students::list_students(&state.pool).await?;
    Ok(Json(all))
}

// ── Get student ───────────────────────────────────────────────────────────

async fn get_student(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    let student = students::get_student(&state.pool, &id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("student {id} not found")))?;
    Ok(Json(student))
}
