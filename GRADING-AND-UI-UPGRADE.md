# EduClaw — Grading Upgrade & Chat UI Upgrade

---

## Part 1: How Grading Works Today

### Where the code lives

```
educlaw-mcp/
  src/servers/eval_engine/
    checker.rs   ← the actual grading logic
    spec.rs      ← spec.json types (QuestionSpec, AssignmentSpec)
    mod.rs       ← MCP tools: check_answer, record_answer, get_progress, submit_assignment
```

### The three question types

Every assignment is a `spec.json` file the teacher writes.
Each question has a `"type"` field — one of three values.

---

#### 1. `numeric`

```json
{ "type": "numeric", "expected": 9.81, "tolerance": 0.05, "points": 10 }
```

Grading:  
→ Parse student answer as `f64`.  
→ Compute `|student − expected| ≤ tolerance`.  
→ All-or-nothing: full `points` if within tolerance, 0 if not.

**Works perfectly.** No improvement needed.

---

#### 2. `multiple_choice`

```json
{ "type": "multiple_choice", "expected": "B", "points": 5 }
```

Grading:  
→ Case-insensitive string compare: `student.trim().eq_ignore_ascii_case(expected)`.  
→ All-or-nothing.

**Works perfectly.** No improvement needed.

---

#### 3. `essay`

```json
{
  "type": "essay",
  "rubric_keywords": ["photosynthesis", "chlorophyll", "sunlight", "glucose", "carbon dioxide"],
  "min_keywords": 3,
  "points": 20
}
```

Grading (in `checker.rs → grade_essay()`):

```
1. Lowercase the student answer
2. Count how many rubric_keywords appear in it via str.contains()
3. correct = matched_count >= min_keywords
4. score   = floor( (matched / total_keywords) × points )
```

**Example:**  
Student writes: _"Plants use sunlight and absorb carbon dioxide to make glucose."_  
→ Matched: `sunlight`, `carbon dioxide`, `glucose` = 3 out of 5  
→ `correct = true` (3 ≥ 3)  
→ `score  = floor(3/5 × 20) = 12`

---

### What is wrong with keyword grading for essays

| Problem | Example |
|---------|---------|
| **Synonym blindness** | Student writes "solar energy" instead of "sunlight" → not counted |
| **Copy-paste gaming** | Student writes the keywords once with no coherent sentence → full score |
| **No understanding check** | "I don't know about sunlight, photosynthesis, and chlorophyll" → scored as correct |
| **Context blindness** | Mentions the word in a negation: "plants do NOT use glucose" → still counted as matched |
| **Zero partial credit nuance** | Score is purely mechanical — no reward for good reasoning without exact keywords |
| **No feedback quality** | Hint is just a static string from the spec, not targeted to _what the student got wrong_ |

---

## Part 2: AI-Assisted Essay Grading with `--output-format json`

### The idea

Instead of keyword matching, we spawn `claw` (the AI) in **headless mode**, feed it the rubric and the student answer, and parse a structured JSON score back. No PTY involved — pure process I/O.

### How `--output-format json` actually works (verified from source)

`CliOutputFormat` in real claw only has two variants: `Text` and `Json`. There is no `Ndjson` in real claw — that only exists in the `rusty-claude-cli` parity mock prototype.

When `claw` is launched with:

```bash
claw --output-format json prompt "Grade this essay..."
```

It does NOT open a terminal. It runs ONE turn, blocks until the response is complete, then writes a single JSON object to **stdout** and exits:

```json
{
  "message": "Score: 14/20\n\nFeedback: Good explanation of...",
  "model": "claude-opus-4-6",
  "iterations": 1,
  "tool_uses": [],
  "usage": { "input_tokens": 312, "output_tokens": 98 },
  "estimated_cost": "$0.0004"
}
```

The Rust server reads that stdout, parses the JSON, extracts the score and feedback. Done.

**Important: this is strictly one-shot per invocation.** No streaming. No conversation continuation in a single process. Each `claw prompt` call is a fresh turn that exits immediately after responding.

### Comparison: old vs new

