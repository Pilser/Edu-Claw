// eval_engine/mod.rs — MCP tool definitions and call_tool dispatch.
//
// Assignment specs: {classes_root}/{class_id}/assignments/{assignment_id}/spec.json
// Student progress: {students_root}/{student_id}/upper/.educlaw_evals/{assignment_id}/
//
// Security contract: expected answers and rubric keywords are loaded here but
// never forwarded to the agent. Only {correct, score, hint} is returned.

mod checker;
mod records;
mod spec;

use std::path::PathBuf;

use rmcp::{model::CallToolResult, model::Content, ErrorData as McpError};
use serde::Serialize;
use serde_json::{json, Value};
use tracing::debug;

use crate::config::StudentInfo;
use checker::{check_answer, grade_essay_ai};
use records::{
    load_answers, require_student, save_answers, submitted_path, AnswerRecord, AnswersFile,
};
use spec::{load_spec, validate_slug};

// ── Tool definitions ───────────────────────────────────────────────────────

pub fn tools() -> Vec<rmcp::model::Tool> {
    vec![
        crate::make_tool(
            "list_assignments",
            "List all assignments available for a class. Returns IDs, titles, \
             question counts, and max points. Does NOT return expected answers.",
            json!({
                "type": "object",
                "properties": {
                    "class_id": { "type": "string", "description": "Class identifier" }
                },
                "required": ["class_id"]
            }),
        ),
        crate::make_tool(
            "get_assignment",
            "Get the assignment definition for the student: title, description, \
             questions (prompts, types, choices if applicable, max points). \
             Expected answers and rubrics are intentionally omitted.",
            json!({
                "type": "object",
                "properties": {
                    "class_id":      { "type": "string" },
                    "assignment_id": { "type": "string" }
                },
                "required": ["class_id", "assignment_id"]
            }),
        ),
        crate::make_tool(
            "check_answer",
            "Check a student's answer to a specific question. Returns whether the \
             answer is correct, the score awarded, max possible points, and an \
             optional hint if wrong. The correct answer is NEVER revealed.",
            json!({
                "type": "object",
                "properties": {
                    "class_id":      { "type": "string" },
                    "assignment_id": { "type": "string" },
                    "question_id":   {
                        "type": "string",
                        "description": "Question ID from get_assignment"
                    },
                    "answer": {
                        "type": "string",
                        "description": "Student's answer: number as string, choice label, or essay text"
                    }
                },
                "required": ["class_id", "assignment_id", "question_id", "answer"]
            }),
        ),
        crate::make_tool(
            "record_answer",
            "Persist the student's confirmed answer for a question. Call after \
             check_answer once the student is satisfied with their response. \
             Tracks attempt count and scores per question.",
            json!({
                "type": "object",
                "properties": {
                    "class_id":      { "type": "string" },
                    "assignment_id": { "type": "string" },
                    "question_id":   { "type": "string" },
                    "answer":        { "type": "string" }
                },
                "required": ["class_id", "assignment_id", "question_id", "answer"]
            }),
        ),
        crate::make_tool(
            "get_progress",
            "Return the student's current progress on an assignment: which \
             questions have been answered, per-question scores, and running total.",
            json!({
                "type": "object",
                "properties": {
                    "class_id":      { "type": "string" },
                    "assignment_id": { "type": "string" }
                },
                "required": ["class_id", "assignment_id"]
            }),
        ),
        crate::make_tool(
            "submit_assignment",
            "Finalise the assignment. A submitted assignment is locked — answers \
             cannot be changed. Returns the final score and per-question summary.",
            json!({
                "type": "object",
                "properties": {
                    "class_id":      { "type": "string" },
                    "assignment_id": { "type": "string" }
                },
                "required": ["class_id", "assignment_id"]
            }),
        ),
    ]
}

// ── Dispatch ───────────────────────────────────────────────────────────────

