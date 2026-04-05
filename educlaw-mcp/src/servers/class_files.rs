// educlaw-mcp/src/servers/class_files.rs
//
// Tools:
//   list_class_files    — directory tree of /data/classes/{class_id}/
//   read_class_file     — read a specific file (path traversal enforced)
//   search_class_content— semantic KNN search over indexed chunks
//   index_class_file    — embed a markdown file into the search index
//
// All paths are constrained to EDUCLAW_CLASSES_ROOT/{class_id}/.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use rmcp::{model::CallToolResult, model::Content, ErrorData as McpError};
use serde_json::Value;
use tracing::{debug, info};
use walkdir::WalkDir;

use crate::embedding::EmbeddingClient;
use crate::search_index::SearchIndex;
use crate::config::StudentInfo;

// ── Tool definitions (input schemas) ───────────────────────────────────────

pub fn tools() -> Vec<rmcp::model::Tool> {
    vec![
        crate::make_tool(
            "list_class_files",
            "List all files in a class materials folder as a JSON tree. \
             Returns file names, sizes, and types.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "class_id": { "type": "string", "description": "Class identifier (e.g. 'math101')" }
                },
                "required": ["class_id"]
            }),
        ),
        crate::make_tool(
            "read_class_file",
            "Read the content of a class material file. \
             Path must be relative to the class folder. \
             Text files (.md, .txt, .py, .rs, etc.) are returned as-is. \
             Binary files return a base64 data-URI.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "class_id": { "type": "string", "description": "Class identifier" },
                    "path": { "type": "string", "description": "File path relative to class root (e.g. 'chapter3.md')" }
                },
                "required": ["class_id", "path"]
            }),
        ),
        crate::make_tool(
            "search_class_content",
            "Semantic search over indexed class materials. \
             Returns the most relevant text chunks for the query, \
             ranked by embedding similarity. \
             Call index_class_file first to index new materials.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "class_id": { "type": "string", "description": "Class identifier" },
                    "query": { "type": "string", "description": "Natural language search query" },
                    "limit": { "type": "integer", "description": "Max results to return (default 5)" }
                },
                "required": ["class_id", "query"]
            }),
        ),
        crate::make_tool(
            "index_class_file",
            "Embed a markdown or text file from the class folder into the \
             semantic search index. Run this once per file when materials \
             are first uploaded or updated. \
             Splits the file into 500-char chunks with 50-char overlap.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "class_id": { "type": "string", "description": "Class identifier" },
                    "path": { "type": "string", "description": "File path relative to class root" }
                },
                "required": ["class_id", "path"]
            }),
        ),
    ]
}

// ── Dispatcher ─────────────────────────────────────────────────────────────

