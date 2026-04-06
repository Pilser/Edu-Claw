// educlaw-server/src/api/admin.rs
//
// Admin REST API
//
// All routes require JWT role == "admin".
// Read-only routes additionally allow role == "teacher".
//
// Students
//   GET    /admin/students              — list all students + active-session flag
//   POST   /admin/students              — create student account
//   PUT    /admin/students/:id          — update display_name / cohort_id / password
// Sessions
//   GET    /admin/sessions              — live sessions with idle_secs
//   DELETE /admin/sessions/:id          — force-terminate a session
// Cohorts
//   GET    /admin/cohorts               — list all cohorts
//   POST   /admin/cohorts               — create a cohort
// Subjects
//   GET    /admin/subjects?cohort_id=   — list subjects (filter by cohort)
//   POST   /admin/subjects              — create a subject (cohort_id + slug required)
// Assignments
//   GET    /admin/assignments?subject_id=  — list assignments (all, or filter by subject)
//   POST   /admin/assignments              — create an assignment

use std::path::Path;

use axum::{
    extract::{Path as AxumPath, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::{bearer_token, hash_password, Claims};
use crate::db::{assignments as db_assign, cohorts as db_cohorts, sessions as db_sess, students, subjects as db_subjects};
use crate::error::{AppError, Result};
use crate::session::broker::now_secs;

use super::AppState;

// ── Router ────────────────────────────────────────────────────────────────

pub fn routes() -> Router<AppState> {
    Router::new()
        // Students
        .route("/admin/students",          get(list_students).post(create_student))
        .route("/admin/students/{id}",     put(update_student).delete(delete_student))
        // Sessions
        .route("/admin/sessions",          get(list_sessions))
        .route("/admin/sessions/{id}",     delete(terminate_session))
        // Cohorts
        .route("/admin/cohorts",           get(list_cohorts).post(create_cohort))
        .route("/admin/cohorts/{id}",      put(update_cohort).delete(delete_cohort))
        // Subjects
        .route("/admin/subjects",          get(list_subjects_handler).post(create_subject))
        .route("/admin/subjects/{id}",     put(update_subject_handler).delete(delete_subject_handler))
        // Assignments
        .route("/admin/assignments",       get(list_assignments).post(create_assignment))
}

// ── Auth guard ────────────────────────────────────────────────────────────

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
    cohort_id: String,
    last_seen: Option<String>,
    has_active_session: bool,
}

async fn list_students(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse> {
    require_staff(&state, &headers)?;

    let rows = students::list_students(&state.pool).await?;

    let live: std::collections::HashSet<String> = state
        .broker.list_entries().await.into_iter().map(|(sid, _)| sid).collect();

    let active_sessions = db_sess::list_active_sessions(&state.pool).await?;
    let active_student_ids: std::collections::HashSet<String> = active_sessions
        .iter()
        .filter(|s| live.contains(&s.id))
        .map(|s| s.student_id.clone())
        .collect();

    let views: Vec<StudentView> = rows
        .into_iter()
        .map(|s| StudentView {
            has_active_session: active_student_ids.contains(&s.id),
            id: s.id,
            username: s.username,
            display_name: s.display_name,
            role: s.role,
            cohort_id: s.cohort_id,
            last_seen: s.last_seen,
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
    cohort_id: Option<String>,
}

async fn create_student(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateStudentBody>,
) -> Result<impl IntoResponse> {
    require_admin(&state, &headers)?;

    let password_hash = hash_password(&body.password)?;

    let student = students::Student {
        id: Uuid::new_v4().to_string(),
        username: body.username,
        password_hash,
        display_name: body.display_name,
        role: body.role.unwrap_or_else(|| "student".to_string()),
        cohort_id: body.cohort_id.unwrap_or_default(),
        uid_slot: None,
        created_at: String::new(),
        last_seen: None,
    };

    students::insert_student(&state.pool, &student).await?;

    tracing::info!(student_id = %student.id, username = %student.username, "admin: student created");
    Ok((StatusCode::CREATED, Json(serde_json::json!({ "id": student.id }))))
}

// ── PUT /admin/students/:id ───────────────────────────────────────────────

#[derive(Deserialize)]
struct UpdateStudentBody {
    display_name: Option<String>,
    cohort_id: Option<String>,
    new_password: Option<String>,
}

async fn update_student(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Json(body): Json<UpdateStudentBody>,
) -> Result<impl IntoResponse> {
    require_admin(&state, &headers)?;

    students::get_student(&state.pool, &id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("student {id}")))?;

    let new_hash = body.new_password.as_deref().map(hash_password).transpose()?;

    students::update_student(
        &state.pool,
        &id,
        body.display_name.as_deref(),
        body.cohort_id.as_deref(),
        new_hash.as_deref(),
    )
    .await?;

    tracing::info!(student_id = %id, "admin: student updated");
    Ok(StatusCode::NO_CONTENT)
}

// ── DELETE /admin/students/:id ────────────────────────────────────────────

async fn delete_student(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
) -> Result<impl IntoResponse> {
    require_admin(&state, &headers)?;
    students::get_student(&state.pool, &id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("student {id}")))?;
    students::delete_student(&state.pool, &id).await?;
    tracing::info!(student_id = %id, "admin: student deleted");
    Ok(StatusCode::NO_CONTENT)
}

// ── GET /admin/sessions ───────────────────────────────────────────────────

#[derive(Serialize)]
struct SessionView {
    id: String,
    student_id: String,
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

    let db_sessions = db_sess::list_active_sessions(&state.pool).await?;
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

    if let Some(pid) = session.namespace_pid {
        let _ = tokio::process::Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .output()
            .await;
    }

    crate::session::namespace::umount_overlay(std::path::Path::new(&session.overlay_mount)).await;
    db_sess::set_session_state(&state.pool, &id, "ended").await?;
    state.broker.remove_entry(&id).await;
    crate::security::cgroups::teardown_session_cgroup(&id).await;

    tracing::info!(session_id = %id, "admin: session force-terminated");
    Ok(StatusCode::NO_CONTENT)
}

// ── GET /admin/cohorts ────────────────────────────────────────────────────

async fn list_cohorts(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse> {
    require_staff(&state, &headers)?;
    let rows = db_cohorts::list_cohorts(&state.pool).await?;
    Ok(Json(rows))
}

// ── POST /admin/cohorts ───────────────────────────────────────────────────

#[derive(Deserialize)]
struct CreateCohortBody {
    /// Human-readable slug used as the cohort ID, e.g. "primary-2b", "year-5a".
    /// Alphanumeric, dashes, underscores only.
    slug: String,
    title: String,
}

async fn create_cohort(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateCohortBody>,
) -> Result<impl IntoResponse> {
    require_admin(&state, &headers)?;

    if body.slug.is_empty()
        || !body.slug.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return Err(AppError::BadRequest(
            "cohort slug must be non-empty alphanumeric/dash/underscore".into(),
        ));
    }

    let cohort = db_cohorts::Cohort {
        id: body.slug.clone(),
        title: body.title,
        created_at: String::new(),
    };
    db_cohorts::insert_cohort(&state.pool, &cohort).await?;

    tracing::info!(cohort_id = %cohort.id, "admin: cohort created");
    Ok((StatusCode::CREATED, Json(serde_json::json!({ "id": cohort.id }))))
}

// ── PUT /admin/cohorts/:id ────────────────────────────────────────────────

#[derive(Deserialize)]
struct UpdateCohortBody {
    /// New slug (becomes the new PK / directory name). Optional — omit to keep current.
    new_slug: Option<String>,
    title: String,
}

async fn update_cohort(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Json(body): Json<UpdateCohortBody>,
) -> Result<impl IntoResponse> {
    require_admin(&state, &headers)?;

    db_cohorts::get_cohort(&state.pool, &id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("cohort {id}")))?;

    let new_slug = body.new_slug.as_deref().unwrap_or(&id);

    if new_slug.is_empty()
        || !new_slug.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return Err(AppError::BadRequest("slug must be alphanumeric/dash/underscore".into()));
    }

    let subjects_root = std::env::var("EDUCLAW_SUBJECTS_ROOT")
        .unwrap_or_else(|_| "/srv/educlaw/data/subjects".to_string());

    // Rename directory on disk if slug changed.
    if new_slug != id {
        let old_dir = Path::new(&subjects_root).join(&id);
        let new_dir = Path::new(&subjects_root).join(new_slug);
        if old_dir.exists() {
            tokio::fs::rename(&old_dir, &new_dir)
                .await
                .map_err(|e| AppError::Internal(anyhow::anyhow!("rename cohort dir: {e}")))?;
        }
        db_cohorts::rename_cohort(&state.pool, &id, new_slug, &body.title).await?;
    } else {
        db_cohorts::update_cohort_title(&state.pool, &id, &body.title).await?;
    }

    tracing::info!(old_id = %id, new_id = %new_slug, "admin: cohort updated");
    Ok(Json(serde_json::json!({ "id": new_slug })))
}

