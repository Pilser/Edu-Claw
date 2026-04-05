// educlaw-mcp/src/servers/teacher_broadcast.rs
//
// Tool: broadcast_message
//
// Teachers push a plain-text message or hint to one or all student sessions.
// The message appears inline in the student's terminal as a highlighted banner.
//
// Requires: role == "teacher" or "admin" (enforced via EDUCLAW_STUDENT_JSON).
// Uses:     POST {EDUCLAW_SERVER_URL}/sessions/{session_id}/broadcast
//           Body: { "message": "...", "all": false }

use rmcp::{model::CallToolResult, model::Content, ErrorData as McpError};
use serde_json::Value;

use crate::config::{Config, StudentInfo};

// ── Tool definitions ───────────────────────────────────────────────────────

pub fn tools() -> Vec<rmcp::model::Tool> {
    vec![crate::make_tool(
        "broadcast_message",
        "Send a message or hint to a specific student's terminal, or to all \
         active sessions at once. The message appears as a highlighted banner \
         inside the student's terminal — no new UI needed. \
         Only available to teachers and admins.",
        serde_json::json!({
            "type": "object",
            "properties": {
                "message": {
                    "type": "string",
                    "description": "Text to display in the student terminal(s)."
                },
                "session_id": {
                    "type": "string",
                    "description": "Target session UUID. Omit (or set all=true) to broadcast to every active session."
                },
                "all": {
                    "type": "boolean",
                    "description": "If true, send to ALL active sessions regardless of session_id."
                }
            },
            "required": ["message"]
        }),
    )]
}

// ── Dispatcher ────────────────────────────────────────────────────────────

pub async fn call_tool(
    name: &str,
    args: &serde_json::Map<String, Value>,
    student: &Option<StudentInfo>,
    config: &Config,
) -> Result<CallToolResult, McpError> {
    match name {
        "broadcast_message" => handle_broadcast(args, student, config).await,
        _ => Err(McpError::invalid_params(format!("unknown tool: {name}"), None)),
    }
}

// ── Handler ────────────────────────────────────────────────────────────────

async fn handle_broadcast(
    args: &serde_json::Map<String, Value>,
    student: &Option<StudentInfo>,
    config: &Config,
) -> Result<CallToolResult, McpError> {
    // Enforce teacher or admin role.
    match student {
        Some(s) if s.role == "teacher" || s.role == "admin" => {}
        _ => {
            return Err(McpError::invalid_params(
                "broadcast_message is only available to teachers and admins",
                None,
            ))
        }
    }

    let message = args
        .get("message")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("missing 'message'", None))?;

    let all = args.get("all").and_then(|v| v.as_bool()).unwrap_or(false);

    let session_id = if all {
        // When all=true the server endpoint ignores the path param but still
        // needs a valid-looking segment — use "all" as a placeholder.
        "all".to_string()
    } else {
        args.get("session_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| {
                // Fall back to the teacher's own session_id from env.
                std::env::var("EDUCLAW_SESSION_ID").ok()
            })
            .ok_or_else(|| {
                McpError::invalid_params(
                    "provide 'session_id' or set 'all' to true",
                    None,
                )
            })?
    };

    // POST /sessions/{session_id}/broadcast
    let url = format!("{}/sessions/{session_id}/broadcast", config.server_url);

    // Teacher needs a token — we use EDUCLAW_TEACHER_TOKEN env if present,
    // otherwise fall back to EDUCLAW_SESSION_TOKEN (set by the broker).
    let token = std::env::var("EDUCLAW_TEACHER_TOKEN")
        .or_else(|_| std::env::var("EDUCLAW_SESSION_TOKEN"))
        .unwrap_or_default();

    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .bearer_auth(&token)
        .json(&serde_json::json!({ "message": message, "all": all }))
        .send()
        .await
        .map_err(|e| McpError::internal_error(format!("POST {url} failed: {e}"), None))?;

    if !resp.status().is_success() {
        return Err(McpError::internal_error(
            format!("broadcast returned {}", resp.status()),
            None,
        ));
    }

    let target = if all { "all sessions".to_string() } else { session_id };
    Ok(CallToolResult::success(vec![Content::text(format!(
        "Message sent to {target}."
    ))]))
}
