# EduClaw System Prompt Design & Override Plan

## Background: What the Agent Sees Today

The claw binary builds the system prompt in a fixed order. Our `instructions.md`
is always last — section #7 — so the hardcoded "coding assistant" identity in
section #1 dominates the model's self-perception.

```
[1] "You are an interactive agent that helps users with software engineering tasks."
    ↑ hardcoded in get_simple_intro_section() — always wins
[2] # System           (tool rules)
[3] # Doing tasks      (code-centric rules)
[4] # Executing actions with care
[5] # Environment context  (cwd, date, model)
[6] # Project context  (git status, file counts)
[7] # Claude instructions  ← our .claw/instructions.md  (read last, often overridden)
```

---

## Two-Layer Fix

We use **two prompt layers** that together give the agent its full student context:

| Layer | What | Where written | When read |
|-------|------|---------------|-----------|
| **Layer 1** — `EDUCLAW_SYSTEM_PROMPT` env var | Replaces section #1 (the hardcoded intro) | `sessions.rs` → `student.json`, `ws.rs` → env | When `build_system_prompt()` runs, before first turn |
| **Layer 2** — `upper/.claw/instructions.md` | Detailed per-session context + workspace tree | `sessions.rs` writes to `upper/` before mount | Section #7; read at same time as Layer 1 |

These two layers complement each other:
- Layer 1 fixes *who the agent thinks it is*
- Layer 2 tells it *what it knows about this specific session*

---

## Layer 1 — EDUCLAW_SYSTEM_PROMPT: The Tutor Intro

This replaces the hardcoded "software engineering tasks" first line.

### Core design principles
1. The agent uses code tools (Python, Bun, bash) **invisibly** — like a calculator.
   The student never hears "I'll write a script". They just get the answer or chart.
2. No IT/programming jargon in student-facing text.
3. The student's subject is the focus. Tools exist to serve the subject, not the other way around.
4. Socratic method: ask one guiding question before giving a full answer.

### Exact text (written by sessions.rs, base64-encoded into EDUCLAW_SYSTEM_PROMPT)

```
You are a personal AI tutor for {display_name}.

Your student is working on their subjects. Your role is to guide their
understanding through questions, clear explanations, and demonstrations that
match their level.

You have tools available: you can run computations, read files, draw charts,
and solve problems. Use them silently in service of the student:
  - Run calculations or draw plots when it helps explain something. Show only
    the result — never the code, unless the student specifically asks "how did
    you do that?"
  - Read subject materials from the workspace to understand what the student is
    studying before responding.
  - When the student shares work, look at it carefully using your file tools
    before commenting.

Language rules (strictly followed):
  - Never use programming terms, technical computing words, or IT jargon with
    the student. They are not studying computing.
  - Keep explanations simple and age-appropriate.
  - Always ask one guiding question before giving a complete answer (Socratic method).
  - If an answer involves a number, calculation, or diagram — compute it; do not
    just describe what the answer would look like.
```

### Why not "be a tutor for math/physics/..."?

Because the cohort contains multiple subjects. The agent should adapt to
whichever subject the student brings up, not be hardcoded to one.
Layer 2 (instructions.md) tells it what subjects are available in the workspace.

---

## Layer 2 — instructions.md: Session Context + Workspace Tree

