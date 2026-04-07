// educlaw-server/src/api/files.rs
//
// File browser REST API
//
// Endpoints:
//   GET  /files/subject/:subject_id              — list subject materials tree
//   GET  /files/subject/:subject_id/read?path=   — read a subject file
//   GET  /files/student/:session_id/tree         — list student workspace tree
//   GET  /files/student/:session_id/read?path=   — read a student file
//   PUT  /files/student/:session_id/write?path=  — write a student file
//   DELETE /files/student/:session_id/delete?path= — delete a student file
//
// Auth:
//   All routes require a valid Bearer JWT.
//   Subject routes: student's cohort must include the subject_id
//     (subject_ids from JWT) OR be a teacher.
//   Student routes: session_id in path must match JWT's session_id claim
//     OR caller is a teacher.

use std::path::{Path, PathBuf};

use axum::{
    body::Body,
    extract::{Path as AxumPath, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::auth::{bearer_token, Claims};
use crate::db::students;
use crate::error::{AppError, Result};

use super::AppState;

// ── Router ────────────────────────────────────────────────────────────────

pub fn routes() -> Router<AppState> {
    Router::new()
        // Subject materials
        .route("/files/subject/:subject_id/tree", get(list_subject_files))
        .route("/files/subject/:subject_id/read", get(read_subject_file))
        // Student workspace
        .route("/files/student/:session_id/tree",   get(list_student))
        .route("/files/student/:session_id/read",   get(read_student_file))
        .route("/files/student/:session_id/write",  put(write_student_file))
        .route("/files/student/:session_id/delete", delete(delete_student_file))
}

// ── Shared types ──────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct FileNode {
    pub name: String,
    pub path: String,
    pub kind: &'static str,   // "file" | "dir"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<FileNode>>,
}

// ── JWT extraction helper ─────────────────────────────────────────────────

fn require_claims(state: &AppState, headers: &axum::http::HeaderMap) -> Result<Claims> {
    let token = bearer_token(headers).ok_or(AppError::Unauthorized)?;
    state.auth.verify(token)
}

// ── Path safety ───────────────────────────────────────────────────────────

/// Validates that `rel` resolves inside `base` — no `..` escapes.
/// Returns the canonicalisable absolute path (without requiring it to exist).
fn safe_join(base: &Path, rel: &str) -> Result<PathBuf> {
    let rel = rel.trim_start_matches('/');
    let joined = base.join(rel);
    let final_path = normalize_path(&joined);
    let base_norm  = normalize_path(base);
    if !final_path.starts_with(&base_norm) {
        return Err(AppError::BadRequest("path traversal detected".into()));
    }
    Ok(final_path)
}

fn normalize_path(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            std::path::Component::ParentDir => { out.pop(); }
            std::path::Component::CurDir    => {}
            other => out.push(other),
        }
    }
    out
}

/// Validate a subject_id or session_id slug — alphanumeric + dash + underscore only.
fn validate_id(s: &str, label: &str) -> Result<String> {
    if !s.is_empty() && s.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
        Ok(s.to_string())
    } else {
        Err(AppError::BadRequest(format!("invalid {label}")))
    }
}

// ── File tree builder ─────────────────────────────────────────────────────

fn build_tree(root: &Path, root_prefix: &str) -> Vec<FileNode> {
    let mut nodes: Vec<FileNode> = Vec::new();

    let entries = match std::fs::read_dir(root) {
        Ok(e) => e,
        Err(_) => return nodes,
    };

    let mut dirs  = Vec::new();
    let mut files = Vec::new();

    for entry in entries.flatten() {
        let ft = match entry.file_type() {
            Ok(f) => f,
            Err(_) => continue,
        };
        let name = entry.file_name().to_string_lossy().to_string();
        // Skip hidden files and OverlayFS internals
        if name.starts_with('.') { continue; }
        let rel_path = format!("{root_prefix}/{name}");

        if ft.is_dir() {
            dirs.push((name, rel_path, entry.path()));
        } else {
            let size = entry.metadata().ok().map(|m| m.len());
            files.push(FileNode {
                name,
                path: rel_path,
                kind: "file",
                size,
                children: None,
            });
        }
    }

    dirs.sort_by(|a, b| a.0.cmp(&b.0));
    files.sort_by(|a, b| a.name.cmp(&b.name));

    for (name, rel_path, abs_path) in dirs {
        let children = build_tree(&abs_path, &rel_path);
        nodes.push(FileNode {
            name,
            path: rel_path,
            kind: "dir",
            size: None,
            children: Some(children),
        });
    }
    nodes.extend(files);
    nodes
}

// ── Content response ──────────────────────────────────────────────────────

fn file_response(path: &Path) -> Result<Response> {
    let bytes = std::fs::read(path)
        .map_err(|e| AppError::NotFound(format!("file not found: {e}")))?;

    let mime = mime_guess(path);
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime)
        .body(Body::from(bytes))
        .unwrap())
}

fn mime_guess(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("css")  => "text/css; charset=utf-8",
        Some("js")   => "application/javascript",
        Some("json") => "application/json",
        Some("md")   => "text/markdown; charset=utf-8",
        Some("py")   => "text/x-python; charset=utf-8",
        Some("rs")   => "text/x-rust; charset=utf-8",
        Some("txt")  => "text/plain; charset=utf-8",
        Some("png")  => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("svg")  => "image/svg+xml",
        Some("pdf")  => "application/pdf",
        _            => "application/octet-stream",
    }
}

// ── Student home dir resolver ────────────────────────────────────────────

