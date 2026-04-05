// eval_engine/records.rs — Student answer persistence.
//
// Answers are stored per-student inside the OverlayFS upper layer so they
// are isolated from other students and survive session restarts.
//
// Layout:
//   {students_root}/{student_id}/upper/.educlaw_evals/{assignment_id}/answers.json
//   {students_root}/{student_id}/upper/.educlaw_evals/{assignment_id}/submitted.json

use std::path::PathBuf;

use rmcp::ErrorData as McpError;
use serde::{Deserialize, Serialize};

use crate::config::StudentInfo;

// ── Types ─────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct AnswerRecord {
    pub question_id: String,
    pub answer: String,
    pub correct: bool,
    pub score: u32,
    pub max_points: u32,
    pub attempt: u32,
    pub recorded_at: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct AnswersFile {
    pub answers: Vec<AnswerRecord>,
}

// ── Paths ─────────────────────────────────────────────────────────────────

pub fn answers_path(students_root: &str, student_id: &str, assignment_id: &str) -> PathBuf {
    eval_dir(students_root, student_id, assignment_id).join("answers.json")
}

pub fn submitted_path(students_root: &str, student_id: &str, assignment_id: &str) -> PathBuf {
    eval_dir(students_root, student_id, assignment_id).join("submitted.json")
}

fn eval_dir(students_root: &str, student_id: &str, assignment_id: &str) -> PathBuf {
    PathBuf::from(students_root)
        .join(student_id)
        .join("upper")
        .join(".educlaw_evals")
        .join(assignment_id)
}

// ── Load / Save ───────────────────────────────────────────────────────────

pub fn load_answers(students_root: &str, student_id: &str, assignment_id: &str) -> AnswersFile {
    std::fs::read_to_string(answers_path(students_root, student_id, assignment_id))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_answers(
    students_root: &str,
    student_id: &str,
    assignment_id: &str,
    af: &AnswersFile,
) -> Result<(), McpError> {
    let p = answers_path(students_root, student_id, assignment_id);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| McpError::invalid_params(format!("cannot create eval dir: {e}"), None))?;
    }
    std::fs::write(&p, serde_json::to_string_pretty(af).unwrap())
        .map_err(|e| McpError::invalid_params(format!("cannot write answers: {e}"), None))
}

// ── Guard ─────────────────────────────────────────────────────────────────

pub fn require_student(student: &Option<StudentInfo>) -> Result<&StudentInfo, McpError> {
    student
        .as_ref()
        .ok_or_else(|| McpError::invalid_params("no active student session", None))
}
