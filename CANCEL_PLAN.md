# Cancel Mid-Response — Implementation Record

Graceful cancel of a running AI turn from the frontend without killing the
claw process or losing session history.

**Status: IMPLEMENTED** ✅

---

## Mechanism (as implemented)

The server sends `SIGINT` to the claw process when the frontend sends a
`{"type":"cancel"}` WebSocket frame. Inside claw, the existing `ctrl_c()`
handler catches SIGINT, sets `educlaw_cancel_flag` and calls
`abort_signal.abort()`. The HTTP stream loop checks the cancel flag each
event and returns `RuntimeError("cancelled")` early. `run_repl()` catches
that specific error, emits `{"type":"chat_end"}` and continues — session
preserved.

> **Why SIGINT and not `\x03` on stdin?**  
> A `spawn_blocking` stdin watcher thread was tried first but it leaked
> across turns: after `HookAbortMonitor::stop()` the blocking `read()` call
> cannot be interrupted, so the thread outlives the turn and consumes bytes
> from the next user message, causing the REPL to receive duplicate input.
> `kill -s INT <pid>` sends the exact same signal as Ctrl+C—the process
> stays alive—and has no threading side effects.

Full flow:
```
User clicks ■ Stop
  → frontend sends {"type":"cancel"} WS frame
  → pty_bridge: tokio::process::Command::new("kill") -s INT <child_pid>
  → claw: ctrl_c() fires → educlaw_cancel_flag = true, abort_signal.abort()
  → HTTP stream loop: checks cancel flag each event, returns Err("cancelled")
  → run_turn(): detects "cancelled" error, emits OSC {"type":"chat_end"}, returns Err
  → run_repl(): catches Err where msg == "cancelled", continues loop
  → frontend receives chat_end frame → aiTyping = false, stop button hidden
```

---

## Changes Made

### 1. `claw-code/crates/rusty-claude-cli/src/main.rs`

#### Change A — `educlaw_cancel_flag()` global  
Added a `OnceLock<AtomicBool>` static (`EDUCLAW_CANCEL`) with a
`educlaw_cancel_flag()` accessor just after the `osc_emit` helper.

#### Change B — `HookAbortMonitor::spawn()` sets cancel flag  
The `ctrl_c()` branch now *also* sets `educlaw_cancel_flag` before calling
`abort_signal.abort()` — no separate stdin watcher thread.

```rust
result = tokio::signal::ctrl_c() => {
    if result.is_ok() {
        educlaw_cancel_flag().store(true, Ordering::SeqCst);
        abort_signal.abort();
    }
}
```

**Revert:** remove the `educlaw_cancel_flag().store(...)` line.

#### Change C — HTTP stream loop cancel check  
At the top of the `while let Some(event) = stream.next_event()...` loop body:

```rust
if educlaw_cancel_flag().load(Ordering::Relaxed) {
    return Err(RuntimeError::new("cancelled"));
}
```

**Revert:** remove the three-line `if` block.

#### Change D — `run_turn()` cancel vs error distinction  
In the `Err(error)` arm, before emitting `chat_error`:

```rust
if educlaw_structured() && error.to_string() == "cancelled" {
    osc_emit(&json!({ "type": "chat_end" }));
} else {
    osc_emit(&json!({ "type": "chat_error", "message": error.to_string() }));
}
```

**Revert:** replace with the single `osc_emit(chat_error)` line.

#### Change E — `run_turn()` resets cancel flag  
At the very top of `run_turn()` before `prepare_turn_runtime()`:

```rust
educlaw_cancel_flag().store(false, Ordering::SeqCst);
```

Ensures a previous cancel does not bleed into the next turn.

**Revert:** remove that one line.

#### Change F — `run_repl()` absorbs cancel error  
`cli.run_turn(&trimmed)?` replaced with:

```rust
match cli.run_turn(&trimmed) {
    Ok(()) => {}
    Err(e) if educlaw_structured() && e.to_string() == "cancelled" => {}
    Err(e) => return Err(e),
}
```

