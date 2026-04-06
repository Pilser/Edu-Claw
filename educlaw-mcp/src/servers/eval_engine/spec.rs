// eval_engine/spec.rs — Assignment spec types, path resolution, and loading.
//
// These types are deserialized from teacher-authored spec.json files.
// They are NEVER sent to the agent whole — handlers strip expected answers
// and rubric content before returning data through any tool.

use std::path::PathBuf;

use rmcp::ErrorData as McpError;
use serde::Deserialize;
use serde_json::Value;

// ── Types ─────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct AssignmentSpec {
    pub assignment_id: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub questions: Vec<QuestionSpec>,
}

#[derive(Debug, Deserialize)]
pub struct QuestionSpec {
    pub id: String,
    pub prompt: String,
    #[serde(rename = "type")]
    pub kind: String,
    // numeric
    pub expected: Option<Value>,
    pub tolerance: Option<f64>,
    pub unit: Option<String>,
    // essay
    pub rubric_keywords: Option<Vec<String>>,
    pub min_keywords: Option<usize>,
    // multiple_choice
    pub choices: Option<Vec<String>>,
    // shared
    pub hint_on_wrong: Option<String>,
    #[serde(default = "default_points")]
    pub points: u32,
}

fn default_points() -> u32 {
    10
}

// ── Path ──────────────────────────────────────────────────────────────────

pub fn spec_path(subjects_root: &str, cohort_id: &str, subject_id: &str, assignment_id: &str) -> PathBuf {
    PathBuf::from(subjects_root)
        .join(cohort_id)
        .join(subject_id)
        .join("assignments")
        .join(assignment_id)
        .join("spec.json")
}

// ── Validation ────────────────────────────────────────────────────────────

/// Allow only alphanumeric, `-`, `_` in IDs to prevent path traversal.
pub fn validate_slug(s: &str, label: &str) -> Result<String, McpError> {
    if !s.is_empty() && s.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
        Ok(s.to_string())
    } else {
        Err(McpError::invalid_params(
            format!("invalid {label}: must be alphanumeric / dash / underscore"),
            None,
        ))
    }
}

// ── Loading ───────────────────────────────────────────────────────────────

pub fn load_spec(
    subjects_root: &str,
    cohort_id: &str,
    subject_id: &str,
    assignment_id: &str,
) -> Result<AssignmentSpec, McpError> {
    let p = spec_path(subjects_root, cohort_id, subject_id, assignment_id);
    let raw = std::fs::read_to_string(&p).map_err(|e| {
        McpError::invalid_params(format!("cannot read spec for {assignment_id}: {e}"), None)
    })?;
    serde_json::from_str(&raw).map_err(|e| {
        McpError::invalid_params(format!("invalid spec JSON for {assignment_id}: {e}"), None)
    })
}
