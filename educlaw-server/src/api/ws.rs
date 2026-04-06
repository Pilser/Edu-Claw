// educlaw-server/src/api/ws.rs
//
// GET /sessions/:id/ws?token=<jwt>
// Validates the session JWT, registers with the broker, opens PTY bridge.

use std::sync::atomic::AtomicU64;
use std::sync::Arc;

use axum::{
    extract::{Path, Query, State, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
    Router,
};
use serde::Deserialize;
use tokio::sync::mpsc;

use crate::db::sessions as db_sess;
use crate::error::{AppError, Result};
use crate::pty_bridge::run_pty_session;
use crate::session::{broker::now_secs, dirs::SessionDirs};

use super::AppState;

pub fn routes() -> Router<AppState> {
    Router::new().route("/sessions/:id/ws", get(session_ws))
}

#[derive(Deserialize)]
struct WsQuery {
    token: String,
}

async fn session_ws(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Query(q): Query<WsQuery>,
    ws: WebSocketUpgrade,
) -> Result<impl IntoResponse> {
    // Validate JWT and check it belongs to this session
    let claims = state.auth.verify(&q.token)?;
    if claims.session_id.as_deref() != Some(&session_id) {
        return Err(AppError::Unauthorized);
    }

    let session = db_sess::get_session(&state.pool, &session_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("session {session_id} not found")))?;

    if session.state != "active" {
        return Err(AppError::BadRequest(format!(
            "session {session_id} is not active (state: {})",
            session.state
        )));
    }

    let dirs = SessionDirs::new(&state.workspace_root, &session_id);
    let claw_bin = state.claw_bin.clone();
    let broker = Arc::clone(&state.broker);
    let sid = session_id.clone();

    Ok(ws.on_upgrade(move |socket| async move {
        // Retrieve the last_input_at Arc created when the session was born.
        // Fall back to a fresh one if the entry was somehow already removed.
        let last_input_at = broker
            .get_last_input_arc(&sid)
            .await
            .unwrap_or_else(|| Arc::new(AtomicU64::new(now_secs())));

        // Create channel: broker (watchdog) → PTY
        let (broker_tx, broker_rx) = mpsc::channel::<Vec<u8>>(32);
        broker.set_pty_sender(&sid, Some(broker_tx)).await;

        let mut env_vars = vec![
            ("EDUCLAW_SESSION_ID".to_string(), sid.clone()),
            ("EDUCLAW_WORKSPACE".to_string(), dirs.merged.display().to_string()),
        ];

        // portable_pty::CommandBuilder spawns with an empty environment.
        // Forward the essential vars the child process needs.
        for key in &[
            "HOME", "USER", "PATH", "TERM",
            "XAI_API_KEY", "ANTHROPIC_API_KEY", "OPENAI_API_KEY",
            "GEMINI_API_KEY", "GOOGLE_API_KEY", "MISTRAL_API_KEY",
            "GROQ_API_KEY", "OPENAI_BASE_URL",
        ] {
            if let Ok(val) = std::env::var(key) {
                env_vars.push((key.to_string(), val));
            }
        }

        // Use CLAW_MODEL env var as default model; overridable per-session in the future.
        let model = std::env::var("CLAW_MODEL").ok();

        // ── DEBUG: log exactly what we are about to pass to claw ──────────
        tracing::debug!(session_id = %sid, model = ?model, "PTY spawn: model resolved");
        let forwarded_keys: Vec<&str> = env_vars.iter()
            .map(|(k, _)| k.as_str())
            .collect();
        tracing::debug!(session_id = %sid, keys = ?forwarded_keys, "PTY spawn: env keys forwarded");
        let xai_present = env_vars.iter().any(|(k, _)| k == "XAI_API_KEY");
        let anthropic_present = env_vars.iter().any(|(k, _)| k == "ANTHROPIC_API_KEY");
        tracing::info!(session_id = %sid, model = ?model, xai_key = xai_present, anthropic_key = anthropic_present, "PTY spawn: credentials check");
        // ──────────────────────────────────────────────────────────────────

        if let Err(e) = run_pty_session(
            socket,
            claw_bin.into(),
            dirs.merged.clone(),
            dirs.config.clone(),
            env_vars,
            model,
            last_input_at,
            broker_rx,
        )
        .await
        {
            tracing::error!(session_id = %sid, error = %e, "PTY session error");
        }

        // Browser disconnected: clear sender so watchdog knows
        broker.set_pty_sender(&sid, None).await;
        tracing::info!(session_id = %sid, "WebSocket disconnected");
    }))
}