**Revert:** change back to `cli.run_turn(&trimmed)?;`.

---

### 2. `educlaw-server/src/pty_bridge.rs`

#### Change G — `ClientMessage::Cancel` variant  
```rust
Cancel,
```
added to the `ClientMessage` serde enum.

**Revert:** remove the `Cancel` variant.

#### Change H — child PID captured  
```rust
let child_pid: Option<u32> = child.id();
```
captured immediately after `cmd.spawn()`, before stdin/stdout handles are taken.

**Revert:** remove that line and references to `child_pid`.

#### Change I — Cancel handler sends SIGINT  
```rust
ClientMessage::Cancel => {
    if let Some(pid) = child_pid {
        let _ = tokio::process::Command::new("kill")
            .args(["-s", "INT", &pid.to_string()])
            .status()
            .await;
    }
}
```

**Revert:** remove the `ClientMessage::Cancel` arm.

#### Change J — `structured_mode` flag + double-text fix  
```rust
let structured_mode = env_vars
    .iter()
    .any(|(k, v)| k == "EDUCLAW_STRUCTURED_OUTPUT" && v == "1");
```
The Clean-bytes fallback that sent a second `assistant_text` frame is now
gated on `!structured_mode`.

**Revert:** remove the `structured_mode` variable and restore
`if chat_active_out.load(...) && !inside_tool {`.

---

### 3. `educlaw-web/src/hooks/useWebSocket.ts`

#### Change K — `sendCancel` exported  
```typescript
const sendCancel = useCallback(() => {
  send({ type: 'cancel' })
}, [send])
// return { status, sendInput, sendResize, sendCancel }
```

**Revert:** remove `sendCancel` from the hook and return value.

---

### 4. `educlaw-web/src/pages/SessionPage.tsx`

#### Change L — `sendCancel` wired to `ChatPanel`  
`sendCancel` destructured from `useWebSocket`; passed as `onCancel` prop.

**Revert:** remove the destructure and `onCancel` prop.

---

### 5. `educlaw-web/src/components/ChatPanel.tsx`

#### Change M — Stop button  
`onCancel?: () => void` prop added. Button rendered when `disabled && onCancel`:

```tsx
{disabled && onCancel && (
  <button onClick={onCancel}
    className="shrink-0 w-10 h-10 rounded-xl bg-red-500 hover:bg-red-600
               text-white flex items-center justify-center transition-colors shadow-sm"
    title="Stop (cancel current response)">
    <span className="text-base">■</span>
  </button>
)}
```

**Revert:** remove `onCancel` prop and button JSX.

---

## Build Order

1. `cd claw-code && cargo build --release` — builds claw binary
2. Deploy binary (stop server, copy, restart)
3. `cd educlaw-server && cargo build --release`
4. `cd educlaw-web && npm run build`

## Revert Order

Reverse build order. Each change is isolated — any single block can be
removed without affecting the others.


---

## Files to Change

### 1. `claw-code/crates/rusty-claude-cli/src/main.rs`

#### Change A — `HookAbortMonitor::spawn()`
**Location:** `impl HookAbortMonitor` — `fn spawn()`  
**Current:** Only listens for `tokio::signal::ctrl_c()`.  
**New:** When `educlaw_structured()`, also spawn a second blocking thread that
reads one byte from stdin. If the byte is `\x03`, call `abort_signal.abort()`.
Stop that thread via a channel when `HookAbortMonitor::stop()` is called.

```rust
// pseudocode
if educlaw_structured() {
    thread::spawn(move || {
        let mut byte = [0u8; 1];
        loop {
            select! {
                stop => break,
                stdin.read(&mut byte) => {
                    if byte[0] == 0x03 { abort_signal.abort(); break; }
                }
            }
        }
    });
}
```

**Revert:** Remove the `if educlaw_structured()` block entirely. No other
code changes.

---