pub fn call_tool(
    name: &str,
    args: &serde_json::Map<String, Value>,
    classes_root: &str,
    students_root: &str,
    student: &Option<StudentInfo>,
) -> Result<CallToolResult, McpError> {
    let str_arg = |key: &str| -> Result<String, McpError> {
        args.get(key)
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| McpError::invalid_params(format!("missing `{key}`"), None))
    };

    match name {
        "list_assignments"  => handle_list_assignments(str_arg, classes_root, student),
        "get_assignment"    => handle_get_assignment(str_arg, classes_root, student),
        "check_answer"      => handle_check_answer(str_arg, classes_root, student),
        "record_answer"     => handle_record_answer(str_arg, classes_root, students_root, student),
        "get_progress"      => handle_get_progress(str_arg, classes_root, students_root, student),
        "submit_assignment" => handle_submit(str_arg, classes_root, students_root, student),
        _ => Err(McpError::invalid_params(format!("unknown eval tool: {name}"), None)),
    }
}

// ── Handlers ──────────────────────────────────────────────────────────────

fn handle_list_assignments(
    str_arg: impl Fn(&str) -> Result<String, McpError>,
    classes_root: &str,
    student: &Option<StudentInfo>,
) -> Result<CallToolResult, McpError> {
    let class_id = validate_slug(&str_arg("class_id")?, "class_id")?;
    let s = require_student(student)?;
    s.assert_class_access(&class_id)?;
    let dir = PathBuf::from(classes_root).join(&class_id).join("assignments");

    let entries = std::fs::read_dir(&dir)
        .map_err(|e| McpError::invalid_params(format!("cannot list assignments: {e}"), None))?;

    let mut list: Vec<Value> = entries
        .flatten()
        .filter(|e| e.path().join("spec.json").exists())
        .filter_map(|e| {
            let id = e.file_name().to_string_lossy().to_string();
            // Validate slug before loading to prevent path traversal
            validate_slug(&id, "assignment_id").ok()?;
            let spec = load_spec(classes_root, &class_id, &id).ok()?;
            let max: u32 = spec.questions.iter().map(|q| q.points).sum();
            Some(json!({
                "assignment_id": spec.assignment_id,
                "title": spec.title,
                "description": spec.description,
                "question_count": spec.questions.len(),
                "max_points": max
            }))
        })
        .collect();

    list.sort_by(|a, b| {
        a["assignment_id"]
            .as_str()
            .unwrap_or("")
            .cmp(b["assignment_id"].as_str().unwrap_or(""))
    });

    ok_json(&json!(list))
}

fn handle_get_assignment(
    str_arg: impl Fn(&str) -> Result<String, McpError>,
    classes_root: &str,
    student: &Option<StudentInfo>,
) -> Result<CallToolResult, McpError> {
    let class_id      = validate_slug(&str_arg("class_id")?, "class_id")?;
    let assignment_id = validate_slug(&str_arg("assignment_id")?, "assignment_id")?;
    let s = require_student(student)?;
    s.assert_class_access(&class_id)?;
    let spec = load_spec(classes_root, &class_id, &assignment_id)?;

    // Strip expected answers and rubric before returning
    let questions: Vec<Value> = spec
        .questions
        .iter()
        .map(|q| {
            let mut qv = json!({ "id": q.id, "prompt": q.prompt, "type": q.kind, "points": q.points });
            if q.kind == "multiple_choice" {
                if let Some(c) = &q.choices { qv["choices"] = json!(c); }
            }
            if q.kind == "numeric" {
                if let Some(u) = &q.unit { qv["unit"] = json!(u); }
            }
            qv
        })
        .collect();

    let max: u32 = spec.questions.iter().map(|q| q.points).sum();
    ok_json(&json!({
        "assignment_id": spec.assignment_id,
        "title": spec.title,
        "description": spec.description,
        "questions": questions,
        "max_points": max
    }))
}

