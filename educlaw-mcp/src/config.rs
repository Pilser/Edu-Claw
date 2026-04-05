// educlaw-mcp/src/config.rs
//
// All configuration is sourced from environment variables so the server
// works both in child-process (stdio) mode and as a persistent TCP daemon.

#[derive(Clone, Debug)]
pub struct Config {
    /// Root directory where class content is stored (read-only lower layer).
    /// Default: /data/classes
    pub classes_root: String,

    /// Root directory for per-student upper layers.
    /// Default: /data/students
    pub students_root: String,

    /// Path to the sqlite-vec search index database.
    /// Default: /var/lib/educlaw/educlaw-search.db
    pub search_db: String,

    /// Embedding server URL.
    /// Default: http://127.0.0.1:8181
    pub embedding_url: String,

    /// Number of dimensions the embedding model produces.
    /// Must match the vec0 table definition in the search index.
    /// Set EMBEDDING_DIM env var when using a non-384-dim model.
    /// Common values: 384 (MiniLM-L6), 768 (MPNet, BGE-base), 1536 (OpenAI ada-002)
    /// Default: 384
    pub embedding_dim: usize,

    /// JSON string injected by the session broker describing the student.
    /// Shape: { student_id, username, display_name, role, class_ids, session_id }
    /// May be empty if running in admin / standalone mode.
    pub student_json: Option<String>,
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        Ok(Self {
            classes_root: std::env::var("EDUCLAW_CLASSES_ROOT")
                .unwrap_or_else(|_| "/data/classes".to_string()),
            students_root: std::env::var("EDUCLAW_STUDENTS_ROOT")
                .unwrap_or_else(|_| "/data/students".to_string()),
            search_db: std::env::var("EDUCLAW_SEARCH_DB")
                .unwrap_or_else(|_| "/var/lib/educlaw/educlaw-search.db".to_string()),
            embedding_url: std::env::var("EMBEDDING_API_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:8181".to_string()),
            embedding_dim: std::env::var("EMBEDDING_DIM")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(384),
            student_json: std::env::var("EDUCLAW_STUDENT_JSON").ok(),
        })
    }

    /// Parse the student JSON into a `StudentInfo` struct.
    /// Returns None when running without a student context (admin mode).
    pub fn student_info(&self) -> Option<StudentInfo> {
        self.student_json
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
    }
}

// ── Student identity ──────────────────────────────────────────────────────

/// Set by the session broker via EDUCLAW_STUDENT_JSON at session start.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct StudentInfo {
    pub student_id: String,
    pub username: String,
    pub display_name: String,
    pub role: String,
    pub class_ids: Vec<String>,
    #[serde(default)]
    pub session_id: Option<String>,
}