/// Resolve `{students_root}/{username}` from the JWT `sub` claim (student UUID).
/// This is the OverlayFS upper layer — the student's persistent home.
async fn student_home_dir(state: &AppState, student_id: &str) -> Result<PathBuf> {
    let student = students::get_student(&state.pool, student_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("student {student_id} not found")))?;
    Ok(PathBuf::from(&state.students_root).join(&student.username))
}

// ── Subject routes ────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct PathQuery {
    path: Option<String>,
}

async fn list_subject_files(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    AxumPath(subject_id): AxumPath<String>,
) -> Result<Json<Vec<FileNode>>> {
    let claims = require_claims(&state, &headers)?;
    let subject_id = validate_id(&subject_id, "subject_id")?;
    assert_subject_access(&claims, &subject_id)?;

    let base = PathBuf::from(&state.subjects_root).join(&claims.cohort_id).join(&subject_id);

    if !base.exists() {
        return Err(AppError::NotFound(format!("subject {subject_id} not found")));
    }

    Ok(Json(build_tree(&base, "")))
}

async fn read_subject_file(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    AxumPath(subject_id): AxumPath<String>,
    Query(q): Query<PathQuery>,
) -> Result<Response> {
    let rel_path = q.path.ok_or_else(|| AppError::BadRequest("missing ?path= query param".into()))?;
    let claims = require_claims(&state, &headers)?;
    let subject_id = validate_id(&subject_id, "subject_id")?;
    assert_subject_access(&claims, &subject_id)?;

    let base = PathBuf::from(&state.subjects_root).join(&claims.cohort_id).join(&subject_id);
    let path = safe_join(&base, &rel_path)?;

    file_response(&path)
}

// ── Student routes ────────────────────────────────────────────────────────

async fn list_student(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    AxumPath(session_id): AxumPath<String>,
) -> Result<Json<Vec<FileNode>>> {
    let claims = require_claims(&state, &headers)?;
    let session_id = validate_id(&session_id, "session_id")?;
    assert_session_access(&claims, &session_id)?;

    let upper = student_home_dir(&state, &claims.sub).await?;

    if !upper.exists() {
        return Ok(Json(vec![]));
    }

    // OverlayFS copy-up: when the agent reads or stats a subject directory,
    // the kernel can create an empty stub of that dir inside `upper`.
    // Filter those stubs out so "My Files" only shows student-owned content.
    // The subject slugs are already in the JWT — no extra DB call needed.
    let subject_slugs: std::collections::HashSet<&str> =
        claims.subject_ids.iter().map(|s| s.as_str()).collect();

    let tree = build_tree(&upper, "");
    let filtered = tree
        .into_iter()
        .filter(|node| !(node.kind == "dir" && subject_slugs.contains(node.name.as_str())))
        .collect();

    Ok(Json(filtered))
}

async fn read_student_file(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    AxumPath(session_id): AxumPath<String>,
    Query(q): Query<PathQuery>,
) -> Result<Response> {
    let rel_path = q.path.ok_or_else(|| AppError::BadRequest("missing ?path= query param".into()))?;
    let claims = require_claims(&state, &headers)?;
    let session_id = validate_id(&session_id, "session_id")?;
    assert_session_access(&claims, &session_id)?;

    let upper = student_home_dir(&state, &claims.sub).await?;
    let path = safe_join(&upper, &rel_path)?;
    file_response(&path)
}

#[derive(Deserialize)]
struct WriteBody {
    content: String,
}

async fn write_student_file(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    AxumPath(session_id): AxumPath<String>,
    Query(q): Query<PathQuery>,
    Json(body): Json<WriteBody>,
) -> Result<impl IntoResponse> {
    let rel_path = q.path.ok_or_else(|| AppError::BadRequest("missing ?path= query param".into()))?;
    let claims = require_claims(&state, &headers)?;
    let session_id = validate_id(&session_id, "session_id")?;
    assert_session_access(&claims, &session_id)?;

    let upper = student_home_dir(&state, &claims.sub).await?;
    let path = safe_join(&upper, &rel_path)?;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, body.content.as_bytes())?;
    Ok(StatusCode::NO_CONTENT)
}

async fn delete_student_file(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    AxumPath(session_id): AxumPath<String>,
    Query(q): Query<PathQuery>,
) -> Result<impl IntoResponse> {
    let rel_path = q.path.ok_or_else(|| AppError::BadRequest("missing ?path= query param".into()))?;
    let claims = require_claims(&state, &headers)?;
    let session_id = validate_id(&session_id, "session_id")?;
    assert_session_access(&claims, &session_id)?;

    let upper = student_home_dir(&state, &claims.sub).await?;
    let path = safe_join(&upper, &rel_path)?;

    if path.is_dir() {
        std::fs::remove_dir_all(&path)?;
    } else {
        std::fs::remove_file(&path)
            .map_err(|e| AppError::NotFound(format!("file not found: {e}")))?;
    }
    Ok(StatusCode::NO_CONTENT)
}

// ── Access control ────────────────────────────────────────────────────────

fn assert_subject_access(claims: &Claims, subject_id: &str) -> Result<()> {
    if claims.role == "teacher" || claims.role == "admin" {
        return Ok(());
    }
    if claims.subject_ids.iter().any(|s| s == subject_id) {
        Ok(())
    } else {
        Err(AppError::Unauthorized)
    }
}

fn assert_session_access(claims: &Claims, session_id: &str) -> Result<()> {
    if claims.role == "teacher" || claims.role == "admin" {
        return Ok(());
    }
    if claims.session_id.as_deref() == Some(session_id) {
        Ok(())
    } else {
        Err(AppError::Unauthorized)
    }
}
