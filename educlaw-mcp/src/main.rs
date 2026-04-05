// educlaw-mcp/src/main.rs
//
// Entry point for the EduClaw MCP server.
//
// Transport modes (selected by env var MCP_TRANSPORT):
//   stdio  (default) — launched as a child process by claw; communicates over stdin/stdout
//   tcp              — persistent host process; claw connects via TCP socket

mod config;
mod embedding;
mod search_index;
mod server;
mod servers;

// ── Shared helper used by all servers/ modules ────────────────────────────

/// Build an rmcp `Tool` from a name, description, and JSON Schema value.
/// The schema value must be a JSON object with "type", "properties", "required".
pub fn make_tool(
    name: &str,
    description: &str,
    schema: serde_json::Value,
) -> rmcp::model::Tool {
    use std::borrow::Cow;
    use std::sync::Arc;
    rmcp::model::Tool {
        name: Cow::Owned(name.to_string()),
        title: None,
        description: Some(Cow::Owned(description.to_string())),
        input_schema: Arc::new(
            schema
                .as_object()
                .cloned()
                .unwrap_or_else(serde_json::Map::new),
        ),
        output_schema: None,
        annotations: None,
        icons: None,
        meta: None,
    }
}

use std::sync::Arc;

use tracing::info;

use config::Config;
use server::EduclawMcpServer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialise logging to stderr (claw reads stdout as MCP protocol)
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .with_writer(std::io::stderr)
        .init();

    let config = Config::load()?;
    info!("config loaded — classes_root={}", config.classes_root);

    let server = Arc::new(EduclawMcpServer::new(config).await?);
    info!("EduclawMcpServer ready");

    let transport = std::env::var("MCP_TRANSPORT").unwrap_or_else(|_| "stdio".to_string());

    match transport.as_str() {
        "tcp" => run_tcp(server).await,
        _ => run_stdio(server).await,
    }
}

// ── stdio transport ───────────────────────────────────────────────────────

async fn run_stdio(server: Arc<EduclawMcpServer>) -> anyhow::Result<()> {
    use rmcp::transport::io::stdio;
    use rmcp::ServiceExt;

    info!("starting stdio MCP transport");
    let service = (*server).clone().serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}

// ── TCP transport ─────────────────────────────────────────────────────────

async fn run_tcp(server: Arc<EduclawMcpServer>) -> anyhow::Result<()> {
    use rmcp::serve_server;
    use tokio::net::TcpListener;

    let port: u16 = std::env::var("MCP_TCP_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(9000);

    let addr = format!("127.0.0.1:{port}");
    let listener = TcpListener::bind(&addr).await?;
    info!("TCP MCP server listening on {addr}");

    loop {
        let (stream, peer) = listener.accept().await?;
        stream.set_nodelay(true)?;
        info!("connection from {peer}");
        let srv = (*server).clone();
        tokio::spawn(async move {
            match serve_server(srv, stream).await {
                Ok(svc) => {
                    if let Err(e) = svc.waiting().await {
                        tracing::warn!("session ended: {e}");
                    }
                }
                Err(e) => tracing::error!("serve_server error: {e}"),
            }
        });
    }
}
