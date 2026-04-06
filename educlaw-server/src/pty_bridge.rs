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
//
// Chat panel routing (added for hybrid UI):
//   When the user sends a message (ClientMessage::Input), we set a flag
//   and route ANSI-stripped PTY text as {"type":"chat_text","text":"..."} frames.
//   When the claw REPL prompt ("> ") appears in the PTY stream, we emit
//   {"type":"chat_end"} and stop accumulating until the next user input.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use futures_util::{SinkExt, StreamExt};
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use serde::Deserialize;
use tokio::sync::{mpsc, Mutex};

use crate::error::{AppError, Result};
use crate::session::broker::now_secs;

// ---------------------------------------------------------------------------
// OSC envelope interceptor
// OSC sentinel format written by the Python shell wrapper:
//   ESC ] 99 ; {json} BEL   (\x1b]99;...\x07)
// ---------------------------------------------------------------------------

const OSC_PREFIX: &[u8] = b"\x1b]99;";
const OSC_BEL: u8 = 0x07;

/// Split raw PTY bytes into (clean_terminal_bytes, extracted_envelopes).
/// `buf` is a carry-over buffer; call this with the same `buf` on every chunk
/// so that OSC sequences spanning two read calls are handled correctly.
fn split_pty_output(raw: &[u8], buf: &mut Vec<u8>) -> (Vec<u8>, Vec<serde_json::Value>) {
    buf.extend_from_slice(raw);
    let mut clean = Vec::new();
    let mut envelopes: Vec<serde_json::Value> = Vec::new();
    let mut pos = 0;

    loop {
        match find_bytes(&buf[pos..], OSC_PREFIX) {
            Some(start) => {
                let abs_start = pos + start;
                clean.extend_from_slice(&buf[pos..abs_start]);
                let body_start = abs_start + OSC_PREFIX.len();
                match buf[body_start..].iter().position(|&b| b == OSC_BEL) {
                    Some(bel_offset) => {
                        let body = &buf[body_start..body_start + bel_offset];
                        if let Ok(val) = serde_json::from_slice::<serde_json::Value>(body) {
                            envelopes.push(val);
                        }
                        pos = body_start + bel_offset + 1;
                    }
                    None => {
                        // BEL not yet received; keep everything from abs_start
                        let remaining = buf[abs_start..].to_vec();
                        *buf = remaining;
                        return (clean, envelopes);
                    }
                }
            }
            None => {
                // No prefix found, but the tail might be a partial prefix
                let safe_len = safe_output_len(&buf[pos..]);
                clean.extend_from_slice(&buf[pos..pos + safe_len]);
                *buf = buf[pos + safe_len..].to_vec();
                return (clean, envelopes);
            }
        }
    }
}

/// Find the first occurrence of `needle` in `haystack`.
fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Number of bytes from `data` that are safe to emit now, i.e., we exclude
/// any suffix of `data` that is a proper prefix of `OSC_PREFIX` — those bytes
/// might form the start of a future OSC sequence.
fn safe_output_len(data: &[u8]) -> usize {
    for pfx_len in (1..=OSC_PREFIX.len().min(data.len())).rev() {
        if data.ends_with(&OSC_PREFIX[..pfx_len]) {
            return data.len() - pfx_len;
        }
    }
    data.len()
}

// ---------------------------------------------------------------------------
// ANSI stripping + chat turn detection
// ---------------------------------------------------------------------------

