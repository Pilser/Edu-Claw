// educlaw-server/src/main.rs
//
// EduClaw backend — entry point.

mod api;
mod auth;
mod db;
mod error;
mod pty_bridge;
mod session;

use std::net::SocketAddr;

use axum::Router;
use sqlx::sqlite::SqlitePoolOptions;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::auth::AuthConfig;
use crate::db::schema::init_db;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();

    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite://educlaw.db?mode=rwc".to_string());
    let pool = SqlitePoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await?;
    init_db(&pool).await?;
    tracing::info!("database ready at {database_url}");

    let jwt_secret = std::env::var("JWT_SECRET")
        .unwrap_or_else(|_| "change-me-in-production".to_string());
    let auth_config = AuthConfig::new(jwt_secret);

    let workspace_root = std::env::var("EDUCLAW_WORKSPACE_ROOT")
        .unwrap_or_else(|_| "/var/lib/educlaw/sessions".to_string());
    std::fs::create_dir_all(&workspace_root)?;

    let plugin_path = std::env::var("EDUCLAW_PLUGIN_PATH")
        .unwrap_or_else(|_| "/opt/educlaw/plugin".to_string());

    let claw_bin = std::env::var("CLAW_BIN").unwrap_or_else(|_| "claw".to_string());

    let state = api::AppState::new(pool, auth_config, workspace_root, plugin_path, claw_bin);

    let app = Router::new()
        .merge(api::routes())
        .with_state(state)
        .layer(tower_http::cors::CorsLayer::permissive())
        .layer(tower_http::trace::TraceLayer::new_for_http());

    let addr: SocketAddr = std::env::var("BIND_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:8080".to_string())
        .parse()?;

    tracing::info!("educlaw-server listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
