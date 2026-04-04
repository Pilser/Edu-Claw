#!/bin/sh
# EduClaw plugin — PreToolUse hook
# Called before every tool invocation inside a student session.
#
# Receives tool context via environment variables:
#   HOOK_EVENT       — always "PreToolUse"
#   HOOK_TOOL_NAME   — name of the tool being called (e.g. "Bash", "Write")
#   HOOK_TOOL_INPUT  — JSON-encoded tool input
#
# Also receives the full hook payload as stdin (JSON).
#
# Exit codes:
#   0 — allow the tool call
#   2 — deny the tool call (message on stdout is shown to the AI)
#   other — hook failure (logged, tool call may still proceed depending on config)
#
# Current policy:
#   - Block any Bash command that tries to leave the workspace (cd /, rm -rf /, etc.)
#   - Allow everything else (outer namespace + OverlayFS provides the real boundary)

set -e

TOOL="${HOOK_TOOL_NAME:-}"
INPUT="${HOOK_TOOL_INPUT:-}"

# Only inspect Bash tool calls
if [ "$TOOL" != "Bash" ]; then
  exit 0
fi

# Extract the command string from the JSON input using python3
COMMAND=$(python3 -c "
import json, sys, os
raw = os.environ.get('HOOK_TOOL_INPUT', '{}')
try:
    data = json.loads(raw)
    cmd = data.get('command', '')
    print(cmd)
except Exception:
    pass
" 2>/dev/null)

# Deny attempts to escape the workspace filesystem boundary
case "$COMMAND" in
  *"rm -rf /"* | *"rm -rf /*"*)
    printf 'EduClaw: recursive root deletion is not allowed inside a student session.\n'
    exit 2
    ;;
  *"mkfs"* | *"dd if="* | *":(){ :|:& };:"*)
    printf 'EduClaw: that command is not permitted in a student session.\n'
    exit 2
    ;;
esac

exit 0