#### Change B — `run_repl()` error handling
**Location:** `fn run_repl()` — `cli.run_turn(&trimmed)?;`  
**Current:** `?` propagates any error out of the loop, exiting claw.  
**New:** Match on the error — if it is an abort/cancel error, emit
`osc_emit(&json!({"type":"chat_end"}))` and `continue` the loop.
All other errors still propagate with `?`.

```rust
match cli.run_turn(&trimmed) {
    Ok(()) => {}
    Err(e) if is_abort_error(&e) => {
        osc_emit(&json!({"type":"chat_end"}));
        continue;
    }
    Err(e) => return Err(e),
}
```

Need a small helper `fn is_abort_error(e: &dyn Error) -> bool` that checks
the error message contains "aborted" or matches the runtime abort type.

**Revert:** Change back to `cli.run_turn(&trimmed)?;` and delete helper.

---

### 2. `educlaw-server/src/pty_bridge.rs`

#### Change C — `ClientMessage` enum
**Location:** `enum ClientMessage`  
**Current:** `Input { data }` and `Resize { cols, rows }`.  
**New:** Add `Cancel` variant.

```rust
#[serde(rename = "cancel")]
Cancel,
```

**Revert:** Remove `Cancel` variant.

---

#### Change D — input loop cancel handler
**Location:** `ClientMessage::Input` match arm, add new arm below it.  
**New:**
```rust
ClientMessage::Cancel => {
    // Write ETX byte to claw stdin — HookAbortMonitor will pick it up.
    let mut w = stdin.lock().await;
    let _ = w.write_all(b"\x03\n").await;
}
```

**Revert:** Remove the `ClientMessage::Cancel` arm.

---

### 3. `educlaw-web/src/hooks/useWebSocket.ts`

#### Change E — `sendCancel()` export
**Location:** bottom of `useWebSocket` — return object.  
**New:** Add `sendCancel: () => send({ type: 'cancel' })` to the returned object.

**Revert:** Remove `sendCancel` from return and callsites.

---

### 4. `educlaw-web/src/pages/SessionPage.tsx`

#### Change F — Stop button wiring
**Location:** `useWebSocket` call and send section.  
**New:** Destructure `sendCancel` from `useWebSocket`. Pass it to `ChatPanel`
or render a Stop button inline next to the input that appears when `aiTyping`.

**Revert:** Remove `sendCancel` destructure and stop button JSX.

---

### 5. `educlaw-web/src/components/ChatPanel.tsx`

#### Change G — Stop button in input area
**Location:** Input row at bottom of `ChatPanel`.  
**New:** When `disabled` (aiTyping) and a `onCancel` prop is provided, show a
red Stop button alongside the disabled send button.

```tsx
{disabled && onCancel && (
  <button onClick={onCancel} className="...">■ Stop</button>
)}
```

**Revert:** Remove `onCancel` prop and button JSX.

---

## Build Order

1. Build `claw-code` first: `cd claw-code && cargo build --release`
2. Deploy binary (stop server, copy, restart)
3. Build `educlaw-server`: `cd educlaw-server && cargo build --release`
4. Build frontend: `cd educlaw-web && npm run build`

## Revert Order

Reverse of build order. Each change is isolated — removing any one block
restores that component to its previous behaviour without affecting others.

## Risk Notes

- The stdin watcher reads **one byte at a time**. It must be stopped via its
  channel before `read_line_fallback()` runs for the next turn, otherwise it
  could consume part of the next user message. `HookAbortMonitor::stop()` is
  already called immediately after `run_turn()` returns, so timing is safe.
- `\x03\n` — the `\n` after ETX is needed because `read_line_fallback()` may
  have already passed the `\x03` to the stdin watcher and is now blocking on
  the next `\n`. Without it the REPL loop would stall waiting for a newline.
  The watcher absorbs `\x03` and the `\n` is consumed by `read_line_fallback()`
  as an empty line → `ReadOutcome::Submit("")` → `trimmed.is_empty()` →
  `continue` in the REPL loop. Safe.
