// eval_engine/checker.rs — Answer grading logic for all question types.
//
// Security: this module has full access to the spec (expected answers,
// rubric keywords). It returns only {correct, score, hint} — callers
// must NEVER forward raw spec fields to the agent.

use serde::Deserialize;
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

// ── AI essay grading (async, requires CLAW_BIN + API key in env) ──────────

/// Grade an essay using AI. Never falls back to keyword matching.
/// Returns a failure outcome with an explanatory hint if AI is unavailable.
pub async fn grade_essay_ai(q: &QuestionSpec, answer: &str) -> CheckOutcome {
    match try_ai_grade(q, answer).await {
        Ok(outcome) => outcome,
        Err(e) => {
            tracing::error!("AI essay grading failed: {e}");
            CheckOutcome {
                correct: false,
                score: 0,
                hint: Some("Grading is temporarily unavailable. Please try again shortly.".to_string()),
            }
        }
    }
}

#[derive(serde::Deserialize)]
struct ClawJsonOutput {
    message: String,
}

#[derive(serde::Deserialize)]
struct AiGradeResult {
    score: u32,
    feedback: String,
}

async fn try_ai_grade(q: &QuestionSpec, answer: &str) -> Result<CheckOutcome, String> {
    let claw = std::env::var("CLAW_BIN").unwrap_or_else(|_| "claw".to_string());
    let model = std::env::var("CLAW_MODEL").unwrap_or_else(|_| "grok-3".to_string());
    let keywords = q.rubric_keywords.as_deref().unwrap_or(&[]).join(", ");
    let prompt = format!(
        "You are an essay grader. Grade the following student answer.\n\
         Question: {question}\n\
         Required concepts: {keywords}\n\
         Max points: {max}\n\
         Student answer: {answer}\n\n\
         Respond with ONLY valid JSON on a single line, no explanation: \
         {{\"score\": <integer 0-{max}>, \"feedback\": \"<one sentence>\"}}\n\
         Be fair and proportional. Reward understanding over exact wording.",
        question = q.prompt,
        keywords = keywords,
        max = q.points,
        answer = answer,
    );

    let output = tokio::process::Command::new(&claw)
        .args(["--model", &model, "--output-format", "json", "prompt", &prompt])
        .output()
        .await
        .map_err(|e| format!("failed to spawn claw: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("claw exited non-zero: {stderr}"));
    }

    let claw_out: ClawJsonOutput = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("failed to parse claw JSON: {e}"))?;

    // claw's `message` field contains the raw LLM text; parse our inner JSON from it.
    // The LLM might wrap it in markdown backticks — strip those first.
    let inner = claw_out.message
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    // Find the first `{` in the text to handle any preamble.
    let json_start = inner.find('{').ok_or("no JSON object in claw response")?;
    let json_str = &inner[json_start..];

    let result: AiGradeResult = serde_json::from_str(json_str)
        .map_err(|e| format!("failed to parse AI grade JSON: {e}"))?;

    let score = result.score.min(q.points);
    let correct = score >= (q.points / 2);
    Ok(CheckOutcome {
        correct,
        score,
        hint: if correct { None } else { Some(result.feedback) },
    })
}
