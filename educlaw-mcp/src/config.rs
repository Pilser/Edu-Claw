// educlaw-mcp/src/config.rs
//
// All configuration is sourced from environment variables so the server
// works both in child-process (stdio) mode and as a persistent TCP daemon.

#[derive(Clone, Debug)]
pub struct Config {
    /// Root directory where subject content is stored (read-only lower layer).
    /// Default: /data/subjects
    pub subjects_root: String,

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
    /// Shape: { student_id, username, display_name, role, cohort_id, subject_ids, session_id }
    /// May be empty if running in admin / standalone mode.
    pub student_json: Option<String>,

    /// Base URL of the educlaw-server REST API.
    /// Shell MCP uses this to look up namespace_pid for nsenter.
    /// Default: http://127.0.0.1:3000
    pub server_url: String,
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        Ok(Self {
            subjects_root: std::env::var("EDUCLAW_SUBJECTS_ROOT")
                .unwrap_or_else(|_| "/data/subjects".to_string()),
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
            server_url: std::env::var("EDUCLAW_SERVER_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:3000".to_string()),
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
/// subject_ids are included here for access control only;
/// they are NOT shown to the agent in the system prompt.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct StudentInfo {
    pub student_id: String,
    pub username: String,
    pub display_name: String,
    pub role: String,
    pub cohort_id: String,
    /// Subject UUIDs the cohort is enrolled in — used for access control only.
    #[serde(default)]
    pub subject_ids: Vec<String>,
    #[serde(default)]
    pub session_id: Option<String>,
}

impl StudentInfo {
    /// Returns `Ok(())` if this session may access `subject_id`.
    /// Teachers are allowed all subjects.
    /// Students must be in a cohort enrolled in the subject.
    pub fn assert_subject_access(&self, subject_id: &str) -> Result<(), rmcp::ErrorData> {
        if self.role == "teacher" || self.role == "admin" {
            return Ok(());
        }
        if self.subject_ids.iter().any(|s| s == subject_id) {
            Ok(())
        } else {
            Err(rmcp::ErrorData::invalid_params(
                format!(
                    "student `{}` does not have access to subject `{subject_id}`",
                    self.student_id
                ),
                None,
            ))
        }
    }
}
