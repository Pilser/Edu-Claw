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

        let env_vars = vec![
            ("EDUCLAW_SESSION_ID".to_string(), sid.clone()),
            ("EDUCLAW_WORKSPACE".to_string(), dirs.merged.display().to_string()),
        ];

        if let Err(e) = run_pty_session(
            socket,
            claw_bin.into(),
            dirs.merged.clone(),
            dirs.config.clone(),
            dirs.plugin.clone(),
            env_vars,
            None,
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
