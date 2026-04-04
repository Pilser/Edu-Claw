// educlaw-server/src/api/ws.rs
//
// GET /sessions/:id/ws — upgrade to WebSocket and start PTY bridge.

use axum::{
    extract::{Path, State, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
    Router,
};

use crate::db::sessions as db_sess;
use crate::error::{AppError, Result};
use crate::pty_bridge::run_pty_session;
use crate::session::dirs::SessionDirs;

use super::AppState;

pub fn routes() -> Router<AppState> {
    Router::new().route("/sessions/:id/ws", get(session_ws))
}

async fn session_ws(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    ws: WebSocketUpgrade,
) -> Result<impl IntoResponse> {
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

    Ok(ws.on_upgrade(move |socket| async move {
        let env_vars = vec![
            ("EDUCLAW_SESSION_ID".to_string(), session_id.clone()),
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
        )
        .await
        {
            tracing::error!(session_id = %session_id, error = %e, "PTY session error");
        }
    }))
}