// ── DELETE /admin/cohorts/:id ─────────────────────────────────────────────

async fn delete_cohort(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
) -> Result<impl IntoResponse> {
    require_admin(&state, &headers)?;

    db_cohorts::get_cohort(&state.pool, &id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("cohort {id}")))?;

    // Delete cohort dir from disk (best-effort — don't fail if missing).
    let subjects_root = std::env::var("EDUCLAW_SUBJECTS_ROOT")
        .unwrap_or_else(|_| "/srv/educlaw/data/subjects".to_string());
    let cohort_dir = Path::new(&subjects_root).join(&id);
    if cohort_dir.exists() {
        tokio::fs::remove_dir_all(&cohort_dir)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("remove cohort dir: {e}")))?;
    }

    db_cohorts::delete_cohort(&state.pool, &id).await?;
    tracing::info!(cohort_id = %id, "admin: cohort deleted");
    Ok(StatusCode::NO_CONTENT)
}

// ── GET /admin/subjects?cohort_id= ───────────────────────────────────────

#[derive(Deserialize)]
struct ListSubjectsQuery {
    cohort_id: Option<String>,
}

async fn list_subjects_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<ListSubjectsQuery>,
) -> Result<impl IntoResponse> {
    require_staff(&state, &headers)?;
    let cohort_id = q.cohort_id.as_deref().unwrap_or("");
    let rows = db_subjects::list_subjects(&state.pool, cohort_id).await?;
    Ok(Json(rows))
}

