// educlaw-server/src/api/sessions.rs
//
// POST   /sessions        — create and initialise a student session
// GET    /sessions/:id    — get session status
// DELETE /sessions/:id    — terminate session (SIGTERM + umount)
// POST   /sessions/:id/broadcast — push a message to a session's PTY (teacher/admin)

use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::sessions::{self as db_sess, Session};
use crate::db::students;
use crate::auth::bearer_token;
use crate::error::{AppError, Result};
use crate::session::{
    broker::now_secs,
    dirs::SessionDirs,
    namespace::{install_plugin, mount_overlay, umount_overlay, spawn_claw_session, SpawnParams},
    watchdog::{run_watchdog, WatchdogConfig},
};

use super::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/sessions", post(create_session))
        .route("/sessions/:id", get(get_session))
        .route("/sessions/:id", delete(terminate_session))
        .route("/sessions/:id/broadcast", post(broadcast_message))
}

// ── Create session ────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct CreateSessionRequest {
    student_id: String,
    course: String,
    assignment: Option<String>,
    model: Option<String>,
    /// Extra env vars to forward into the session (e.g. API keys).
    api_keys: Option<std::collections::HashMap<String, String>>,
}

#[derive(Serialize)]
struct CreateSessionResponse {
    session_id: String,
    ws_url: String,
    token: String,
}

async fn create_session(
    State(state): State<AppState>,
    Json(body): Json<CreateSessionRequest>,
) -> Result<impl IntoResponse> {
    let student = students::get_student(&state.pool, &body.student_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("student {} not found", body.student_id)))?;

    let session_id = Uuid::new_v4().to_string();
    let dirs = SessionDirs::new(&state.workspace_root, &session_id);

    // Prepare filesystem layout
    dirs.create_all()
        .map_err(|e| AppError::Session(format!("failed to create session dirs: {e}")))?;

    // Per-session plugin copy (init.sh will write settings.json into config/)
    install_plugin(&state.plugin_path, &dirs.plugin)
        .map_err(|e| AppError::Session(format!("failed to install plugin: {e}")))?;

    // Mount OverlayFS
    mount_overlay(&dirs).await?;

    // Parse student class_ids for JWT
    let class_ids: Vec<String> =
        serde_json::from_str(&student.class_ids).unwrap_or_default();

    // Build the student JSON blob injected into the session environment.
    // claw's init.sh reads this to write the system prompt.
    let student_json = serde_json::json!({
        "student_id":   student.id,
        "username":     student.username,
        "display_name": student.display_name,
        "role":         student.role,
        "class_ids":    class_ids,
        "session_id":   session_id,
        "course":       body.course,
        "assignment":   body.assignment,
    }).to_string();

    // Persist session record
    let session = Session {
        id: session_id.clone(),
        student_id: student.id.clone(),
        course: body.course.clone(),
        assignment: body.assignment.clone(),
        state: "active".to_string(),
        namespace_pid: None,
        overlay_mount: dirs.merged.display().to_string(),
        last_input_at: None,
        started_at: String::new(), // set by DB DEFAULT
        ended_at: None,
    };
    db_sess::insert_session(&state.pool, &session).await?;

    // Create broker entry + spawn idle watchdog
    let last_input_at = Arc::new(AtomicU64::new(now_secs()));
    state.broker.create_entry(&session_id, Arc::clone(&last_input_at)).await;
    let idle_secs: u64 = std::env::var("IDLE_TIMEOUT_S")
        .ok().and_then(|s| s.parse().ok()).unwrap_or(900);
    let warn_secs: u64 = std::env::var("WARN_BEFORE_S")
        .ok().and_then(|s| s.parse().ok()).unwrap_or(60);
    tokio::spawn(run_watchdog(
        WatchdogConfig {
            session_id: session_id.clone(),
            overlay_mount: dirs.merged.clone(),
            idle_timeout: Duration::from_secs(idle_secs),
            warn_before:  Duration::from_secs(warn_secs),
        },
        Arc::clone(&state.broker),
        state.pool.clone(),
    ));

    // Spawn claw inside an unshare namespace; record the PID for Shell MCP nsenter.
    let mcp_socket = std::env::var("EDUCLAW_MCP_SOCKET")
        .unwrap_or_else(|_| "127.0.0.1:9000".to_string());
    let api_keys: Vec<(String, String)> = body
        .api_keys
        .unwrap_or_default()
        .into_iter()
        .collect();
    let spawn_params = SpawnParams {
        session_id: session_id.clone(),
        student_json: student_json.clone(),
        dirs: dirs.clone(),
        claw_bin: std::env::var("CLAW_BIN").unwrap_or_else(|_| "claw".to_string()),
        model: body.model.clone(),
        env: {
            let mut env = api_keys;
            env.push(("EDUCLAW_MCP_SOCKET".to_string(), mcp_socket));
            env
        },
    };
    match spawn_claw_session(&spawn_params).await {
        Ok(pid) => {
            let _ = db_sess::set_namespace_pid(&state.pool, &session_id, pid as i64).await;
            // Apply cgroups v2 resource caps (best-effort; logs on failure).
            crate::security::cgroups::setup_session_cgroup(&session_id, pid).await;
            tracing::info!(session_id = %session_id, pid = pid, "claw namespace spawned");
        }
        Err(e) => {
            tracing::warn!(session_id = %session_id, error = %e, "claw spawn failed (PTY bridge will retry)");
        }
    }

    // Session-scoped JWT for the WebSocket handshake
    let token = state.auth.sign_session_token(
        &student.id,
        &student.role,
        class_ids,
        &session_id,
    )?;

    // Update student last_seen (non-fatal)
    let _ = students::touch_last_seen(&state.pool, &student.id).await;

    tracing::info!(
        session_id = %session_id,
        student    = %student.display_name,
        course     = %body.course,
        "session created"
    );

    Ok((
        StatusCode::CREATED,
        Json(CreateSessionResponse {
            session_id: session_id.clone(),
            ws_url: format!("/sessions/{session_id}/ws"),
            token,
        }),
    ))
}

