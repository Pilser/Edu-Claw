// educlaw-server/src/pty_bridge.rs
//
// Pipe-based bridge between claw (stdin/stdout) and the WebSocket client.
//
// Design:
//   1. Spawn `claw --dangerously-skip-permissions [--model M]` with piped
//      stdin, stdout, and stderr (no PTY).
//   2. stdin: we write one line per user message ("text\n").
//      claw's input.rs detects stdin is not a terminal and falls back to
//      plain read_line() — no carriage-return tricks required.
//   3. stdout: we read chunks and forward them as:
//      a. Binary frames for xterm.js (raw bytes, ANSI included).
//      b. {"type":"chat_text","text":"..."} JSON frames (ANSI-stripped)
//         for the chat panel.
//   4. Turn detection: accumulated ANSI-stripped output contains "✨ Done"
//      AND ends with "> " (the read_line_fallback prompt).
//      On detection we emit {"type":"chat_end"}.
//   5. stderr: drained and logged at debug level.
//   6. broker_rx: drained without forwarding (no PTY terminal to write to).
//   7. Permission prompts suppressed via --dangerously-skip-permissions.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
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

/// A positionally-ordered output event from a single read chunk.
/// Unlike `split_output` which returns (all_clean, all_envelopes), this
/// preserves the interleaved order so that `inside_tool` state can be
/// tracked correctly even when OSC envelopes and clean bytes land in the
/// same read() call.
enum OutputChunk {
    Clean(Vec<u8>),
    Envelope(serde_json::Value),
}

