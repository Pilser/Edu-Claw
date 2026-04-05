// educlaw-server/src/api/admin.rs
//
// Admin REST API — Phase 5.1
//
// All routes require a JWT with role == "admin".
// Teacher-read routes additionally allow role == "teacher".
//
// Endpoints (plan §11.1):
//   GET    /admin/students              — list all students + active-session flag
//   POST   /admin/students              — create student account
//   PUT    /admin/students/:id          — update class enrolments / reset password
//   GET    /admin/sessions              — live sessions with idle_secs
//   DELETE /admin/sessions/:id          — force-terminate a session
//   POST   /admin/classes               — create a class directory on disk

use std::path::Path;

use axum::{
    extract::{Path as AxumPath, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::{bearer_token, hash_password, Claims};
use crate::db::{sessions as db_sess, students};
use crate::error::{AppError, Result};
use crate::session::broker::now_secs;

use super::AppState;

// ── Router ────────────────────────────────────────────────────────────────

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/admin/students",     get(list_students).post(create_student))
        .route("/admin/students/{id}", put(update_student))
        .route("/admin/sessions",     get(list_sessions))
        .route("/admin/sessions/{id}", delete(terminate_session))
        .route("/admin/classes",      post(create_class))
}

// ── Auth guard ────────────────────────────────────────────────────────────

/// Extract and validate a Bearer JWT; returns Err(Unauthorized) if missing or
/// invalid. Callers then check the role field themselves.
fn claims(state: &AppState, headers: &HeaderMap) -> Result<Claims> {
    let token = bearer_token(headers).ok_or(AppError::Unauthorized)?;
    state.auth.verify(token)
}

fn require_admin(state: &AppState, headers: &HeaderMap) -> Result<Claims> {
    let c = claims(state, headers)?;
    if c.role != "admin" {
        return Err(AppError::Forbidden);
    }
    Ok(c)
}

/// Admin or teacher — used for read-only operations.
fn require_staff(state: &AppState, headers: &HeaderMap) -> Result<Claims> {
    let c = claims(state, headers)?;
    if c.role != "admin" && c.role != "teacher" {
        return Err(AppError::Forbidden);
    }
    Ok(c)
}

// ── GET /admin/students ───────────────────────────────────────────────────

#[derive(Serialize)]
struct StudentView {
    id: String,
    username: String,
    display_name: String,
    role: String,
    class_ids: Vec<String>,
    last_seen: Option<String>,
    has_active_session: bool,
}

async fn list_students(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse> {
    require_staff(&state, &headers)?;

    let rows = students::list_students(&state.pool).await?;

    // Collect session IDs that are live in the broker so we can mark active.
    let live: std::collections::HashSet<String> = state
        .broker
        .list_entries()
        .await
        .into_iter()
        .map(|(sid, _)| sid)
        .collect();

    // We need student_ids that have a live session.  Fetch active sessions once.
    let active_sessions = db_sess::list_active_sessions(&state.pool)
        .await?;
    let active_student_ids: std::collections::HashSet<String> = active_sessions
        .iter()
        .filter(|s| live.contains(&s.id))
        .map(|s| s.student_id.clone())
        .collect();

    let views: Vec<StudentView> = rows
        .into_iter()
        .map(|s| {
            let class_ids: Vec<String> =
                serde_json::from_str(&s.class_ids).unwrap_or_default();
            StudentView {
                has_active_session: active_student_ids.contains(&s.id),
                id: s.id,
                username: s.username,
                display_name: s.display_name,
                role: s.role,
                class_ids,
                last_seen: s.last_seen,
            }
        })
        .collect();

    Ok(Json(views))
}

// ── POST /admin/students ──────────────────────────────────────────────────

#[derive(Deserialize)]
struct CreateStudentBody {
    username: String,
    password: String,
    display_name: String,
    role: Option<String>,
    class_ids: Option<Vec<String>>,
}

async fn create_student(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateStudentBody>,
) -> Result<impl IntoResponse> {
    require_admin(&state, &headers)?;

    let password_hash = hash_password(&body.password)?;
    let class_ids_json = serde_json::to_string(&body.class_ids.unwrap_or_default())
        .map_err(|e| AppError::Internal(e.into()))?;

    let student = students::Student {
        id: Uuid::new_v4().to_string(),
        username: body.username,
        password_hash,
        display_name: body.display_name,
        role: body.role.unwrap_or_else(|| "student".to_string()),
        class_ids: class_ids_json,
        uid_slot: None,
        created_at: String::new(),
        last_seen: None,
    };

    students::insert_student(&state.pool, &student)
        .await?;

    tracing::info!(student_id = %student.id, username = %student.username, "admin: student created");
    Ok((StatusCode::CREATED, Json(serde_json::json!({ "id": student.id }))))
}

// ── PUT /admin/students/:id ───────────────────────────────────────────────

#[derive(Deserialize)]
struct UpdateStudentBody {
    display_name: Option<String>,
    class_ids: Option<Vec<String>>,
    /// If present, password is reset to this value.
    new_password: Option<String>,
}

async fn update_student(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Json(body): Json<UpdateStudentBody>,
) -> Result<impl IntoResponse> {
    require_admin(&state, &headers)?;

    // Verify student exists.
    students::get_student(&state.pool, &id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("student {id}")))?;

    let class_ids_json = body
        .class_ids
        .as_ref()
        .map(|v| serde_json::to_string(v).unwrap_or_default());

    let new_hash = body
        .new_password
        .as_deref()
        .map(hash_password)
        .transpose()?;

    students::update_student(
        &state.pool,
        &id,
        body.display_name.as_deref(),
        class_ids_json.as_deref(),
        new_hash.as_deref(),
    )
    .await?;

    tracing::info!(student_id = %id, "admin: student updated");
    Ok(StatusCode::NO_CONTENT)
}

// ── GET /admin/sessions ───────────────────────────────────────────────────

#[derive(Serialize)]
struct SessionView {
    id: String,
    student_id: String,
    course: String,
    assignment: Option<String>,
    state: String,
    namespace_pid: Option<i64>,
    idle_secs: Option<u64>,
    started_at: String,
}

async fn list_sessions(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse> {
    require_staff(&state, &headers)?;

    let db_sessions = db_sess::list_active_sessions(&state.pool)
        .await?;

    // Build a map of session_id → last_input_at from the in-memory broker.
    let live: std::collections::HashMap<String, u64> =
        state.broker.list_entries().await.into_iter().collect();

    let now = now_secs();
    let views: Vec<SessionView> = db_sessions
        .into_iter()
        .map(|s| {
            let idle_secs = live.get(&s.id).map(|&last| now.saturating_sub(last));
            SessionView {
                idle_secs,
                id: s.id,
                student_id: s.student_id,
                course: s.course,
                assignment: s.assignment,
                state: s.state,
                namespace_pid: s.namespace_pid,
                started_at: s.started_at,
            }
        })
        .collect();

    Ok(Json(views))
}

// ── DELETE /admin/sessions/:id ────────────────────────────────────────────

async fn terminate_session(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
) -> Result<impl IntoResponse> {
    require_admin(&state, &headers)?;

    let session = db_sess::get_session(&state.pool, &id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("session {id}")))?;

    // Kill namespace process if PID is known.
    if let Some(pid) = session.namespace_pid {
        let _ = tokio::process::Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .output()
            .await;
    }

    // Unmount OverlayFS.
    crate::session::namespace::umount_overlay(std::path::Path::new(&session.overlay_mount)).await;

    // Update DB state.
    db_sess::set_session_state(&state.pool, &id, "ended")
        .await?;

    // Remove from broker.
    state.broker.remove_entry(&id).await;
    crate::security::cgroups::teardown_session_cgroup(&id).await;

    tracing::info!(session_id = %id, "admin: session force-terminated");
    Ok(StatusCode::NO_CONTENT)
}

// ── POST /admin/classes ───────────────────────────────────────────────────

#[derive(Deserialize)]
struct CreateClassBody {
    /// Short slug used as the directory name, e.g. "math101".
    class_id: String,
    /// Optional human-readable class title stored in a metadata file.
    title: Option<String>,
}

async fn create_class(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateClassBody>,
) -> Result<impl IntoResponse> {
    require_admin(&state, &headers)?;

    // Validate slug: alphanumeric + dashes/underscores only.
    if body.class_id.is_empty()
        || !body.class_id.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return Err(AppError::BadRequest("class_id must be alphanumeric (-, _ allowed)".into()));
    }

    // Derive the class directory from workspace_root.
    // Convention: {workspace_root}/../../classes/{class_id}
    // But the actual root comes from EDUCLAW_CLASSES_ROOT env, falling back to sibling.
    let classes_root = std::env::var("EDUCLAW_CLASSES_ROOT")
        .unwrap_or_else(|_| "/srv/educlaw/data/classes".to_string());
    let class_dir = Path::new(&classes_root).join(&body.class_id);

    if class_dir.exists() {
        return Err(AppError::BadRequest(format!("class '{}' already exists", body.class_id)));
    }

    tokio::fs::create_dir_all(&class_dir)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("create class dir: {e}")))?;

    // Write a minimal class.json metadata file.
    if let Some(title) = &body.title {
        let meta = serde_json::json!({ "class_id": body.class_id, "title": title });
        let meta_path = class_dir.join("class.json");
        tokio::fs::write(&meta_path, meta.to_string())
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("write class.json: {e}")))?;
    }

    tracing::info!(class_id = %body.class_id, "admin: class created");
    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "class_id": body.class_id,
            "path": class_dir.display().to_string()
        })),
    ))
}
