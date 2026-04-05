// educlaw-mcp/src/servers/student_files.rs
//
// Tools:
//   list_student_files  — directory tree of the student's upper layer
//   read_student_file   — read a file from the student's workspace
//   write_student_file  — write / overwrite a file in the student's workspace
//
// All paths are constrained to EDUCLAW_STUDENTS_ROOT/{student_id}/upper/.
// Path traversal (../.. etc.) is rejected.

use std::path::{Path, PathBuf};

use rmcp::{model::CallToolResult, model::Content, ErrorData as McpError};
use serde_json::Value;
use walkdir::WalkDir;

use crate::config::StudentInfo;

pub fn tools() -> Vec<rmcp::model::Tool> {
    vec![
        crate::make_tool(
            "list_student_files",
            "List all files in the student's private workspace as a JSON tree.",
            serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        ),
        crate::make_tool(
            "read_student_file",
            "Read the content of a file from the student's private workspace. \
             Path is relative to the workspace root.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path relative to student workspace" }
                },
                "required": ["path"]
            }),
        ),
        crate::make_tool(
            "write_student_file",
            "Write content to a file in the student's private workspace. \
             Creates parent directories if needed. Overwrites existing files.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path":    { "type": "string", "description": "File path relative to student workspace" },
                    "content": { "type": "string", "description": "Text content to write" }
                },
                "required": ["path", "content"]
            }),
        ),
    ]
}

pub fn call_tool(
    name: &str,
    args: &serde_json::Map<String, Value>,
    students_root: &str,
    student: &Option<StudentInfo>,
) -> Result<CallToolResult, McpError> {
    let s = student
        .as_ref()
        .ok_or_else(|| McpError::internal_error("no student context", None))?;

    let base = student_workspace(students_root, &s.student_id);

    match name {
        "list_student_files" => list_files(&base),
        "read_student_file" => {
            let path = get_str(args, "path")?;
            read_file(&base, &path)
        }
        "write_student_file" => {
            let path = get_str(args, "path")?;
            let content = get_str(args, "content")?;
            write_file(&base, &path, &content)
        }
        _ => unreachable!("unknown student_files tool: {name}"),
    }
}

// ── Implementations ─────────────────────────────────────────────────────────

fn list_files(base: &Path) -> Result<CallToolResult, McpError> {
    #[derive(serde::Serialize)]
    struct Entry {
        path: String,
        size: u64,
        kind: &'static str,
    }

    let entries: Vec<Entry> = WalkDir::new(base)
        .into_iter()
        .filter_map(|e| e.ok())
        .skip(1)
        .map(|e| {
            let rel = e.path().strip_prefix(base).unwrap().display().to_string();
            let meta = e.metadata().ok();
            Entry {
                path: rel,
                size: meta.map(|m| m.len()).unwrap_or(0),
                kind: if e.file_type().is_dir() { "dir" } else { "file" },
            }
        })
        .collect();

    let out = serde_json::to_string_pretty(&entries)
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;
    Ok(CallToolResult::success(vec![Content::text(out)]))
}

fn read_file(base: &Path, rel_path: &str) -> Result<CallToolResult, McpError> {
    let resolved = safe_resolve(base, rel_path)?;
    if resolved.is_dir() {
        return Err(McpError::invalid_params("path is a directory", None));
    }
    let content = std::fs::read_to_string(&resolved)
        .map_err(|e| McpError::internal_error(format!("read: {e}"), None))?;
    Ok(CallToolResult::success(vec![Content::text(content)]))
}

fn write_file(base: &Path, rel_path: &str, content: &str) -> Result<CallToolResult, McpError> {
    // Validate path before creating parent dirs
    let joined = base.join(rel_path);
    // Normalise without canonicalize (file may not exist yet)
    let components: Vec<_> = joined.components().collect();
    let resolved = PathBuf::from_iter(components);

    // Reject if the resolved path escapes base
    if !resolved.starts_with(base) {
        return Err(McpError::invalid_params("path traversal denied", None));
    }

    if let Some(parent) = resolved.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| McpError::internal_error(format!("mkdir: {e}"), None))?;
    }

    std::fs::write(&resolved, content)
        .map_err(|e| McpError::internal_error(format!("write: {e}"), None))?;

    let size = content.len();
    Ok(CallToolResult::success(vec![Content::text(format!(
        "Written {size} bytes to {rel_path}"
    ))]))
}

// ── Helpers ────────────────────────────────────────────────────────────────

fn student_workspace(students_root: &str, student_id: &str) -> PathBuf {
    // /data/students/{id}/upper  — the OverlayFS upper layer
    Path::new(students_root).join(student_id).join("upper")
}

fn safe_resolve(base: &Path, rel_path: &str) -> Result<PathBuf, McpError> {
    let joined = base.join(rel_path);
    let resolved = joined
        .canonicalize()
        .map_err(|e| McpError::invalid_params(format!("path not found: {e}"), None))?;
    if !resolved.starts_with(base) {
        return Err(McpError::invalid_params("path traversal denied", None));
    }
    Ok(resolved)
}

fn get_str(
    args: &serde_json::Map<String, Value>,
    key: &str,
) -> Result<String, McpError> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| McpError::invalid_params(format!("missing '{key}'"), None))
}