// ── Get session ───────────────────────────────────────────────────────────

async fn get_session(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<impl IntoResponse> {
    let session = db_sess::get_session(&state.pool, &session_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("session {session_id} not found")))?;
    Ok(Json(session))
}

// ── Terminate session ─────────────────────────────────────────────────────

async fn terminate_session(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<impl IntoResponse> {
    let session = db_sess::get_session(&state.pool, &session_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("session {session_id} not found")))?;

    // SIGTERM the namespace init process
    if let Some(pid) = session.namespace_pid {
        let _ = std::process::Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .output();
    }

    // Best-effort overlay unmount
    let dirs = SessionDirs::new(&state.workspace_root, &session_id);
    umount_overlay(&dirs.merged).await;

    db_sess::set_session_state(&state.pool, &session_id, "ended").await?;
    state.broker.remove_entry(&session_id).await;
    crate::security::cgroups::teardown_session_cgroup(&session_id).await;

    tracing::info!(session_id = %session_id, "session terminated");
    Ok(StatusCode::NO_CONTENT)
}

// ── Broadcast message ─────────────────────────────────────────────────────

#[derive(Deserialize)]
struct BroadcastBody {
    message: String,
    /// If true, send to ALL active sessions (teacher/admin only).
    #[serde(default)]
    all: bool,
}

async fn broadcast_message(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<BroadcastBody>,
) -> Result<impl IntoResponse> {
    // Require teacher or admin role.
    let token = bearer_token(&headers).ok_or(AppError::Unauthorized)?;
    let claims = state.auth.verify(token)?;
    if claims.role != "teacher" && claims.role != "admin" {
        return Err(AppError::Forbidden);
    }

    // Format the banner: ANSI bold yellow, visible on any dark/light terminal.
    let banner = format!(
        "\r\n\x1b[1;33m[EduClaw] {}\x1b[0m\r\n",
        body.message
    );
    let bytes = banner.into_bytes();

    if body.all {
        let sessions = state.broker.list_entries().await;
        for (sid, _) in sessions {
            state.broker.write_to_pty(&sid, bytes.clone()).await;
        }
        tracing::info!(from = %claims.sub, "broadcast to all sessions");
    } else {
        state.broker.write_to_pty(&session_id, bytes).await;
        tracing::info!(session_id = %session_id, from = %claims.sub, "broadcast to session");
    }

    Ok(StatusCode::NO_CONTENT)
}