pub async fn call_tool(
    name: &str,
    args: &serde_json::Map<String, Value>,
    classes_root: &str,
    index: &Arc<SearchIndex>,
    embedding: &Arc<EmbeddingClient>,
    student: &Option<StudentInfo>,
) -> Result<CallToolResult, McpError> {
    // Require a student context for all class operations
    let s = student
        .as_ref()
        .ok_or_else(|| McpError::internal_error("no student context", None))?;

    match name {
        "list_class_files" => {
            let class_id = get_str(args, "class_id")?;
            s.assert_class_access(&class_id)?;
            let base = class_dir(classes_root, &class_id)?;
            list_files(&base)
        }
        "read_class_file" => {
            let class_id = get_str(args, "class_id")?;
            s.assert_class_access(&class_id)?;
            let path = get_str(args, "path")?;
            let base = class_dir(classes_root, &class_id)?;
            read_file(&base, &path)
        }
        "search_class_content" => {
            let class_id = get_str(args, "class_id")?;
            s.assert_class_access(&class_id)?;
            let query = get_str(args, "query")?;
            let limit = args
                .get("limit")
                .and_then(|v| v.as_u64())
                .unwrap_or(5) as usize;
            search(class_id, &query, limit, index, embedding, index.dim).await
        }
        "index_class_file" => {
            let class_id = get_str(args, "class_id")?;
            // Only teachers may index files
            if s.role != "teacher" {
                return Err(McpError::invalid_params(
                    "only teachers may index class files",
                    None,
                ));
            }
            let path = get_str(args, "path")?;
            let base = class_dir(classes_root, &class_id)?;
            index_file(&base, class_id, path, index, embedding, index.dim).await
        }
        _ => unreachable!("unknown class_files tool: {name}"),
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
        .skip(1) // skip root itself
        .map(|e| {
            let rel = e.path().strip_prefix(base).unwrap().display().to_string();
            let meta = e.metadata().ok();
            Entry {
                path: rel,
                size: meta.as_ref().map(|m| m.len()).unwrap_or(0),
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

    let ext = resolved
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    // Text extensions — return UTF-8 string
    let is_text = matches!(
        ext,
        "md" | "txt" | "py" | "rs" | "js" | "ts" | "json" | "toml"
            | "yaml" | "yml" | "html" | "css" | "sh" | "csv"
    );

    if is_text {
        let content = std::fs::read_to_string(&resolved)
            .map_err(|e| McpError::internal_error(format!("read error: {e}"), None))?;
        Ok(CallToolResult::success(vec![Content::text(content)]))
    } else {
        // Binary — base64 data URI
        let bytes = std::fs::read(&resolved)
            .map_err(|e| McpError::internal_error(format!("read error: {e}"), None))?;
        let b64 = base64_encode(&bytes);
        let mime = mime_for_ext(ext);
        Ok(CallToolResult::success(vec![Content::text(format!(
            "data:{mime};base64,{b64}"
        ))]))
    }
}

async fn search(
    class_id: String,
    query: &str,
    limit: usize,
    index: &Arc<SearchIndex>,
    embedding: &Arc<EmbeddingClient>,
    dim: usize,
) -> Result<CallToolResult, McpError> {
    let emb = embedding
        .embed_with_dim(query, dim)
        .await
        .map_err(|e| McpError::internal_error(format!("embed: {e}"), None))?;

    let results = index
        .hybrid_search(class_id, query, emb, limit)
        .await
        .map_err(|e| McpError::internal_error(format!("hybrid_search: {e}"), None))?;

    let out = serde_json::to_string_pretty(&results)
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;
    Ok(CallToolResult::success(vec![Content::text(out)]))
}

async fn index_file(
    base: &Path,
    class_id: String,
    rel_path: String,
    index: &Arc<SearchIndex>,
    embedding: &Arc<EmbeddingClient>,
    dim: usize,
) -> Result<CallToolResult, McpError> {
    let resolved = safe_resolve(base, &rel_path)?;

    let text = std::fs::read_to_string(&resolved)
        .map_err(|e| McpError::internal_error(format!("read: {e}"), None))?;

    // Remove stale chunks for this file before re-indexing
    index
        .delete_chunks_for_file(&class_id, &rel_path)
        .await
        .map_err(|e| McpError::internal_error(format!("delete old chunks: {e}"), None))?;

    // Split into overlapping chunks (500 chars, 50-char overlap)
    let chunks = chunk_text(&text, 500, 50);
    let total = chunks.len();

    for chunk in chunks {
        let emb = embedding
            .embed_with_dim(&chunk, dim)
            .await
            .map_err(|e| McpError::internal_error(format!("embed chunk: {e}"), None))?;

        index
            .insert_chunk(class_id.clone(), rel_path.clone(), chunk, emb)
            .await
            .map_err(|e| McpError::internal_error(format!("insert chunk: {e}"), None))?;
    }

    // Rebuild FTS (BM25) and vector indexes so new chunks are immediately searchable.
    index
        .rebuild_fts_index()
        .await
        .map_err(|e| McpError::internal_error(format!("rebuild index: {e}"), None))?;

    info!("indexed {rel_path} ({class_id}) — {total} chunks");
    Ok(CallToolResult::success(vec![Content::text(format!(
        "Indexed {rel_path} into class {class_id}: {total} chunks stored."
    ))]))
}

// ── Helpers ────────────────────────────────────────────────────────────────

/// Resolve and validate that `rel_path` stays inside `base`.
fn safe_resolve(base: &Path, rel_path: &str) -> Result<PathBuf, McpError> {
    let joined = base.join(rel_path);
    // canonicalize requires the path to exist
    let resolved = joined
        .canonicalize()
        .map_err(|e| McpError::invalid_params(format!("path not found: {e}"), None))?;
    if !resolved.starts_with(base) {
        return Err(McpError::invalid_params("path traversal denied", None));
    }
    Ok(resolved)
}

/// Build and verify the class content directory path.
fn class_dir(classes_root: &str, class_id: &str) -> Result<PathBuf, McpError> {
    // Reject class_ids that contain path separators
    if class_id.contains('/') || class_id.contains('\\') || class_id.contains("..") {
        return Err(McpError::invalid_params("invalid class_id", None));
    }
    let path = Path::new(classes_root).join(class_id);
    if !path.exists() {
        return Err(McpError::invalid_params(
            format!("class '{class_id}' not found"),
            None,
        ));
    }
    Ok(path)
}

/// Split text into overlapping chunks for embedding.
fn chunk_text(text: &str, size: usize, overlap: usize) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut chunks = Vec::new();
    let step = size.saturating_sub(overlap);
    let mut start = 0;
    while start < chars.len() {
        let end = (start + size).min(chars.len());
        chunks.push(chars[start..end].iter().collect());
        if end == chars.len() {
            break;
        }
        start += step;
    }
    chunks
}

fn base64_encode(bytes: &[u8]) -> String {
    use std::io::Write;
    // Use only std — no extra dep
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((bytes.len() + 2) / 3 * 4);
    for chunk in bytes.chunks(3) {
        let n = (chunk[0] as u32) << 16
            | (*chunk.get(1).unwrap_or(&0) as u32) << 8
            | (*chunk.get(2).unwrap_or(&0) as u32);
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 { TABLE[((n >> 6) & 63) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { TABLE[(n & 63) as usize] as char } else { '=' });
    }
    out
}

fn mime_for_ext(ext: &str) -> &'static str {
    match ext {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
}

fn get_str<'a>(
    args: &'a serde_json::Map<String, Value>,
    key: &str,
) -> Result<String, McpError> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| McpError::invalid_params(format!("missing '{key}'"), None))
}
