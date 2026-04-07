# Claw Change Log

All significant changes made to the Claw Source code

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

---

## Structured OSC Output Events

### Problem
No machine-readable contract existed between claw and downstream consumers.
Structured data (tool name, command, stdout, exit code, turn boundaries) was
only available as visual terminal art and had to be reverse-engineered.

### Fix — `claw-code/crates/rusty-claude-cli/src/main.rs`

Added two helpers just after the `DEFAULT_MODEL` constant:

- **`educlaw_structured() -> bool`** — returns `true` when
  `EDUCLAW_STRUCTURED_OUTPUT=1` is set in the environment.
- **`osc_emit(payload: &serde_json::Value)`** — writes an OSC 99 envelope
  (`\x1b]99;{json}\x07`) to stdout and flushes. Same sentinel format already
  used by the Python shell wrapper. No-op when `educlaw_structured()` is false.

Three call sites added:

| Location | Envelope emitted |
|---|---|
| `run_turn()` — before `spinner.tick()` | `{"type":"thinking"}` |
| `run_turn()` — after `spinner.finish()` (success) | `{"type":"chat_end"}` |
| `run_turn()` — after `spinner.fail()` (error) | `{"type":"chat_error","message":"..."}` |
| `format_tool_call_start(name, input)` | `{"type":"tool_start","tool":"bash","command":"python --version","input":{…}}` — command field is the bash `command` arg, or file path for read/write/edit, or pattern for glob/grep |
| `format_tool_result(name, output, is_error)` | `{"type":"tool_end","tool":"bash","exit_code":0,"is_error":false,"stdout":"…","stderr":"…"}` — extracts `stdout`/`stderr` from the structured JSON that tool executors already return internally |

---

## Cancel Mid-Response (Graceful Stop)

### Problem
No way to interrupt a running AI turn without killing the claw process and
losing the session.

### Root Cause
`HookAbortMonitor` only listened for OS `SIGINT` (Ctrl+C from a terminal).
When claw runs with piped stdin under educlaw-server it never receives a
terminal Ctrl+C event, so there was no way to trigger `abort_signal.abort()`
mid-stream. Even if the signal arrived, `run_repl()` propagated the
resulting error out of the loop with `?`, exiting the process.

### Fix — `claw-code/crates/rusty-claude-cli/src/main.rs`

- **`educlaw_cancel_flag()`** — `OnceLock<AtomicBool>` static added after
  `osc_emit`. Reset to `false` at the start of every `run_turn()`.
- **`HookAbortMonitor::spawn()`** — the `ctrl_c()` branch now also stores
  `true` into `educlaw_cancel_flag` before calling `abort_signal.abort()`.
  No separate stdin watcher thread (a `spawn_blocking` read cannot be
  interrupted and would leak across turns).
- **HTTP stream loop** — checks `educlaw_cancel_flag` on each event; returns
  `RuntimeError::new("cancelled")` immediately when set.
- **`run_turn()` error arm** — when the error message equals `"cancelled"`,
  emits `{"type":"chat_end"}` instead of `{"type":"chat_error"}`.
- **`run_repl()`** — `cli.run_turn(&trimmed)?` replaced with a `match` that
  silently `continue`s the REPL loop on a `"cancelled"` error so the session
  is preserved.


