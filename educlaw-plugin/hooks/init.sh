#!/bin/sh
# EduClaw plugin — Init lifecycle hook
# Called once when `claw` starts inside a student session.
#
# Environment variables provided by educlaw-server:
#   EDUCLAW_STUDENT_JSON  — JSON blob: {student_id, username, display_name, role, cohort_id, session_id}
#                           NOTE: subject_ids are in the blob for MCP access control only —
#                           they must NOT appear in the system prompt.
#   EDUCLAW_SESSION_ID    — unique session UUID
#   EDUCLAW_WORKSPACE     — absolute path to the student's OverlayFS merged workspace
#   EDUCLAW_MCP_SOCKET    — address/path to the educlaw-mcp stdio server (optional)
#   CLAW_CONFIG_HOME      — set by educlaw-server to the per-session config dir
#   EDUCLAW_INTERNAL_TOKEN — bearer token for MCP → server REST calls
#
# This hook:
#   1. Validates required env vars
#   2. Writes a per-session settings.json that pre-authorizes all tool calls and
#      injects a LEAN student system prompt (name + workspace only)
#   3. Registers educlaw-mcp as an MCP stdio server when available
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

export EDUCLAW_STUDENT_JSON EDUCLAW_SESSION_ID EDUCLAW_WORKSPACE EDUCLAW_MCP_SOCKET EDUCLAW_INTERNAL_TOKEN

python3 - <<'PYEOF'
import json
import os
import sys

student_json_raw = os.environ.get("EDUCLAW_STUDENT_JSON", "{}")
session_id = os.environ.get("EDUCLAW_SESSION_ID", "")
workspace = os.environ.get("EDUCLAW_WORKSPACE", "")
mcp_socket = os.environ.get("EDUCLAW_MCP_SOCKET", "")
internal_token = os.environ.get("EDUCLAW_INTERNAL_TOKEN", "")
config_home = os.environ.get(
    "CLAW_CONFIG_HOME", os.path.join(os.path.expanduser("~"), ".claw")
)

try:
    student = json.loads(student_json_raw)
except json.JSONDecodeError as exc:
    print(f"EduClaw init: failed to parse EDUCLAW_STUDENT_JSON: {exc}", file=sys.stderr)
    sys.exit(1)

# Lean system prompt — identity only.
# The agent must NOT receive a subject list here.
# Subjects are discovered on demand via MCP tools.
display_name = student.get("display_name") or student.get("username") or "Student"
cohort_id    = student.get("cohort_id") or ""

lines = [
    f"You are an AI tutor for {display_name}.",
    "Your workspace is at: " + workspace,
]
if session_id:
    lines.append(f"Session ID: {session_id}")
if cohort_id:
    lines.append(f"Cohort ID: {cohort_id}")
lines += [
    "",
    "Guide the student — ask Socratic questions rather than giving direct answers.",
    "Use your tools to discover what subjects and assignments are available when asked.",
]

system_prompt = "\n".join(lines)

settings = {
    "permissionMode": "danger-full-access",
    "systemPrompt": system_prompt,
}

if mcp_socket:
    mcp_env = {
        "EDUCLAW_STUDENT_JSON": student_json_raw,
        "EDUCLAW_SESSION_ID": session_id,
    }
    if internal_token:
        mcp_env["EDUCLAW_INTERNAL_TOKEN"] = internal_token

    settings["mcpServers"] = {
        "educlaw-context": {
            "type": "stdio",
            "command": "educlaw-mcp",
            "args": ["--socket", mcp_socket],
            "env": mcp_env,
        }
    }

os.makedirs(config_home, exist_ok=True)
settings_path = os.path.join(config_home, "settings.json")

# Merge into the existing settings.json (the server pre-writes externalDirectories there).
# We add our keys without touching keys the server already set.
try:
    with open(settings_path) as fh:
        existing = json.load(fh)
except (FileNotFoundError, json.JSONDecodeError):
    existing = {}

existing.update(settings)

with open(settings_path, "w") as fh:
    json.dump(existing, fh, indent=2)

print(f"EduClaw: session settings written to {settings_path}")
PYEOF