| | Keyword matching (now) | AI grading (proposed) |
|--|--|--|
| Synonyms | ❌ fails | ✅ understands them |
| Context/negation | ❌ ignores | ✅ detects |
| Copy-paste gaming | ❌ rewards it | ✅ detects surface text |
| Partial credit | mechanical fraction | nuanced judgement |
| Feedback to student | static hint string | targeted explanation |
| Speed | ~0 ms (local CPU) | ~1–3 s (LLM call) |
| Cost | free | token cost per submission |
| Offline | ✅ yes | ❌ needs API key |

### Where it would be added in code

In `eval_engine/mod.rs`, the `handle_check_answer` tool currently calls `checker::check_answer()` directly. For questions of type `essay`, it would instead call a new function:

```rust
async fn ai_grade_essay(q: &QuestionSpec, answer: &str) -> CheckOutcome {
    // Build prompt — rubric description, student answer, ask for JSON output
    let prompt = format!(
        "You are a grader. Rubric: {rubric}\n\
         Required concepts: {keywords:?}\n\
         Student answer: {answer}\n\
         Respond ONLY with JSON: {{\"score\": <0-{points}>, \"feedback\": \"...\"}}",
        rubric     = q.hint_on_wrong.as_deref().unwrap_or(""),
        keywords   = q.rubric_keywords.as_deref().unwrap_or(&[]),
        points     = q.points,
        answer     = answer,
    );

    // Spawn claw in headless JSON mode
    let output = Command::new("claw")
        .args(["--output-format", "json", "prompt", &prompt])
        .output()
        .await?;

    // Parse: {"message": "{\"score\":14, \"feedback\":\"...\"}", ...}
    let claw_json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let inner_text = claw_json["message"].as_str()?;
    let graded: serde_json::Value = serde_json::from_str(inner_text)?;
    // Return score + feedback
}
```

Keyword grading stays as **fallback** if claw is not available (no API key).

---

## Part 3: The Chat UI — What Actually Works

### The real constraint (verified from claw source)

Real claw has two modes:

| Mode | Command | What it does |
|---|---|---|
| **REPL** | `claw` (no subcommand) | Multi-turn conversation, reads stdin with rustyline, prints ANSI to terminal. **This is what runs in the PTY right now.** |
| **One-shot JSON** | `claw --output-format json prompt "..."` | Single turn, exits, prints one JSON blob. No streaming. No memory. |

There is **no** streaming ndjson mode in real claw. There is no way to "send messages" to a running headless claw process. The REPL is the only multi-turn option, and it requires a TTY.

---

### Why the current PTY looks like a terminal and not like Claude.ai

Claw in REPL mode renders markdown via **ANSI escape codes**:
- `**bold**` → `\x1b[1mbold\x1b[0m`
- `# Heading` → ANSI bold + color bytes
- Code fences → colored via ANSI

xterm.js interprets those ANSI bytes correctly. The student **does** see bold, colors, and syntax highlighting — but it's inside a terminal emulator widget, not a polished chat bubble with proper HTML layout.

---

### The correct architecture for a ChatGPT-style panel

The REPL is the only multi-turn conversation engine that works. It is already running in the PTY. The solution is not to replace it — it's to **intercept what claw writes to the PTY and render it nicely in the browser** instead of displaying raw ANSI.

```
claw REPL (PTY, unchanged)
    │  writes ANSI markdown text + tool output
    ▼
pty_bridge.rs — split_pty_output() already strips OSC envelopes
    │
    ├── AI response text portion → sent as { "type": "chat_text", "text": "..." }
    │   → ChatPanel.tsx  (react-markdown, renders headings/code/bold)
    │
    └── Non-AI terminal output → sent as binary frame
        → TutorTerminal.tsx (xterm.js, hidden/toggled below)
```

The challenge: today `pty_bridge.rs` can't distinguish "Claude's response text" from "tool execution output" because they're mixed in the same PTY stream.

### The right way to separate them: OSC sentinel from the plugin

The EduClaw plugin (`educlaw-plugin/hooks/`) already uses OSC sequences to send structured signals to the server. We extend that:

