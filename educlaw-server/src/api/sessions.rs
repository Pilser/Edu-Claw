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
use crate::db::{students, subjects};
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
    model: Option<String>,
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

    // lower = {subjects_root}/{cohort_id}/{...subject slugs...} — read-only via OverlayFS
    //   cohort.id is a slug (e.g. "primary-2b"), not a UUID — already human-readable.
    // upper = {students_root}/{username}/ — student persistent home, read-write.
    //   username is unique and human-readable; the agent sees paths like
    //   /srv/educlaw/data/students/alice/ instead of an opaque UUID.
    let lower = if student.cohort_id.is_empty() {
        // No cohort assigned: fall back to an empty dir so OverlayFS still mounts cleanly.
        let fallback = std::path::PathBuf::from(&state.subjects_root).join("_empty");
        std::fs::create_dir_all(&fallback)
            .map_err(|e| AppError::Session(format!("failed to create fallback lower dir: {e}")))?;
        fallback
    } else {
        std::path::PathBuf::from(&state.subjects_root).join(&student.cohort_id)
    };
    let upper = std::path::PathBuf::from(&state.students_root).join(&student.username);

    let dirs = SessionDirs::new(&state.workspace_root, &session_id, lower, upper);

    // Prepare filesystem layout
    dirs.create_all()
        .map_err(|e| AppError::Session(format!("failed to create session dirs: {e}")))?;

    // Write student context as a claw instruction file into the upper (persistent) layer.
    // This must happen before claw starts because claw reads instructions.md while building
    // the system prompt — BEFORE plugin lifecycle hooks (init.sh) run.
    // Writing to upper/ directly means it appears in merged/ after OverlayFS mounts.
    {
        let display_name = if student.display_name.trim().is_empty() {
            student.username.clone()
        } else {
            student.display_name.clone()
        };
        let workspace_path = dirs.merged.display().to_string();
        let mut lines = vec![
            format!("You are an AI tutor for {}.", display_name),
            format!("Your workspace is at: {workspace_path}"),
            format!("Session ID: {session_id}"),
        ];
        if !student.cohort_id.is_empty() {
            lines.push(format!("Cohort ID: {}", student.cohort_id));
        }
        lines.extend_from_slice(&[
            String::new(),
            "Guide the student — ask Socratic questions rather than giving direct answers.".to_string(),
            "Use your tools to discover what subjects and assignments are available when asked.".to_owned(),
        ]);
        let instructions = lines.join("\n") + "\n";
        let claw_dir = dirs.upper.join(".claw");
        std::fs::create_dir_all(&claw_dir)
            .map_err(|e| AppError::Session(format!("failed to create .claw dir: {e}")))?;
        std::fs::write(claw_dir.join("instructions.md"), instructions)
            .map_err(|e| AppError::Session(format!("failed to write instructions.md: {e}")))?;
    }

    // Per-session plugin copy
    install_plugin(&state.plugin_path, &dirs.plugin)
        .map_err(|e| AppError::Session(format!("failed to install plugin: {e}")))?;

    // Write initial settings.json so claw discovers the plugin via externalDirectories.
    // init.sh will merge its own keys (systemPrompt, permissionMode, mcpServers) into this.
    let initial_settings = serde_json::json!({
        "plugins": {
            "externalDirectories": [dirs.plugin.display().to_string()]
        }
    });
    std::fs::write(
        dirs.config.join("settings.json"),
        serde_json::to_string_pretty(&initial_settings).unwrap(),
    ).map_err(|e| AppError::Session(format!("failed to write session settings: {e}")))?;

    // Copy credentials.json from the user's default claw config home so the
    // per-session CLAW_CONFIG_HOME has working API credentials (OAuth token etc.).
    let default_claw_home = std::env::var("HOME")
        .map(|h| std::path::PathBuf::from(h).join(".claw"))
        .ok();
    if let Some(creds_src) = default_claw_home.map(|h| h.join("credentials.json")) {
        if creds_src.exists() {
            if let Err(e) = std::fs::copy(&creds_src, dirs.config.join("credentials.json")) {
                tracing::warn!(src = %creds_src.display(), error = %e, "could not copy credentials.json into session config");
            }
        }
    }

    // Mount OverlayFS
    mount_overlay(&dirs).await?;

    // Resolve subject slugs for this cohort
    let subject_ids: Vec<String> = if student.cohort_id.is_empty() {
        vec![]
    } else {
        subjects::list_subjects(&state.pool, &student.cohort_id)
            .await?
            .into_iter()
            .map(|s| s.slug)
            .collect()
    };

    // Build the student JSON blob injected into the session environment.
    // claw's init.sh reads this to write the system prompt.
    // cohort_id and subject_ids are included for MCP access control,
    // but init.sh must NOT include subject_ids in the system prompt
    // (the agent discovers them on demand via tools).
    let student_json = serde_json::json!({
        "student_id":   student.id,
        "username":     student.username,
        "display_name": student.display_name,
        "role":         student.role,
        "cohort_id":    student.cohort_id,
        "subject_ids":  subject_ids,
        "session_id":   session_id,
    }).to_string();

    // Write student.json to config dir so ws.rs can forward EDUCLAW_STUDENT_JSON
    // to the interactive PTY spawn (the one the student actually types into).
    std::fs::write(dirs.config.join("student.json"), &student_json)
        .map_err(|e| AppError::Session(format!("failed to write student.json: {e}")))?;

    // Persist session record
    let session = Session {
        id: session_id.clone(),
        student_id: student.id.clone(),
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

    // Prefer request body model → CLAW_MODEL env → None (claw uses its own default).
    let effective_model = body.model.clone()
        .or_else(|| std::env::var("CLAW_MODEL").ok());
    if effective_model.is_none() {
        tracing::warn!("no model set in request or CLAW_MODEL env; claw will use its compiled default");
    }

    let spawn_params = SpawnParams {
        session_id: session_id.clone(),
        student_json: student_json.clone(),
        dirs: dirs.clone(),
        claw_bin: std::env::var("CLAW_BIN").unwrap_or_else(|_| "claw".to_string()),
        model: effective_model,
        env: vec![
            ("EDUCLAW_MCP_SOCKET".to_string(), mcp_socket),
        ],
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
        &student.cohort_id,
        subject_ids,
        &session_id,
    )?;

    // Update student last_seen (non-fatal)
    let _ = students::touch_last_seen(&state.pool, &student.id).await;

    tracing::info!(
        session_id = %session_id,
        student    = %student.display_name,
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
    let dirs = SessionDirs::for_existing_session(&state.workspace_root, &session_id);
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