// ── POST /admin/subjects ──────────────────────────────────────────────────

#[derive(Deserialize)]
struct CreateSubjectBody {
    cohort_id: String,
    slug: String,
    title: String,
}

async fn create_subject(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateSubjectBody>,
) -> Result<impl IntoResponse> {
    require_admin(&state, &headers)?;

    // Validate slug: alphanumeric, dash, underscore only.
    if body.slug.is_empty()
        || !body.slug.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return Err(AppError::BadRequest(
            "slug must be non-empty alphanumeric/dash/underscore".into(),
        ));
    }

    let subject = db_subjects::Subject {
        id: Uuid::new_v4().to_string(),
        cohort_id: body.cohort_id.clone(),
        slug: body.slug.clone(),
        title: body.title,
        created_at: String::new(),
    };
    db_subjects::insert_subject(&state.pool, &subject).await?;

    // Create the subject directory on disk: {subjects_root}/{cohort_id}/{slug}/
    let subjects_root = std::env::var("EDUCLAW_SUBJECTS_ROOT")
        .unwrap_or_else(|_| "/srv/educlaw/data/subjects".to_string());
    let subject_dir = Path::new(&subjects_root)
        .join(&subject.cohort_id)
        .join(&subject.slug);
    tokio::fs::create_dir_all(&subject_dir)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("create subject dir: {e}")))?;

    // Write a minimal subject.json metadata file.
    let meta = serde_json::json!({
        "id": subject.id,
        "cohort_id": subject.cohort_id,
        "slug": subject.slug,
        "title": subject.title,
    });
    tokio::fs::write(subject_dir.join("subject.json"), meta.to_string())
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("write subject.json: {e}")))?;

    tracing::info!(
        subject_id = %subject.id,
        cohort_id = %subject.cohort_id,
        slug = %subject.slug,
        title = %subject.title,
        "admin: subject created"
    );
    Ok((StatusCode::CREATED, Json(serde_json::json!({ "id": subject.id, "slug": subject.slug }))))
}

// ── PUT /admin/subjects/:id ───────────────────────────────────────────────

#[derive(Deserialize)]
struct UpdateSubjectBody {
    /// New slug. Optional — omit to keep current slug.
    new_slug: Option<String>,
    title: String,
}

