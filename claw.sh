#!/usr/bin/env bash
# Wrapper: sources .env then launches claw with xAI defaults.
# Usage: ./claw.sh [any claw flags]
# Examples:
#   ./claw.sh                          # REPL (uses last /model or grok-3)
#   ./claw.sh --model grok-3-mini      # REPL with specific model
#   ./claw.sh prompt "Hello"           # one-shot
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Load .env from same directory
if [[ -f "$SCRIPT_DIR/.env" ]]; then
  # shellcheck disable=SC1091
  source "$SCRIPT_DIR/.env"
  export XAI_API_KEY
  export CLAW_BIN
  export CLAW_MODEL
fi

# Default model to grok-3 unless --model is already in the args
DEFAULT_MODEL="${CLAW_MODEL:-grok-3}"
if [[ " $* " != *" --model "* ]]; then
  exec "$SCRIPT_DIR/claw" --model "$DEFAULT_MODEL" "$@"
else
  exec "$SCRIPT_DIR/claw" "$@"
fi
