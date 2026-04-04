// educlaw-server/src/pty_bridge.rs
//
// PTY-over-WebSocket bridge.
//
// When a WebSocket client connects to /sessions/:id/ws:
//   1. We open a PTY pair using portable-pty
//   2. We spawn `claw` with the slave end as its stdio
//   3. We relay PTY output → WebSocket and WebSocket input → PTY
//   4. We handle resize events: {"type":"resize","cols":N,"rows":N}
//   5. Raw text / binary frames are forwarded as terminal input

use std::io::Write as _;
use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use serde::Deserialize;
use tokio::sync::Mutex;

use crate::error::{AppError, Result};

/// Message sent by the browser over the WebSocket connection.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum ClientMessage {
    Input { data: String },
    Resize { cols: u16, rows: u16 },
}

/// Spawn `claw` on a PTY and bridge its I/O to the WebSocket.
pub async fn run_pty_session(
    ws: WebSocket,
    claw_bin: PathBuf,
    workspace: PathBuf,
    config_home: PathBuf,
    plugin_dir: PathBuf,
    env_vars: Vec<(String, String)>,
    model: Option<String>,
) -> Result<()> {
    let pty_system = native_pty_system();

    let pty_pair = pty_system
        .openpty(PtySize { rows: 24, cols: 80, pixel_width: 0, pixel_height: 0 })
        .map_err(|e| AppError::Session(format!("failed to open PTY: {e}")))?;

    // Build claw command
    let mut cmd = CommandBuilder::new(&claw_bin);
    if let Some(ref m) = model {
        cmd.arg("--model");
        cmd.arg(m);
    }
    cmd.arg("--add-dir");
    cmd.arg(plugin_dir.display().to_string());
    cmd.cwd(&workspace);
    cmd.env("CLAW_CONFIG_HOME", config_home.display().to_string());
    for (k, v) in &env_vars {
        cmd.env(k, v);
    }

    let mut child = pty_pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| AppError::Session(format!("failed to spawn claw on PTY: {e}")))?;

    // Wrap the master writer behind an Arc<Mutex> so we can share it across tasks
    let master_writer = Arc::new(Mutex::new(
        pty_pair
            .master
            .take_writer()
            .map_err(|e| AppError::Session(format!("PTY writer error: {e}")))?,
    ));

    // Clone the master for reading (blocking — run in a background thread)
    let master_reader = pty_pair
        .master
        .try_clone_reader()
        .map_err(|e| AppError::Session(format!("PTY reader error: {e}")))?;

    let (mut ws_sender, mut ws_receiver) = ws.split();

    // Task: PTY output → WebSocket
    let pty_to_ws = tokio::spawn(async move {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<Vec<u8>>(64);

        std::thread::spawn(move || {
            let mut reader = master_reader;
            let mut buf = [0u8; 4096];
            loop {
                match std::io::Read::read(&mut reader, &mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if tx.blocking_send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                }
            }
        });

        while let Some(data) = rx.recv().await {
            if ws_sender
                .send(Message::Binary(data.into()))
                .await
                .is_err()
            {
                break;
            }
        }
    });

    // Main loop: WebSocket input → PTY
    while let Some(msg) = ws_receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                if let Ok(client_msg) = serde_json::from_str::<ClientMessage>(&text) {
                    match client_msg {
                        ClientMessage::Input { data } => {
                            let mut w = master_writer.lock().await;
                            let _ = w.write_all(data.as_bytes());
                        }
                        ClientMessage::Resize { cols, rows } => {
                            let _ = pty_pair.master.resize(PtySize {
                                rows,
                                cols,
                                pixel_width: 0,
                                pixel_height: 0,
                            });
                        }
                    }
                } else {
                    let mut w = master_writer.lock().await;
                    let _ = w.write_all(text.as_bytes());
                }
            }
            Ok(Message::Binary(bytes)) => {
                let mut w = master_writer.lock().await;
                let _ = w.write_all(&bytes);
            }
            Ok(Message::Close(_)) | Err(_) => break,
            _ => {}
        }
    }

    pty_to_ws.abort();
    let _ = child.kill();

    Ok(())
}
