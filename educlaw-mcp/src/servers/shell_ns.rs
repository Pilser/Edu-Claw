// educlaw-mcp/src/servers/shell_ns.rs
//
// Tools:
//   run_command  — run a shell command inside the student's Linux namespace
//   run_python   — run a Python snippet inside the student's namespace
//
// How it works:
//   1. Reads EDUCLAW_SESSION_ID from env (set by the session broker at spawn)
//   2. Queries GET {EDUCLAW_SERVER_URL}/sessions/{session_id} to get namespace_pid
//   3. Executes: nsenter --mount --pid -t {namespace_pid} -- <command>
//      This runs the command inside the same mount + PID namespace as the student's claw process.
//
// Security:
//   - Commands run as the same unprivileged uid used for the namespace (no privilege escalation)
//   - The OverlayFS mount means writes land in the student's upper dir only
//   - Timeout enforced per-call (default SHELL_TIMEOUT_S=30)
//   - No network namespace entry — student's NET ns is isolated

use std::sync::Arc;
use std::time::Duration;

use rmcp::{model::CallToolResult, model::Content, ErrorData as McpError};
use serde::Deserialize;
use serde_json::Value;
use tokio::process::Command;
use tokio::time::timeout;
use tracing::debug;

use crate::config::Config;

// ── Tool definitions ───────────────────────────────────────────────────────

pub fn tools() -> Vec<rmcp::model::Tool> {
    vec![
        crate::make_tool(
            "run_command",
            "Run a shell command inside the student's isolated Linux namespace. \
             The command executes in the student's OverlayFS workspace — writes \
             persist to their private upper layer. \
             stdout and stderr are returned. Timeout 30s.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "Shell command to run (executed via sh -c)"
                    },
                    "timeout_secs": {
                        "type": "integer",
                        "description": "Max seconds to wait (default 30, max 120)"
                    }
                },
                "required": ["command"]
            }),
        ),
        crate::make_tool(
            "run_python",
            "Run a Python 3 snippet inside the student's isolated Linux namespace. \
             Has access to numpy, sympy, matplotlib, scipy, pandas and all other \
             pre-installed libraries. \
             Saves any plots to the student's workspace automatically. \
             Returns stdout, stderr, and a list of generated image paths.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "code": {
                        "type": "string",
                        "description": "Python 3 source code to execute"
                    },
                    "timeout_secs": {
                        "type": "integer",
                        "description": "Max seconds to wait (default 30, max 120)"
                    }
                },
                "required": ["code"]
            }),
        ),
    ]
}

// ── Dispatcher ─────────────────────────────────────────────────────────────

pub async fn call_tool(
    name: &str,
    args: &serde_json::Map<String, Value>,
    config: &Arc<Config>,
) -> Result<CallToolResult, McpError> {
    match name {
        "run_command" => {
            let command = get_str(args, "command")?;
            let secs = args.get("timeout_secs").and_then(|v| v.as_u64()).unwrap_or(30).min(120);
            run_in_namespace(&command, secs, config).await
        }
        "run_python" => {
            let code = get_str(args, "code")?;
            let secs = args.get("timeout_secs").and_then(|v| v.as_u64()).unwrap_or(30).min(120);
            run_python_in_namespace(&code, secs, config).await
        }
        _ => unreachable!("unknown shell_ns tool: {name}"),
    }
}

// ── Implementation ─────────────────────────────────────────────────────────

/// Run `sh -c <command>` inside the student's namespace via nsenter.
async fn run_in_namespace(
    command: &str,
    timeout_secs: u64,
    config: &Arc<Config>,
) -> Result<CallToolResult, McpError> {
    let pid = namespace_pid(config).await?;
    debug!("nsenter pid={pid} command={command:?}");

    let result = timeout(
        Duration::from_secs(timeout_secs),
        Command::new("nsenter")
            .args(["--mount", "--pid", "-t", &pid.to_string(), "--", "sh", "-c", command])
            .output(),
    )
    .await
    .map_err(|_| McpError::internal_error(
        format!("command timed out after {timeout_secs}s"), None,
    ))?
    .map_err(|e| McpError::internal_error(format!("nsenter failed: {e}"), None))?;

    let stdout = String::from_utf8_lossy(&result.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&result.stderr).into_owned();
    let exit_code = result.status.code().unwrap_or(-1);

    let out = serde_json::json!({
        "exit_code": exit_code,
        "stdout": stdout,
        "stderr": stderr,
    });
    Ok(CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(&out).unwrap_or_default(),
    )]))
}

