# Edu-Claw Change Log

All significant changes made to the Edu-Claw project.

---

## xAI / Grok-3 Integration

### Problem
claw always routed requests to `api.anthropic.com` regardless of which model was specified. Using `--model grok-3` still sent requests to Anthropic, causing 401 errors.

### Root Cause
`AnthropicRuntimeClient.client` was hardcoded to `AnthropicClient`, which always uses `Authorization: x-api-key` and sends to Anthropic's endpoint. `ProviderClient` already existed in the codebase and could route by model name prefix (`grok-*` → xAI, `claude-*` → Anthropic) but was never wired into the CLI runner.

### Fix — `claw-code-parity/rust/crates/rusty-claude-cli/src/main.rs`
- Added `ProviderClient` to the `use api::{...}` import block
- Changed `AnthropicRuntimeClient.client` field type from `AnthropicClient` → `ProviderClient`
- Changed `AnthropicRuntimeClient::new()` to use `ProviderClient::from_model_with_anthropic_auth(&model, Some(resolve_cli_auth_source()?))`
- Updated `push_prompt_cache_record` signature from `&AnthropicClient` → `&ProviderClient`

### Fix — `Edu-Claw/claw.sh` (NEW FILE)
claw's startup always calls `resolve_startup_auth_source()` which requires an Anthropic API key even when using xAI. The wrapper satisfies this gate without sending the fake key to xAI:

```sh
#!/usr/bin/env bash
set -a
source "$(dirname "$0")/.env"
set +a
export XAI_API_KEY
export ANTHROPIC_API_KEY="${ANTHROPIC_API_KEY:-not-needed-using-xai}"
MODEL="${CLAW_MODEL:-grok-3}"
exec "$(dirname "$0")/claw" --model "$MODEL" "$@"
```

### Fix — `Edu-Claw/.env`
```
XAI_API_KEY=xai-<key>
CLAW_BIN=/home/pilser/Edu-Claw/claw
# CLAW_MODEL=grok-3   ← commented out; override at runtime with /model in REPL
```

---

## Fix: claw Says "I'm Claude Opus 4.6" When Using Grok

### Problem
When asked "what model are you?", claw replied "I'm Claude, specifically Claude 3.7 Sonnet" even when running grok-3.

### Root Cause
`runtime/src/prompt.rs:38`:
```rust
pub const FRONTIER_MODEL_NAME: &str = "Claude Opus 4.6";
```
This was injected into every session's system prompt as:
```
Model family: Claude Opus 4.6
```
Grok reads its own system prompt and reports that as its identity. No Anthropic API key required — it's just hardcoded text.

### Fix — `claw-code-parity/rust/crates/runtime/src/prompt.rs`
- Added `model_name: Option<String>` field to `SystemPromptBuilder`
- Added `pub fn with_model(mut self, model_name: impl Into<String>) -> Self` builder method
- Changed `environment_section()` to use `self.model_name.as_deref().unwrap_or(FRONTIER_MODEL_NAME)` instead of the constant
- Added `pub fn load_system_prompt_for_model(...)` — same as `load_system_prompt` but accepts `model_name: Option<&str>` and calls `builder.with_model(name)`

### Fix — `claw-code-parity/rust/crates/runtime/src/lib.rs`
- Added `load_system_prompt_for_model` to the `pub use prompt::{...}` re-export block

### Fix — `claw-code-parity/rust/crates/rusty-claude-cli/src/main.rs`
- Imported `load_system_prompt_for_model` from runtime
- Changed `build_system_prompt()` signature to `build_system_prompt(model: &str)`  
- Passes `Some(model)` to `load_system_prompt_for_model`
- Updated `LiveCli::new()` to call `build_system_prompt(&model)`

### Result
```
$ ./claw.sh --output-format json prompt "what model are you, in one sentence"
{"message":"I'm Grok-3, a large language model developed by xAI.",...}
```

---

## AI-Only Essay Grading (No Keyword Fallback)

### Problem
`grade_essay_maybe_ai` had an env var gate (`ENABLE_AI_GRADING`) and silently fell back to keyword matching when AI was unavailable. This meant grading could pass without AI involvement.

### Fix — `educlaw-mcp/src/servers/eval_engine/checker.rs`
- Removed `grade_essay_maybe_ai`
- Added `grade_essay_ai` — always calls AI, returns `score: 0, hint: "Grading temporarily unavailable"` on failure (no fallback)
- Added `try_ai_grade` — spawns `$CLAW_BIN --model $CLAW_MODEL --output-format json prompt "..."` and parses JSON response
- Added `ClawJsonOutput` and `AiGradeResult` structs for JSON deserialization

### Fix — `educlaw-mcp/src/servers/eval_engine/mod.rs`
- Import and call site updated from `grade_essay_maybe_ai` → `grade_essay_ai`

---

## Chat UI — SessionPage & ChatPanel

### Fix — `educlaw-web/src/pages/SessionPage.tsx` (REWRITE)
Previous layout had PTY terminal as the primary panel. Rewritten to:
- Chat panel as the primary view (centre)
- File tree collapsible sidebar (left)
- Assignment panel (right)
- Terminal collapsible at the bottom (always mounted, CSS `h-0`/`h-64` transition so PTY stays live)
- State: `messages: ChatMessage[]`, `chatInput`, `aiTyping`, `terminalOpen`
- Callbacks: `handleChatStart`, `handleChatText`, `handleChatEnd` for streaming frames
- `handleSendWithWs` sends text to PTY and appends user bubble

### Fix — `educlaw-web/src/components/ChatPanel.tsx` (NEW FILE)
Full markdown streaming chat component:
- `react-markdown` + `remark-gfm` for rich text
- `react-syntax-highlighter` (Prism / oneDark theme) for code blocks
- Streaming cursor while AI is typing
- Auto-scroll on new messages
- Auto-grow textarea, Enter to send

### Fix — `educlaw-web/src/hooks/useWebSocket.ts`
- Added `onChatStart`, `onChatText`, `onChatEnd` optional callbacks

### Fix — `educlaw-web/src/api/types.ts`
- Added `ChatMessage`, `ChatRole` types
- Added `chat_start`, `chat_text`, `chat_end` WebSocket envelope types

---

## PTY Bridge — Chat Routing

### Fix — `educlaw-server/src/pty_bridge.rs`
- Added `strip_ansi()` — strips ANSI escape codes from PTY output
- Added `ends_with_repl_prompt()` — detects when claw's REPL prompt is idle
- Added `chat_active: Arc<AtomicBool>` flag and `ctrl_tx/rx` mpsc channel
- `pty_to_ws` task uses `tokio::select!` to route frames as `chat_text` during active turns and raw PTY otherwise

### Fix — `educlaw-mcp/src/server.rs`
- `check_answer` now dispatches to async `check_answer_async`
