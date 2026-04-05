// educlaw-mcp/src/servers/student_context.rs
//
// Tools:
//   get_student_context — return the student's profile (from EDUCLAW_STUDENT_JSON)
//
// No database connection needed — the session broker injects the student JSON
// as an environment variable when spawning the claw process.

use rmcp::{model::CallToolResult, model::Content, ErrorData as McpError};
use serde_json::Value;

use crate::config::StudentInfo;

pub fn tools() -> Vec<rmcp::model::Tool> {
    vec![crate::make_tool(
        "get_student_context",
        "Return the current student's profile: name, role, enrolled class IDs, \
         and session ID. Use this to understand who you are tutoring and which \
         classes they are enrolled in before accessing class materials.",
        serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
    )]
}

pub fn call_tool(
    name: &str,
    _args: &serde_json::Map<String, Value>,
    student: &Option<StudentInfo>,
) -> Result<CallToolResult, McpError> {
    match name {
        "get_student_context" => get_context(student),
        _ => unreachable!("unknown student_context tool: {name}"),
    }
}

fn get_context(student: &Option<StudentInfo>) -> Result<CallToolResult, McpError> {
    match student {
        Some(s) => {
            let json = serde_json::to_string_pretty(s)
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;
            Ok(CallToolResult::success(vec![Content::text(json)]))
        }
        None => Err(McpError::internal_error(
            "EDUCLAW_STUDENT_JSON not set — running without student context",
            None,
        )),
    }
}