/// Remove ANSI/VT100 escape sequences from a string to produce clean text.
/// Handles CSI sequences (\x1b[...X), OSC sequences (\x1b]...\x07 or \x1b]...\x1b\\),
/// and simple two-byte sequences (\x1b X).
fn strip_ansi(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == 0x1b {
            i += 1;
            if i >= bytes.len() { break; }
            match bytes[i] {
                b'[' => {
                    // CSI: skip until final byte (0x40–0x7e)
                    i += 1;
                    while i < bytes.len() && !(0x40..=0x7eu8).contains(&bytes[i]) {
                        i += 1;
                    }
                    i += 1;
                }
                b']' => {
                    // OSC: skip until BEL (0x07) or ST (\x1b\\)
                    i += 1;
                    while i < bytes.len() {
                        if bytes[i] == 0x07 { i += 1; break; }
                        if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i+1] == b'\\' {
                            i += 2; break;
                        }
                        i += 1;
                    }
                }
                _ => { i += 1; } // skip 2-byte sequence
            }
        } else if bytes[i] == 0x07 || bytes[i] == 0x08 || bytes[i] == 0x0c {
            // BEL, BS, FF — skip
            i += 1;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Returns true when `text` (ANSI-stripped) ends with the claw REPL prompt
/// `> ` on a new line — indicating Claude has finished and is awaiting input.
fn ends_with_repl_prompt(text: &str) -> bool {
    // The claw REPL prompt is "> " (from LineEditor::new("> ", ...)).
    // After a response, the PTY will contain "...response text\r\n> " or similar.
    let t = text.trim_end_matches(' ');
    t.ends_with("\n>") || t.ends_with("\r>") || t == ">"
}

/// Map an image file extension to a MIME type for the data-URL.
fn mime_for_image(path: &Path) -> &'static str {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        _ => "application/octet-stream",
    }
}

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
    env_vars: Vec<(String, String)>,
    model: Option<String>,
    last_input_at: Arc<AtomicU64>,
    mut broker_rx: mpsc::Receiver<Vec<u8>>,
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
    cmd.cwd(&workspace);
    cmd.env("CLAW_CONFIG_HOME", config_home.display().to_string());
    for (k, v) in &env_vars {
        cmd.env(k, v);
    }

    // ── DEBUG: log what we're about to exec ───────────────────────────────
    tracing::info!(
        bin  = %claw_bin.display(),
        model = ?model,
        workspace = %workspace.display(),
        xai_key_forwarded = env_vars.iter().any(|(k,_)| k == "XAI_API_KEY"),
        "PTY: spawning claw"
    );

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

    // ── Chat routing state ─────────────────────────────────────────────────
    // `chat_active`: true while we're waiting for Claude's response.
    // `ctrl_tx`: sends chat control frames (chat_start) from the input loop
    //            to the pty_to_ws task, which owns ws_sender.
    let chat_active = Arc::new(AtomicBool::new(false));
    let chat_active_pty = Arc::clone(&chat_active);
    let (ctrl_tx, mut ctrl_rx) = mpsc::channel::<String>(8);

    // Task: broker → PTY (watchdog warnings, teacher messages)
    let broker_writer = Arc::clone(&master_writer);
    let broker_fwd = tokio::spawn(async move {
        while let Some(data) = broker_rx.recv().await {
            let mut w = broker_writer.lock().await;
            let _ = w.write_all(&data);
        }
    });

    // Task: PTY output → WebSocket  (also handles chat_text routing)
    let osc_workspace = workspace.clone();
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

        let mut osc_buf: Vec<u8> = Vec::new();
        // Accumulator for ANSI-stripped text during an AI turn.
        let mut chat_clean_buf = String::new();

        'outer: loop {
            tokio::select! {
                // Forward chat control frames (chat_start) from the input loop.
                Some(ctrl_msg) = ctrl_rx.recv() => {
                    if ws_sender.send(Message::Text(ctrl_msg)).await.is_err() {
                        break 'outer;
                    }
                }
                // Process PTY output bytes.
                Some(data) = rx.recv() => {
                    let (clean, envelopes) = split_pty_output(&data, &mut osc_buf);

                    // ── Chat panel routing ──────────────────────────────
                    if chat_active_pty.load(Ordering::Relaxed) && !clean.is_empty() {
                        let text_chunk = strip_ansi(
                            &String::from_utf8_lossy(&clean)
                        );
                        if !text_chunk.is_empty() {
                            chat_clean_buf.push_str(&text_chunk);

                            // Stream the chunk to the chat panel.
                            let frame = serde_json::json!({
                                "type": "chat_text",
                                "text": text_chunk
                            });
                            if let Ok(json) = serde_json::to_string(&frame) {
                                if ws_sender.send(Message::Text(json)).await.is_err() {
                                    break 'outer;
                                }
                            }

                            // Detect the claw REPL prompt: turn is over.
                            if ends_with_repl_prompt(&chat_clean_buf) {
                                chat_active_pty.store(false, Ordering::Relaxed);
                                chat_clean_buf.clear();
                                let end_frame = r#"{"type":"chat_end"}"#.to_string();
                                if ws_sender.send(Message::Text(end_frame)).await.is_err() {
                                    break 'outer;
                                }
                            }
                        }
                    }

                    // ── Always forward raw PTY bytes to xterm.js ────────
                    if !clean.is_empty()
                        && ws_sender
                            .send(Message::Binary(clean.into()))
                            .await
                            .is_err()
                    {
                        break 'outer;
                    }

                    // ── Forward OSC envelopes as JSON frames ────────────
                    for env in envelopes {
                        let json_msg =
                            if env.get("type").and_then(|t| t.as_str()) == Some("show_image") {
                                let path_str = match env
                                    .get("payload")
                                    .and_then(|p| p.get("path"))
                                    .and_then(|p| p.as_str())
                                {
                                    Some(s) => s.to_owned(),
                                    None => continue,
                                };
                                let img_path = if Path::new(&path_str).is_absolute() {
                                    PathBuf::from(&path_str)
                                } else {
                                    osc_workspace.join(&path_str)
                                };
                                match tokio::fs::read(&img_path).await {
                                    Ok(bytes) => serde_json::json!({
                                        "type": "show_image",
                                        "payload": {
                                            "path": path_str,
                                            "data_url": format!(
                                                "data:{};base64,{}",
                                                mime_for_image(&img_path),
                                                STANDARD.encode(&bytes)
                                            )
                                        }
                                    }),
                                    Err(_) => continue,
                                }
                            } else {
                                env
                            };

                        if let Ok(text) = serde_json::to_string(&json_msg) {
                            if ws_sender.send(Message::Text(text)).await.is_err() {
                                break 'outer;
                            }
                        }
                    }
                }
                else => break 'outer,
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
                            last_input_at.store(now_secs(), Ordering::Relaxed);
                            // Signal chat panel: Claude is about to respond.
                            if !data.trim().is_empty() {
                                chat_active.store(true, Ordering::Relaxed);
                                let _ = ctrl_tx
                                    .send(r#"{"type":"chat_start"}"#.to_string())
                                    .await;
                            }
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
                }
                // Unknown JSON frames (e.g. {"type":"ping"}) are intentionally dropped.
            }
            Ok(Message::Binary(bytes)) => {
                last_input_at.store(now_secs(), Ordering::Relaxed);
                let mut w = master_writer.lock().await;
                let _ = w.write_all(&bytes);
            }
            Ok(Message::Close(_)) | Err(_) => break,
            _ => {}
        }
    }

    broker_fwd.abort();
    pty_to_ws.abort();
    let _ = child.kill();

    Ok(())
}