fn handle_check_answer(
    str_arg: impl Fn(&str) -> Result<String, McpError>,
    classes_root: &str,
    student: &Option<StudentInfo>,
) -> Result<CallToolResult, McpError> {
    let class_id      = validate_slug(&str_arg("class_id")?, "class_id")?;
    let assignment_id = validate_slug(&str_arg("assignment_id")?, "assignment_id")?;
    let question_id   = str_arg("question_id")?;
    let answer        = str_arg("answer")?;
    let s = require_student(student)?;
    s.assert_class_access(&class_id)?;

    let spec = load_spec(classes_root, &class_id, &assignment_id)?;
    let q = spec
        .questions
        .iter()
        .find(|q| q.id == question_id)
        .ok_or_else(|| McpError::invalid_params(format!("question `{question_id}` not found"), None))?;

    debug!("check_answer class={class_id} assignment={assignment_id} q={question_id}");

    let out = check_answer(q, &answer);
    let mut result = json!({ "correct": out.correct, "score": out.score, "max_points": q.points });
    if let Some(hint) = out.hint { result["hint"] = json!(hint); }
    ok_json(&result)
}

/// Async version of check_answer — uses AI grading for essay type questions
/// when `EDUCLAW_AI_GRADING=1` is set. Falls back to keyword matching otherwise.
pub async fn check_answer_async(
    args: &serde_json::Map<String, Value>,
    classes_root: &str,
    student: &Option<StudentInfo>,
) -> Result<CallToolResult, McpError> {
    let str_arg = |key: &str| -> Result<String, McpError> {
        args.get(key)
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| McpError::invalid_params(format!("missing `{key}`"), None))
    };
    let class_id      = validate_slug(&str_arg("class_id")?, "class_id")?;
    let assignment_id = validate_slug(&str_arg("assignment_id")?, "assignment_id")?;
    let question_id   = str_arg("question_id")?;
    let answer        = str_arg("answer")?;
    let s = require_student(student)?;
    s.assert_class_access(&class_id)?;

    let spec = load_spec(classes_root, &class_id, &assignment_id)?;
    let q = spec
        .questions
        .iter()
        .find(|q| q.id == question_id)
        .ok_or_else(|| McpError::invalid_params(format!("question `{question_id}` not found"), None))?;

    debug!("check_answer_async class={class_id} assignment={assignment_id} q={question_id}");

    let out = if q.kind == "essay" {
        grade_essay_ai(q, &answer).await
    } else {
        check_answer(q, &answer)
    };
    let mut result = json!({ "correct": out.correct, "score": out.score, "max_points": q.points });
    if let Some(hint) = out.hint { result["hint"] = json!(hint); }
    ok_json(&result)
}

fn handle_record_answer(
    str_arg: impl Fn(&str) -> Result<String, McpError>,
    classes_root: &str,
    students_root: &str,
    student: &Option<StudentInfo>,
) -> Result<CallToolResult, McpError> {
    let class_id      = validate_slug(&str_arg("class_id")?, "class_id")?;
    let assignment_id = validate_slug(&str_arg("assignment_id")?, "assignment_id")?;
    let question_id   = str_arg("question_id")?;
    let answer        = str_arg("answer")?;
    let s = require_student(student)?;
    s.assert_class_access(&class_id)?;

    if submitted_path(students_root, &s.student_id, &assignment_id).exists() {
        return Err(McpError::invalid_params(
            "assignment already submitted — answers cannot be changed",
            None,
        ));
    }

    let spec = load_spec(classes_root, &class_id, &assignment_id)?;
    let q = spec
        .questions
        .iter()
        .find(|q| q.id == question_id)
        .ok_or_else(|| McpError::invalid_params(format!("question `{question_id}` not found"), None))?;

    let out = check_answer(q, &answer);
    let mut af = load_answers(students_root, &s.student_id, &assignment_id);
    let attempt = af.answers.iter().filter(|r| r.question_id == question_id).count() as u32 + 1;

    af.answers.push(AnswerRecord {
        question_id: question_id.clone(),
        answer,
        correct: out.correct,
        score: out.score,
        max_points: q.points,
        attempt,
        recorded_at: chrono::Utc::now().to_rfc3339(),
    });
    save_answers(students_root, &s.student_id, &assignment_id, &af)?;

    ok_json(&json!({ "recorded": true, "attempt": attempt, "correct": out.correct, "score": out.score }))
}