/// Run Python code inside the student's namespace.
/// Wraps the code to capture generated matplotlib figures automatically.
async fn run_python_in_namespace(
    code: &str,
    timeout_secs: u64,
    config: &Arc<Config>,
) -> Result<CallToolResult, McpError> {
    let student = config.student_info().ok_or_else(|| {
        McpError::internal_error("no student context — cannot run Python", None)
    })?;

    // Wrap user code: set Agg backend + save all figures to workspace
    let wrapper = format!(
        r#"
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as _plt
import os as _os
import json as _json

_figures_saved = []
_orig_show = _plt.show

def _patched_show(*a, **kw):
    n = len(_figures_saved) + 1
    path = _os.path.join(_os.environ.get('EDUCLAW_WORKSPACE', '/tmp'), f'figure_{{n}}.png')
    _plt.savefig(path, dpi=120, bbox_inches='tight')
    _figures_saved.append(path)
    _plt.close()

_plt.show = _patched_show

# ── student code ──────────────────────────────────────────────────────────
{}
# ── end student code ──────────────────────────────────────────────────────

# Auto-save any open figures not shown explicitly
for _fn in _plt.get_fignums():
    _patched_show()

print('\n__EDUCLAW_FIGURES__' + _json.dumps(_figures_saved))
"#,
        code
    );

    // Write wrapper to a temp file in the student's upper dir so nsenter can read it
    let upper_dir = format!("{}/{}/upper", config.students_root, student.student_id);
    let script_path = format!("{upper_dir}/.educlaw_run.py");
    std::fs::create_dir_all(&upper_dir)
        .map_err(|e| McpError::internal_error(format!("mkdir: {e}"), None))?;
    std::fs::write(&script_path, &wrapper)
        .map_err(|e| McpError::internal_error(format!("write script: {e}"), None))?;

    let command = format!("python3 {script_path}");
    let pid = namespace_pid(config).await?;

    let result = timeout(
        Duration::from_secs(timeout_secs),
        Command::new("nsenter")
            .args(["--mount", "--pid", "-t", &pid.to_string(), "--", "sh", "-c", &command])
            .env("EDUCLAW_WORKSPACE", format!("{upper_dir}"))
            .output(),
    )
    .await
    .map_err(|_| McpError::internal_error(
        format!("python timed out after {timeout_secs}s"), None,
    ))?
    .map_err(|e| McpError::internal_error(format!("nsenter failed: {e}"), None))?;

    // Clean up temp script
    let _ = std::fs::remove_file(&script_path);

    let combined = String::from_utf8_lossy(&result.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&result.stderr).into_owned();
    let exit_code = result.status.code().unwrap_or(-1);

    // Extract figure list injected by wrapper
    let (stdout, figures) = if let Some(idx) = combined.rfind("__EDUCLAW_FIGURES__") {
        let text = combined[..idx].trim_end().to_string();
        let json_str = &combined[idx + "__EDUCLAW_FIGURES__".len()..];
        let figs: Vec<String> = serde_json::from_str(json_str.trim()).unwrap_or_default();
        (text, figs)
    } else {
        (combined, vec![])
    };

    let out = serde_json::json!({
        "exit_code": exit_code,
        "stdout": stdout,
        "stderr": stderr,
        "figures": figures,
    });
    Ok(CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(&out).unwrap_or_default(),
    )]))
}

// ── Namespace PID lookup ───────────────────────────────────────────────────

/// Fetch the namespace_pid from the educlaw-server REST API using the current session_id.
async fn namespace_pid(config: &Arc<Config>) -> Result<i64, McpError> {
    let session_id = std::env::var("EDUCLAW_SESSION_ID").map_err(|_| {
        McpError::internal_error("EDUCLAW_SESSION_ID not set — cannot locate namespace", None)
    })?;

    let url = format!("{}/sessions/{session_id}", config.server_url);
    let resp = reqwest::get(&url)
        .await
        .map_err(|e| McpError::internal_error(format!("GET {url} failed: {e}"), None))?;

    if !resp.status().is_success() {
        return Err(McpError::internal_error(
            format!("session lookup returned {}", resp.status()), None,
        ));
    }

    #[derive(Deserialize)]
    struct SessionResp {
        namespace_pid: Option<i64>,
    }
    let body: SessionResp = resp.json().await.map_err(|e| {
        McpError::internal_error(format!("parse session response: {e}"), None)
    })?;

    body.namespace_pid.ok_or_else(|| {
        McpError::internal_error(
            "namespace_pid is null — session may not have spawned claw yet", None,
        )
    })
}

// ── Helper ─────────────────────────────────────────────────────────────────

fn get_str(args: &serde_json::Map<String, Value>, key: &str) -> Result<String, McpError> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| McpError::invalid_params(format!("missing '{key}'"), None))
}
