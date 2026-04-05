// educlaw-server/src/api/files.rs
//
// File browser REST API — Phase 4.1
//
// Endpoints:
//   GET  /files/class/:class_id              — list class materials tree
//   GET  /files/class/:class_id/*path        — read a class file
//   GET  /files/student/:session_id          — list student workspace tree
//   GET  /files/student/:session_id/*path    — read a student file
//   PUT  /files/student/:session_id/*path    — write a student file
//   DELETE /files/student/:session_id/*path  — delete a student file
//   GET  /files/content?path=<abs>           — (internal) raw content by abs path
//
// Auth:
//   All routes require a valid Bearer JWT.
//   Class routes: student must be enrolled in the class
//     (class_ids from JWT) OR be a teacher.
//   Student routes: session_id in path must match the JWT's session_id claim
//     OR caller is a teacher.

use std::path::{Path, PathBuf};

use axum::{
    body::Body,
    extract::{Path as AxumPath, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::auth::{bearer_token, Claims};
use crate::error::{AppError, Result};
use crate::session::dirs::SessionDirs;

use super::AppState;

// ── Router ────────────────────────────────────────────────────────────────

pub fn routes() -> Router<AppState> {
    Router::new()
        // Class materials
        .route("/files/class/{class_id}",       get(list_class))
        .route("/files/class/{class_id}/{*path}", get(read_class_file))
        // Student workspace
        .route("/files/student/{session_id}",        get(list_student))
        .route(
            "/files/student/{session_id}/{*path}",
            get(read_student_file)
                .put(write_student_file)
                .delete(delete_student_file),
        )
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

/// Validate a class_id or session_id slug — alphanumeric + dash + underscore only.
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

// ── Class routes ──────────────────────────────────────────────────────────

async fn list_class(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    AxumPath(class_id): AxumPath<String>,
) -> Result<Json<Vec<FileNode>>> {
    let claims = require_claims(&state, &headers)?;
    let class_id = validate_id(&class_id, "class_id")?;
    assert_class_access(&claims, &class_id)?;

    let classes_root = std::env::var("EDUCLAW_CLASSES_ROOT")
        .unwrap_or_else(|_| "/data/classes".to_string());
    let base = PathBuf::from(&classes_root).join(&class_id);

    if !base.exists() {
        return Err(AppError::NotFound(format!("class {class_id} not found")));
    }

    Ok(Json(build_tree(&base, "")))
}

async fn read_class_file(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    AxumPath((class_id, rel_path)): AxumPath<(String, String)>,
) -> Result<Response> {
    let claims = require_claims(&state, &headers)?;
    let class_id = validate_id(&class_id, "class_id")?;
    assert_class_access(&claims, &class_id)?;

    let classes_root = std::env::var("EDUCLAW_CLASSES_ROOT")
        .unwrap_or_else(|_| "/data/classes".to_string());
    let base = PathBuf::from(&classes_root).join(&class_id);
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

    let dirs = SessionDirs::new(&state.workspace_root, &session_id);

    if !dirs.upper.exists() {
        return Ok(Json(vec![]));
    }

    Ok(Json(build_tree(&dirs.upper, "")))
}

async fn read_student_file(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    AxumPath((session_id, rel_path)): AxumPath<(String, String)>,
) -> Result<Response> {
    let claims = require_claims(&state, &headers)?;
    let session_id = validate_id(&session_id, "session_id")?;
    assert_session_access(&claims, &session_id)?;

    let dirs = SessionDirs::new(&state.workspace_root, &session_id);
    let path = safe_join(&dirs.upper, &rel_path)?;
    file_response(&path)
}

#[derive(Deserialize)]
struct WriteBody {
    content: String,
}

async fn write_student_file(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    AxumPath((session_id, rel_path)): AxumPath<(String, String)>,
    Json(body): Json<WriteBody>,
) -> Result<impl IntoResponse> {
    let claims = require_claims(&state, &headers)?;
    let session_id = validate_id(&session_id, "session_id")?;
    assert_session_access(&claims, &session_id)?;

    let dirs = SessionDirs::new(&state.workspace_root, &session_id);
    let path = safe_join(&dirs.upper, &rel_path)?;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, body.content.as_bytes())?;
    Ok(StatusCode::NO_CONTENT)
}

async fn delete_student_file(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    AxumPath((session_id, rel_path)): AxumPath<(String, String)>,
) -> Result<impl IntoResponse> {
    let claims = require_claims(&state, &headers)?;
    let session_id = validate_id(&session_id, "session_id")?;
    assert_session_access(&claims, &session_id)?;

    let dirs = SessionDirs::new(&state.workspace_root, &session_id);
    let path = safe_join(&dirs.upper, &rel_path)?;

    if path.is_dir() {
        std::fs::remove_dir_all(&path)?;
    } else {
        std::fs::remove_file(&path)
            .map_err(|e| AppError::NotFound(format!("file not found: {e}")))?;
    }
    Ok(StatusCode::NO_CONTENT)
}

// ── Access control ────────────────────────────────────────────────────────

fn assert_class_access(claims: &Claims, class_id: &str) -> Result<()> {
    if claims.role == "teacher" || claims.role == "admin" {
        return Ok(());
    }
    if claims.class_ids.iter().any(|c| c == class_id) {
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