async fn update_subject_handler(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Json(body): Json<UpdateSubjectBody>,
) -> Result<impl IntoResponse> {
    require_admin(&state, &headers)?;

    let subject = db_subjects::get_subject(&state.pool, &id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("subject {id}")))?;

    let new_slug = body.new_slug.as_deref().unwrap_or(&subject.slug);

    if new_slug.is_empty()
        || !new_slug.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return Err(AppError::BadRequest("slug must be alphanumeric/dash/underscore".into()));
    }

    let subjects_root = std::env::var("EDUCLAW_SUBJECTS_ROOT")
        .unwrap_or_else(|_| "/srv/educlaw/data/subjects".to_string());

    // Rename disk directory if slug changed.
    if new_slug != subject.slug {
        let old_dir = Path::new(&subjects_root).join(&subject.cohort_id).join(&subject.slug);
        let new_dir = Path::new(&subjects_root).join(&subject.cohort_id).join(new_slug);
        if old_dir.exists() {
            tokio::fs::rename(&old_dir, &new_dir)
                .await
                .map_err(|e| AppError::Internal(anyhow::anyhow!("rename subject dir: {e}")))?;
        }
    }

    // Update subject.json in the (possibly new) directory.
    let subject_dir = Path::new(&subjects_root).join(&subject.cohort_id).join(new_slug);
    if subject_dir.exists() {
        let meta = serde_json::json!({
            "id": id,
            "cohort_id": subject.cohort_id,
            "slug": new_slug,
            "title": body.title,
        });
        let _ = tokio::fs::write(subject_dir.join("subject.json"), meta.to_string()).await;
    }

    db_subjects::update_subject(&state.pool, &id, new_slug, &body.title).await?;

    tracing::info!(subject_id = %id, new_slug = %new_slug, "admin: subject updated");
    Ok(Json(serde_json::json!({ "id": id, "slug": new_slug })))
}

// ── DELETE /admin/subjects/:id ────────────────────────────────────────────

async fn delete_subject_handler(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
) -> Result<impl IntoResponse> {
    require_admin(&state, &headers)?;

    let subject = db_subjects::get_subject(&state.pool, &id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("subject {id}")))?;

    let subjects_root = std::env::var("EDUCLAW_SUBJECTS_ROOT")
        .unwrap_or_else(|_| "/srv/educlaw/data/subjects".to_string());
    let subject_dir = Path::new(&subjects_root).join(&subject.cohort_id).join(&subject.slug);
    if subject_dir.exists() {
        tokio::fs::remove_dir_all(&subject_dir)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("remove subject dir: {e}")))?;
    }

    db_subjects::delete_subject(&state.pool, &id).await?;
    tracing::info!(subject_id = %id, "admin: subject deleted");
    Ok(StatusCode::NO_CONTENT)
}

// ── GET /admin/assignments?subject_id= ───────────────────────────────────

#[derive(Deserialize)]
struct ListAssignmentsQuery {
    subject_id: Option<String>,
}

async fn list_assignments(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<ListAssignmentsQuery>,
) -> Result<impl IntoResponse> {
    require_staff(&state, &headers)?;

    let rows = match q.subject_id.as_deref() {
        Some(sid) => db_assign::list_assignments_for_subject(&state.pool, sid).await?,
        None => {
            // Return all assignments — useful for admin overview
            sqlx::query_as::<_, db_assign::Assignment>(
                "SELECT * FROM assignments ORDER BY created_at DESC",
            )
            .fetch_all(&state.pool)
            .await?
        }
    };

    Ok(Json(rows))
}

// ── POST /admin/assignments ───────────────────────────────────────────────

#[derive(Deserialize)]
struct CreateAssignmentBody {
    subject_id: String,
    title: String,
    description: Option<String>,
    due_date: Option<String>,
}

async fn create_assignment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateAssignmentBody>,
) -> Result<impl IntoResponse> {
    let c = require_staff(&state, &headers)?;

    let assignment = db_assign::Assignment {
        id: Uuid::new_v4().to_string(),
        subject_id: body.subject_id,
        title: body.title,
        description: body.description.unwrap_or_default(),
        due_date: body.due_date,
        created_by: c.sub,
        created_at: String::new(),
    };
    db_assign::insert_assignment(&state.pool, &assignment).await?;

    tracing::info!(assignment_id = %assignment.id, "admin: assignment created");
    Ok((StatusCode::CREATED, Json(serde_json::json!({ "id": assignment.id }))))
}
