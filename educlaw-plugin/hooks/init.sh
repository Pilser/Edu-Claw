#!/bin/sh
# EduClaw plugin — Init lifecycle hook
# Called once when `claw` starts inside a student session.
#
# Environment variables provided by educlaw-server:
#   EDUCLAW_STUDENT_JSON  — JSON blob: {id, name, email, course, assignment, grade_level}
#   EDUCLAW_SESSION_ID    — unique session UUID
#   EDUCLAW_WORKSPACE     — absolute path to the student's OverlayFS merged workspace
#   EDUCLAW_MCP_SOCKET    — path to the Unix socket for the educlaw-mcp stdio server (optional)
#   CLAW_CONFIG_HOME      — set by educlaw-server to the per-session config dir
#
# This hook:
#   1. Validates required env vars
#   2. Writes a per-session settings.json that pre-authorizes all tool calls and
#      injects the student system prompt
#   3. Optionally registers educlaw-mcp as an MCP stdio server if EDUCLAW_MCP_SOCKET is set
#   4. Exits 0 on success; non-zero aborts claw startup

set -e

if [ -z "$EDUCLAW_STUDENT_JSON" ]; then
  printf 'EduClaw init: EDUCLAW_STUDENT_JSON is not set\n' >&2
  exit 1
fi

if [ -z "$EDUCLAW_WORKSPACE" ]; then
  printf 'EduClaw init: EDUCLAW_WORKSPACE is not set\n' >&2
  exit 1
fi

export EDUCLAW_STUDENT_JSON EDUCLAW_SESSION_ID EDUCLAW_WORKSPACE EDUCLAW_MCP_SOCKET

python3 - <<'PYEOF'
import json
import os
import sys

student_json_raw = os.environ.get("EDUCLAW_STUDENT_JSON", "{}")
session_id = os.environ.get("EDUCLAW_SESSION_ID", "")
workspace = os.environ.get("EDUCLAW_WORKSPACE", "")
mcp_socket = os.environ.get("EDUCLAW_MCP_SOCKET", "")
config_home = os.environ.get(
    "CLAW_CONFIG_HOME", os.path.join(os.path.expanduser("~"), ".claw")
)

try:
    student = json.loads(student_json_raw)
except json.JSONDecodeError as exc:
    print(f"EduClaw init: failed to parse EDUCLAW_STUDENT_JSON: {exc}", file=sys.stderr)
    sys.exit(1)

name = student.get("name") or "Student"
course = student.get("course") or "General Studies"
assignment = student.get("assignment") or ""
grade_level = student.get("grade_level") or ""

lines = [
    f'You are a dedicated AI tutor for {name} in the course "{course}".',
]
if grade_level:
    lines.append(f"The student is at grade level: {grade_level}.")
if assignment:
    lines.append(f'The current assignment is: {assignment}.')
lines += [
    "",
    "Your role is to guide, not to give direct answers. Ask Socratic questions.",
    "Encourage the student to reason through problems step by step.",
    "If they are stuck, provide hints and scaffolding rather than solutions.",
    "Celebrate progress and be patient with mistakes.",
    "",
    f"The student's workspace is at: {workspace}",
]
if session_id:
    lines.append(f"Session ID: {session_id}")

system_prompt = "\n".join(lines)

settings = {
    "permissionMode": "danger-full-access",
    "systemPrompt": system_prompt,
}

if mcp_socket:
    settings["mcpServers"] = {
        "educlaw-context": {
            "type": "stdio",
            "command": "educlaw-mcp",
            "args": ["--socket", mcp_socket],
            "env": {
                "EDUCLAW_STUDENT_JSON": student_json_raw,
                "EDUCLAW_SESSION_ID": session_id,
            },
        }
    }

os.makedirs(config_home, exist_ok=True)
settings_path = os.path.join(config_home, "settings.json")
with open(settings_path, "w") as fh:
    json.dump(settings, fh, indent=2)

print(f"EduClaw: session settings written to {settings_path}")
PYEOF