- When claw starts a response, the plugin emits: `\x1b]99;{"type":"chat_start"}\x07`
- When claw finishes a response, the plugin emits: `\x1b]99;{"type":"chat_end"}\x07`
- Everything between those markers = AI response text → route to chat panel
- Everything outside = tool/shell output → route to terminal panel

`split_pty_output()` in `pty_bridge.rs` already handles OSC parsing. We just add two more envelope types.

---

### Multi-turn conversation: already solved

Because we're keeping claw in REPL mode (the PTY), history is maintained **natively inside claw** — it's the same conversation the whole session. We don't need to manage history at all. Student sends a message → it goes to PTY stdin → claw responds → response appears in chat panel. Next message continues the same conversation automatically.

---

### Layout: chat primary, terminal toggled

```
┌────────────────────────────────────────────────────────────┐
│  Header (unchanged)                                        │
├────────────┬───────────────────────────────────────────────┤
│            │                                               │
│  File tree │   CHAT PANEL  (primary, full height)          │
│  (left     │   ─────────────────────────────────           │
│  sidebar,  │   Bubbles: User | EduClaw                     │
│  unchanged)│   Markdown rendered by react-markdown         │
│            │   Headings, code blocks, bold, bullets        │
│            │   Streaming word-by-word via websocket        │
│            │                                               │
│            │   ┌───────────────────────────────────┐       │
│            │   │ [Terminal ▼]  ← toggle button     │       │
│            │   └───────────────────────────────────┘       │
│            │                                               │
│            │   (when expanded):                            │
│            │   ┌───────────────────────────────────┐       │
│            │   │ xterm.js — shell, code execution  │       │
│            │   │ student can run python, gcc, etc. │       │
│            │   └───────────────────────────────────┘       │
│            │                                               │
├────────────┴───────────────────────────────────────────────┤
│  Chat input bar                [Send]                      │
└────────────────────────────────────────────────────────────┘
```

Student types in the chat input bar → message goes to PTY stdin (same path as before).  
Claude's reply → routed to chat panel via OSC markers → rendered as markdown HTML.  
Tool/shell output → stays in xterm.js terminal (collapsed by default).  
Student can expand the terminal when they need to run code directly.

---

### What changes in the codebase

| File | Change |
|------|--------|
| `educlaw-plugin/hooks/pre_tool_use.sh` or system prompt | Add OSC `chat_start`/`chat_end` markers around Claude's responses |
| `educlaw-server/src/pty_bridge.rs` | Handle `chat_start`/`chat_end` OSC types; route text between them as `{"type":"chat_text","text":"..."}` WS frames |
| `educlaw-web/src/components/ChatPanel.tsx` | New: message list with User/Assistant bubbles, react-markdown renderer |
| `educlaw-web/src/pages/SessionPage.tsx` | Chat panel as primary area; terminal as collapsible section below |
| `educlaw-web/src/hooks/useWebSocket.ts` | Handle `chat_text` and `chat_end` frame types |
| `educlaw-web/package.json` | Add `react-markdown`, `remark-gfm`, `react-syntax-highlighter` |

**Not changed:** claw itself, the PTY bridge core, conversation history (claw handles it).

---

## Summary

| Question | Correct Answer |
|----------|----------------|
| Does `ndjson` exist in real claw? | **No.** Only in the rusty-claude-cli parity mock. Real claw only has `text` and `json`. |
| Does `--output-format json` stream? | **No.** It runs one turn, blocks, prints one JSON blob, then exits. |
| Can you do multi-turn via `json` mode? | **No.** Each invocation is one-shot with no connection to the previous one. |
| Who manages history in json mode? | You would — server must inject full conversation history into each new prompt string. |
| Can the chat panel have multi-turn? | **Yes** — keep claw in REPL mode in the PTY. Route its text output to the chat panel via OSC markers. |
| How to hide the terminal look? | Add OSC `chat_start`/`chat_end` delimiters; route AI text to react-markdown; collapse xterm.js by default. |
| Where to use `json` mode? | Only for the **AI essay grading** feature — single one-shot calls per grading operation, no history needed. |