fn handle_get_progress(
    str_arg: impl Fn(&str) -> Result<String, McpError>,
    classes_root: &str,
    students_root: &str,
    student: &Option<StudentInfo>,
) -> Result<CallToolResult, McpError> {
    let class_id      = validate_slug(&str_arg("class_id")?, "class_id")?;
    let assignment_id = validate_slug(&str_arg("assignment_id")?, "assignment_id")?;
    let s = require_student(student)?;
    s.assert_class_access(&class_id)?
;    let spec      = load_spec(classes_root, &class_id, &assignment_id)?;;
    let af        = load_answers(students_root, &s.student_id, &assignment_id);
    let submitted = submitted_path(students_root, &s.student_id, &assignment_id).exists();

    let questions: Vec<Value> = spec
        .questions
        .iter()
        .map(|q| {
            let attempts: Vec<&AnswerRecord> =
                af.answers.iter().filter(|r| r.question_id == q.id).collect();
            let latest = attempts.last();
            json!({
                "question_id": q.id,
                "prompt": q.prompt,
                "type": q.kind,
                "attempts": attempts.len(),
                "answered": latest.is_some(),
                "correct": latest.map(|r| r.correct).unwrap_or(false),
                "score": latest.map(|r| r.score).unwrap_or(0),
                "max_points": q.points
            })
        })
        .collect();

    let total: u32 = questions.iter().map(|q| q["score"].as_u64().unwrap_or(0) as u32).sum();
    let max: u32   = spec.questions.iter().map(|q| q.points).sum();

    ok_json(&json!({
        "assignment_id": assignment_id,
        "submitted": submitted,
        "total_score": total,
        "max_score": max,
        "questions": questions
    }))
}

fn handle_submit(
    str_arg: impl Fn(&str) -> Result<String, McpError>,
    classes_root: &str,
    students_root: &str,
    student: &Option<StudentInfo>,
) -> Result<CallToolResult, McpError> {
    let class_id      = validate_slug(&str_arg("class_id")?, "class_id")?;
    let assignment_id = validate_slug(&str_arg("assignment_id")?, "assignment_id")?;
    let s = require_student(student)?;
    s.assert_class_access(&class_id)?;

    let sub_p = submitted_path(students_root, &s.student_id, &assignment_id);
    if sub_p.exists() {
        return Err(McpError::invalid_params("assignment already submitted", None));
    }

    let spec = load_spec(classes_root, &class_id, &assignment_id)?;
    let af   = load_answers(students_root, &s.student_id, &assignment_id);

    let mut total = 0u32;
    let max: u32 = spec.questions.iter().map(|q| q.points).sum();

    let summaries: Vec<Value> = spec
        .questions
        .iter()
        .map(|q| {
            let score = af
                .answers
                .iter()
                .filter(|r| r.question_id == q.id)
                .last()
                .map(|r| r.score)
                .unwrap_or(0);
            total += score;
            json!({ "question_id": q.id, "answered": score > 0 || af.answers.iter().any(|r| r.question_id == q.id), "score": score, "max_points": q.points })
        })
        .collect();

    let doc = json!({
        "assignment_id": assignment_id,
        "class_id": class_id,
        "student_id": s.student_id,
        "submitted_at": chrono::Utc::now().to_rfc3339(),
        "total_score": total,
        "max_score": max,
        "questions": summaries
    });

    if let Some(parent) = sub_p.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| McpError::invalid_params(format!("cannot create eval dir: {e}"), None))?;
    }
    std::fs::write(&sub_p, serde_json::to_string_pretty(&doc).unwrap())
        .map_err(|e| McpError::invalid_params(format!("cannot write submitted: {e}"), None))?;

    tracing::info!(
        student_id = %s.student_id,
        class_id = %class_id,
        assignment_id = %assignment_id,
        total_score = total,
        max_score = max,
        "assignment submitted"
    );

    ok_json(&doc)
}

// ── Utility ────────────────────────────────────────────────────────────────

fn ok_json(v: &impl Serialize) -> Result<CallToolResult, McpError> {
    Ok(CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(v).unwrap(),
    )]))
}