Written to `upper/.claw/instructions.md` before OverlayFS mounts.
Appears to the agent as `merged/.claw/instructions.md` (section #7).

### What it must contain

1. Who the student is — name, cohort, session id
2. What subjects are available — human-readable titles from subject.json,
   with the slug (folder name) so the agent can navigate to them
3. What files are in each subject — a one-glance map so agent doesn't explore blindly
4. Student's own files — so agent knows if they have previous work
5. Tool guidance specific to the workspace

### Real subject layout (from /srv/educlaw/data/subjects/{cohort_id}/)

```
af64ff83-a0f0-4b4b-adbd-1d4b5d72d5b7/
├── Math-2/
│   ├── subject.json      → title: "Math two for Senior 1 B"
│   ├── MATHEMATICS PAPER 1 UCE 2024.pdf
│   └── MTC 1 GA.pdf
└── Pysic-practicles/
    ├── subject.json      → title: "This contain all related to pysic practicles"
    └── PHYSICS PRACTICAL PAPER DIRECTOR'S SET TERM ONE 2024 S.4.pdf
```

### Target instructions.md output (generated per session)

```markdown
## Student context
- Name: Alice Namukasa
- Session ID: feeb5edf-4a66-401d-9baa-073e5b6937ea

## Workspace layout

/ (workspace root — read-only subject materials)
├── Math-2/                           ← Math two for Senior 1 B
│   ├── MATHEMATICS PAPER 1 UCE 2024.pdf
│   └── MTC 1 GA.pdf
├── Pysic-practicles/                 ← Physics practicals
│   └── PHYSICS PRACTICAL PAPER DIRECTOR'S SET TERM ONE 2024 S.4.pdf
└── .claw/                            ← session config (do not modify)

~/  (student's personal saved files)
  (no personal files yet)

## How to use the workspace
- Use your file-reading tools to open PDFs when the student asks about them.
  Summarise in plain language — do not quote large blocks of raw text.
- Use Python or JavaScript silently to solve equations, draw diagrams, or
  compute answers. Show only the result, not the code.
- The student's personal files (if any) are their saved work. Treat carefully.
```

---

## Workspace Tree Generation — Rust Algorithm (sessions.rs)

Built at create_session time from dirs.lower and dirs.upper **before** mount_overlay.

```rust
fn build_workspace_tree(lower: &Path, upper: &Path) -> String {
    let mut lines = vec!["/ (workspace root — read-only subject materials)".to_string()];

    // Walk lower/ top-level dirs (each is a subject folder)
    let mut subject_dirs = sorted_subdirs(lower);
    let upper_personal = entries_except(upper, ".claw");
    let last_subject_idx = subject_dirs.len().saturating_sub(1);

    for (i, subj) in subject_dirs.iter().enumerate() {
        let is_last = (i == last_subject_idx) && upper_personal.is_empty();
        let conn = if is_last { "└──" } else { "├──" };
        let name = dir_name(subj);
        let title = read_subject_title(subj).unwrap_or_default();
        let comment = if title.is_empty() { String::new() } else { format!("   ← {title}") };

        lines.push(format!("{conn} {name}/{comment}"));

        let child_prefix = if is_last { "    " } else { "│   " };
        let files = sorted_files(subj);           // exclude subject.json itself
        let show = files.len().min(8);
        for (j, f) in files.iter().take(show).enumerate() {
            let fc = if j == show - 1 && files.len() <= 8 { "└──" } else { "├──" };
            lines.push(format!("{child_prefix}{fc} {}", file_name(f)));
        }
        if files.len() > 8 {
            lines.push(format!("{child_prefix}└── ... {} more", files.len() - 8));
        }
    }

    lines.push("└── .claw/                            ← session config (do not modify)".to_string());
    lines.push(String::new());
    lines.push("~/  (student's personal saved files)".to_string());

    if upper_personal.is_empty() {
        lines.push("  (no personal files yet)".to_string());
    } else {
        for entry in upper_personal.iter().take(20) {
            lines.push(format!("  {}", entry_name(entry)));
        }
        if upper_personal.len() > 20 {
            lines.push(format!("  ... {} more", upper_personal.len() - 20));
        }
    }

    lines.join("\n")
}

fn read_subject_title(subject_dir: &Path) -> Option<String> {
    let raw = std::fs::read_to_string(subject_dir.join("subject.json")).ok()?;
    let v: serde_json::Value = serde_json::from_str(&raw).ok()?;
    v["title"].as_str().map(|s| s.to_string())
}
```

**subject.json is excluded from the file listing** — the agent does not need to
see it (it's EduClaw internal metadata, not course material).

---

## Final Assembled System Prompt (What the Agent Sees)

```
[1] You are a personal AI tutor for Alice.          ← EDUCLAW_SYSTEM_PROMPT override ✓
    Your student is working on their subjects.
    [tool rules: use silently]
    [language rules: no jargon, Socratic, compute answers]

[2] # System
    - All text outside tool use is displayed to the user.
    - Tools execute in a user-selected permission mode...

[3] # Doing tasks
    - Read relevant files before responding...     ← fine for a tutor

[4] # Executing actions with care
    - Local reversible actions are usually fine...

[5] # Environment context
    - Working directory: /var/lib/educlaw/.../merged/
    - Date: 2026-04-07  |  Model: claude-opus-4-6

[6] # Project context
    - 2 instruction files discovered.

[7] # Claude instructions
    ## Student context
    - Name: Alice Namukasa  |  Session: feeb5edf...

    ## Workspace layout
    / (workspace root)
    ├── Math-2/                   ← Math two for Senior 1 B
    │   ├── MATHEMATICS PAPER 1 UCE 2024.pdf
    │   └── MTC 1 GA.pdf
    ├── Pysic-practicles/         ← Physics practicals
    │   └── PHYSICS PRACTICAL PAPER...pdf
    └── .claw/

    ~/  (student's personal files)
      (no personal files yet)

    ## How to use the workspace
    - Use file-reading tools on PDFs...
    - Use Python/JS silently for calculations...
```

---

## Implementation Order

### Step 1 — claw-code/crates/runtime/src/prompt.rs
- Add `intro_override: Option<String>` to `SystemPromptBuilder`
- Add `with_intro_override()` builder method
- In `build()`: use `intro_override` in place of `get_simple_intro_section()` when set
- Re-export via `lib.rs` (builder already public)

### Step 2 — claw-code/crates/rusty-claude-cli/src/main.rs
- Add `educlaw_intro_override()` just after `educlaw_structured()`:
  reads `EDUCLAW_SYSTEM_PROMPT`, base64-decodes → `Option<String>`
- In `build_system_prompt()`: if Some, replace `prompt[0]`
- Check Cargo.toml for `base64` crate; add `base64 = "0.22"` if absent

### Step 3 — educlaw-server/src/api/sessions.rs
- Build full tutor intro text with `display_name` interpolated
- base64-encode → store as `"system_prompt_b64"` field in `student.json`
- Replace current minimal `instructions.md` with the full version:
  header + `build_workspace_tree(dirs.lower, dirs.upper)` + tool guidance section
- Add `build_workspace_tree()` and `read_subject_title()` as private helpers

### Step 4 — educlaw-server/src/api/ws.rs
- When reading `student.json`, also extract `"system_prompt_b64"`
- Push `("EDUCLAW_SYSTEM_PROMPT", system_prompt_b64)` into `env_vars`

### Step 5 — Rebuild
- `cargo build --release` in `claw-code/` (produces new claw binary)
- `cargo build --release` in `educlaw-server/`

---

## Env Vars — Final State

| Var | Set by | Read by | Purpose |
|-----|--------|---------|---------|
| `EDUCLAW_STRUCTURED_OUTPUT=1` | ws.rs | main.rs | Machine-readable OSC frames |
| `EDUCLAW_SESSION_ID` | ws.rs | init.sh, mcp | Session UUID |
| `EDUCLAW_WORKSPACE` | ws.rs | init.sh, mcp | Path to merged/ |
| `EDUCLAW_STUDENT_JSON` | ws.rs ← student.json | init.sh | Student blob for settings.json |
| `EDUCLAW_MCP_SOCKET` | ws.rs ← env | init.sh | MCP stdio address |
| `EDUCLAW_INTERNAL_TOKEN` | ws.rs ← env | init.sh, mcp | Bearer token |
| **`EDUCLAW_SYSTEM_PROMPT`** | **ws.rs ← student.json** | **main.rs** | **Tutor intro; replaces "coding assistant"** |

---

## What We Do NOT Change

- `get_simple_doing_tasks_section()` — "read files before responding" applies to tutors too
- `get_actions_section()` — reversibility rules protect the student's workspace
- `get_simple_system_section()` — tool/hook rules must stay intact
- Opt-in: when `EDUCLAW_SYSTEM_PROMPT` is absent (standalone claw, tests),
  section #1 is the original hardcoded text — zero regression
