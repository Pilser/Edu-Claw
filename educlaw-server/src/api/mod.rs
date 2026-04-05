// educlaw-server/src/api/mod.rs
//
// Shared application state and route assembly.

mod admin;
mod auth_routes;
mod files;
mod sessions;
mod students;
mod ws;

use axum::{routing::get, Json, Router};
use sqlx::SqlitePool;
use std::sync::Arc;

use crate::auth::AuthConfig;
use crate::session::broker::SessionBroker;

// ── Shared application state ──────────────────────────────────────────────

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub auth: AuthConfig,
    pub workspace_root: String,
    pub plugin_path: String,
    pub claw_bin: String,
    pub broker: Arc<SessionBroker>,
}

impl AppState {
    pub fn new(
        pool: SqlitePool,
        auth: AuthConfig,
        workspace_root: String,
        plugin_path: String,
        claw_bin: String,
        broker: Arc<SessionBroker>,
    ) -> Self {
        Self { pool, auth, workspace_root, plugin_path, claw_bin, broker }
    }
}

// ── Router assembly ───────────────────────────────────────────────────────

pub fn routes() -> Router<AppState> {
    Router::new()
        .merge(auth_routes::routes())
        .merge(admin::routes())
        .merge(students::routes())
        .merge(sessions::routes())
        .merge(ws::routes())
        .merge(files::routes())
        .route("/health", get(health))
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
}