fn split_output_ordered(raw: &[u8], buf: &mut Vec<u8>) -> Vec<OutputChunk> {
    buf.extend_from_slice(raw);
    let mut result = Vec::new();
    let mut pos = 0;

    loop {
        match find_bytes(&buf[pos..], OSC_PREFIX) {
            Some(start) => {
                let abs_start = pos + start;
                if abs_start > pos {
                    result.push(OutputChunk::Clean(buf[pos..abs_start].to_vec()));
                }
                let body_start = abs_start + OSC_PREFIX.len();
                match buf[body_start..].iter().position(|&b| b == OSC_BEL) {
                    Some(bel_offset) => {
                        let body = &buf[body_start..body_start + bel_offset];
                        if let Ok(val) = serde_json::from_slice::<serde_json::Value>(body) {
                            result.push(OutputChunk::Envelope(val));
                        }
                        pos = body_start + bel_offset + 1;
                    }
                    None => {
                        // BEL not yet received; hold from abs_start onward.
                        let remaining = buf[abs_start..].to_vec();
                        *buf = remaining;
                        return result;
                    }
                }
            }
            None => {
                let safe_len = safe_output_len(&buf[pos..]);
                if safe_len > 0 {
                    result.push(OutputChunk::Clean(buf[pos..pos + safe_len].to_vec()));
                }
                *buf = buf[pos + safe_len..].to_vec();
                return result;
            }
        }
    }
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

// ---------------------------------------------------------------------------
// Structured text routing
// When EDUCLAW_STRUCTURED_OUTPUT=1 claw suppresses spinner/box-drawing output
// and emits OSC 99 envelopes for every typed event.  Non-OSC bytes while a
// chat turn is active are forwarded as assistant_text; only the bare REPL
// prompt needs to be filtered.
// ---------------------------------------------------------------------------

/// Drop the bare REPL prompt that claw writes after every turn.
fn is_noise_line(line: &str) -> bool {
    line.trim() == ">" || line.trim() == "> "
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
    Cancel,
}

/// Spawn `claw` with piped stdio and bridge its I/O to the WebSocket.
/// The function signature is identical to the old PTY-based version so
/// `ws.rs` requires no changes.
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
    use std::process::Stdio;

    // When claw is running in structured OSC mode every assistant token is
    // already delivered as a typed envelope; the Clean-bytes fallback path
    // must NOT also emit assistant_text or each token appears twice.
    let structured_mode = env_vars
        .iter()
        .any(|(k, v)| k == "EDUCLAW_STRUCTURED_OUTPUT" && v == "1");

    let mut cmd = tokio::process::Command::new(&claw_bin);
    cmd.arg("--dangerously-skip-permissions");
    if let Some(ref m) = model {
        cmd.arg("--model");
        cmd.arg(m);
    }
    cmd.current_dir(&workspace);
    // Start with a clean environment; forward only what we explicitly provide.
    cmd.env_clear();
    cmd.env("CLAW_CONFIG_HOME", config_home.display().to_string());
    for (k, v) in &env_vars {
        cmd.env(k, v);
    }
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    tracing::info!(
        bin = %claw_bin.display(),
        model = ?model,
        workspace = %workspace.display(),
        xai_key_forwarded = env_vars.iter().any(|(k, _)| k == "XAI_API_KEY"),
        "pipe: spawning claw"
    );

    let mut child = cmd
        .spawn()
        .map_err(|e| AppError::Session(format!("failed to spawn claw: {e}")))?;

    // Capture PID before taking stdio handles — used by the Cancel handler.
    let child_pid: Option<u32> = child.id();

    let child_stdin = child.stdin.take().expect("stdin piped");
    let child_stdout = child.stdout.take().expect("stdout piped");
    let child_stderr = child.stderr.take().expect("stderr piped");

    let stdin = Arc::new(Mutex::new(child_stdin));
    let (mut ws_sender, mut ws_receiver) = ws.split();

    // State: is there an active AI turn we are routing to the chat panel?
    let chat_active = Arc::new(AtomicBool::new(false));
    let chat_active_out = Arc::clone(&chat_active);

    // Channel: input-loop → stdout-task for control frames (chat_start).
    let (ctrl_tx, mut ctrl_rx) = mpsc::channel::<String>(16);

    // Drain broker_rx — watchdog/teacher messages written to PTY in the old
    // design have no equivalent target in pipe mode.
    tokio::spawn(async move {
        while broker_rx.recv().await.is_some() {}
    });

    // Drain stderr; log at debug level so we can diagnose claw errors.
    tokio::spawn(async move {
        use tokio::io::AsyncBufReadExt;
        let mut lines = tokio::io::BufReader::new(child_stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            tracing::debug!(target: "claw_stderr", "{}", line);
        }
    });

    let osc_workspace = workspace.clone();

    // --- Task: stdout → WebSocket -----------------------------------------
    let stdout_task = tokio::spawn(async move {
        let mut reader = tokio::io::BufReader::new(child_stdout);
        let mut raw_buf = [0u8; 4096];
        let mut osc_buf: Vec<u8> = Vec::new();
        // True between tool_start and tool_end OSC envelopes; suppresses
        // non-OSC stdout from being forwarded as assistant_text so that
        // raw tool output (ls listings, file contents, etc.) only appears
        // in the terminal panel, not the chat panel.
        let mut inside_tool = false;

        'outer: loop {
            tokio::select! {
                // Forward chat_start; reset parser so each new turn starts clean.
                Some(ctrl_msg) = ctrl_rx.recv() => {
                    inside_tool = false;
                    if ws_sender.send(Message::Text(ctrl_msg)).await.is_err() {
                        break 'outer;
                    }
                }

                // Read a chunk of stdout from claw.
                result = reader.read(&mut raw_buf) => {
                    let n = match result {
                        Ok(0) | Err(_) => break 'outer,
                        Ok(n) => n,
                    };

                    let chunks = split_output_ordered(&raw_buf[..n], &mut osc_buf);

                    for chunk in chunks {
                        match chunk {
                            OutputChunk::Clean(ref bytes) => {
                                // Always forward raw bytes to xterm.js.
                                if ws_sender
                                    .send(Message::Binary(bytes.clone().into()))
                                    .await
                                    .is_err()
                                {
                                    break 'outer;
                                }

                                // Forward as assistant_text only in non-structured mode
                                // (fallback for plain bytes) AND when chat is active
                                // AND we are NOT inside a tool execution.
                                // In structured mode every token arrives as an OSC
                                // assistant_text envelope — forwarding raw bytes too
                                // would cause every token to appear twice in the chat.
                                if !structured_mode && chat_active_out.load(Ordering::Relaxed) && !inside_tool {
                                    let raw_str = String::from_utf8_lossy(bytes);
                                    let text: String = strip_ansi(&raw_str)
                                        .lines()
                                        .filter(|l| !is_noise_line(l))
                                        .collect::<Vec<_>>()
                                        .join("\n");
                                    let text = text.trim_end_matches('\n').to_string();
                                    if !text.trim().is_empty() {
                                        let frame = serde_json::json!({
                                            "type": "assistant_text",
                                            "text": format!("{text}\n")
                                        });
                                        if ws_sender
                                            .send(Message::Text(frame.to_string()))
                                            .await
                                            .is_err()
                                        {
                                            break 'outer;
                                        }
                                    }
                                }
                            }

                            OutputChunk::Envelope(env) => {
                                // Dispatch typed WS frames; update inside_tool state.
                                let json_msg: Option<serde_json::Value> = match env
                                    .get("type")
                                    .and_then(|t| t.as_str())
                                {
                                    Some("thinking") => {
                                        Some(serde_json::json!({ "type": "thinking" }))
                                    }
                                    Some("tool_start") => {
                                        inside_tool = true;
                                        Some(env)
                                    }
                                    Some("tool_end") => {
                                        inside_tool = false;
                                        Some(env)
                                    }
                                    Some("assistant_text") => Some(env),
                                    Some("chat_end") => {
                                        chat_active_out.store(false, Ordering::Relaxed);
                                        inside_tool = false;
                                        Some(serde_json::json!({ "type": "chat_end" }))
                                    }
                                    Some("chat_error") => {
                                        chat_active_out.store(false, Ordering::Relaxed);
                                        inside_tool = false;
                                        Some(env)
                                    }
                                    Some("show_image") => {
                                        let path_str = match env
                                            .get("payload")
                                            .and_then(|p| p.get("path"))
                                            .and_then(|p| p.as_str())
                                        {
                                            Some(s) => s.to_owned(),
                                            None => { continue; }
                                        };
                                        let img_path = if Path::new(&path_str).is_absolute() {
                                            PathBuf::from(&path_str)
                                        } else {
                                            osc_workspace.join(&path_str)
                                        };
                                        match tokio::fs::read(&img_path).await {
                                            Ok(img_bytes) => Some(serde_json::json!({
                                                "type": "show_image",
                                                "payload": {
                                                    "path": path_str,
                                                    "data_url": format!(
                                                        "data:{};base64,{}",
                                                        mime_for_image(&img_path),
                                                        STANDARD.encode(&img_bytes)
                                                    )
                                                }
                                            })),
                                            Err(_) => None,
                                        }
                                    }
                                    _ => None,
                                };

                                if let Some(msg) = json_msg {
                                    if let Ok(text) = serde_json::to_string(&msg) {
                                        if ws_sender.send(Message::Text(text)).await.is_err() {
                                            break 'outer;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                else => break 'outer,
            }
        }
    });

    // --- Main loop: WebSocket input → claw stdin --------------------------
    while let Some(msg) = ws_receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                if let Ok(client_msg) = serde_json::from_str::<ClientMessage>(&text) {
                    match client_msg {
                        ClientMessage::Input { data } => {
                            last_input_at.store(now_secs(), Ordering::Relaxed);
                            if !data.trim().is_empty() {
                                chat_active.store(true, Ordering::Relaxed);
                                let _ = ctrl_tx
                                    .send(r#"{"type":"chat_start"}"#.to_string())
                                    .await;
                            }
                            // Ensure the line ends with a newline so claw's
                            // read_line_fallback returns immediately.
                            let mut line = data;
                            if !line.ends_with('\n') {
                                line.push('\n');
                            }
                            let mut w = stdin.lock().await;
                            if w.write_all(line.as_bytes()).await.is_err() {
                                break;
                            }
                        }
                        ClientMessage::Resize { .. } => {
                            // No PTY: resize messages are silently ignored.
                        }
                        ClientMessage::Cancel => {
                            // Send SIGINT to the claw process.  The ctrl_c()
                            // handler inside claw calls abort_signal.abort()
                            // and sets the educlaw_cancel_flag, which stops
                            // the active HTTP stream on the next event.
                            if let Some(pid) = child_pid {
                                let _ = tokio::process::Command::new("kill")
                                    .args(["-s", "INT", &pid.to_string()])
                                    .status()
                                    .await;
                            }
                        }
                    }
                }
                // Unknown frames (ping, etc.) are silently dropped.
            }
            Ok(Message::Binary(bytes)) => {
                // Raw binary input: write directly to claw's stdin.
                last_input_at.store(now_secs(), Ordering::Relaxed);
                let mut w = stdin.lock().await;
                if w.write_all(&bytes).await.is_err() {
                    break;
                }
            }
            Ok(Message::Close(_)) | Err(_) => break,
            _ => {}
        }
    }

    stdout_task.abort();
    let _ = child.kill().await;
    Ok(())
}
