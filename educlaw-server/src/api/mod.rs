// educlaw-server/src/api/mod.rs
//
// Shared application state and route assembly.

mod auth_routes;
mod sessions;
mod students;
mod ws;

use axum::{routing::get, Json, Router};
use sqlx::SqlitePool;

use crate::auth::AuthConfig;

// ── Shared application state ──────────────────────────────────────────────

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub auth: AuthConfig,
    pub workspace_root: String,
    pub plugin_path: String,
    pub claw_bin: String,
}

impl AppState {
    pub fn new(
        pool: SqlitePool,
        auth: AuthConfig,
        workspace_root: String,
        plugin_path: String,
        claw_bin: String,
    ) -> Self {
        Self { pool, auth, workspace_root, plugin_path, claw_bin }
    }
}

// ── Router assembly ───────────────────────────────────────────────────────

pub fn routes() -> Router<AppState> {
    Router::new()
        .merge(auth_routes::routes())
        .merge(students::routes())
        .merge(sessions::routes())
        .merge(ws::routes())
        .route("/health", get(health))
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
}
