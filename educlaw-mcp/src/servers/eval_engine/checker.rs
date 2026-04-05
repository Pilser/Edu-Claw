// eval_engine/checker.rs — Answer grading logic for all question types.
//
// Security: this module has full access to the spec (expected answers,
// rubric keywords). It returns only {correct, score, hint} — callers
// must NEVER forward raw spec fields to the agent.

use serde_json::Value;

use super::spec::QuestionSpec;

// ── Outcome ───────────────────────────────────────────────────────────────

pub struct CheckOutcome {
    pub correct: bool,
    pub score: u32,
    pub hint: Option<String>,
}

// ── Grading dispatch ──────────────────────────────────────────────────────

pub fn check_answer(q: &QuestionSpec, answer: &str) -> CheckOutcome {
    match q.kind.as_str() {
        "numeric" => grade_numeric(q, answer),
        "multiple_choice" => grade_multiple_choice(q, answer),
        "essay" => grade_essay(q, answer),
        other => CheckOutcome {
            correct: false,
            score: 0,
            hint: Some(format!("Unknown question type: {other}")),
        },
    }
}

// ── Graders ───────────────────────────────────────────────────────────────

fn grade_numeric(q: &QuestionSpec, answer: &str) -> CheckOutcome {
    let expected = match &q.expected {
        Some(Value::Number(n)) => n.as_f64().unwrap_or(0.0),
        Some(Value::String(s)) => s.parse::<f64>().unwrap_or(0.0),
        _ => {
            return CheckOutcome { correct: false, score: 0, hint: q.hint_on_wrong.clone() };
        }
    };
    let student = match answer.trim().parse::<f64>() {
        Ok(v) => v,
        Err(_) => {
            return CheckOutcome {
                correct: false,
                score: 0,
                hint: Some("Please provide a numeric value.".to_string()),
            };
        }
    };
    let tol = q.tolerance.unwrap_or(0.0);
    let correct = (student - expected).abs() <= tol;
    CheckOutcome {
        correct,
        score: if correct { q.points } else { 0 },
        hint: if correct { None } else { q.hint_on_wrong.clone() },
    }
}

fn grade_multiple_choice(q: &QuestionSpec, answer: &str) -> CheckOutcome {
    let expected = match &q.expected {
        Some(Value::String(s)) => s.as_str(),
        _ => "",
    };
    let correct = answer.trim().eq_ignore_ascii_case(expected);
    CheckOutcome {
        correct,
        score: if correct { q.points } else { 0 },
        hint: if correct { None } else { q.hint_on_wrong.clone() },
    }
}

fn grade_essay(q: &QuestionSpec, answer: &str) -> CheckOutcome {
    let keywords = q.rubric_keywords.as_deref().unwrap_or(&[]);
    let min_k = q.min_keywords.unwrap_or((keywords.len() / 2).max(1));
    let lower = answer.to_lowercase();
    let matched = keywords
        .iter()
        .filter(|kw| lower.contains(kw.to_lowercase().as_str()))
        .count();
    let correct = matched >= min_k;
    // Partial scoring: proportional to keywords matched, floored.
    let score = if keywords.is_empty() {
        0
    } else {
        ((matched as f64 / keywords.len() as f64) * q.points as f64) as u32
    };
    CheckOutcome {
        correct,
        score,
        hint: if correct { None } else { q.hint_on_wrong.clone() },
    }
}
