# EduClaw
## Student-Assisted Learning Platform

**Architecture & Full Implementation Plan**

Built on claw-code-parity · Rust + Tokio + Axum · Linux namespaces + OverlayFS

---

> **EduClaw** is a self-hosted, open-source platform that wraps **claw-code-parity** (the multi-model Claude Code equivalent) into a pedagogical shell environment. Every student receives a fully isolated Linux session — namespaced, sandboxed, and scoped — while sharing the same read-only course materials. AI assistance flows through the same terminal, context-aware of who the student is, what class they are in, and what they are working on.

---

## 1  Project Overview & Goals

### 1.1  What is claw-code-parity?

**claw-code-parity** (github.com/ultraworkers/claw-code-parity) is an open-source re-implementation of Claude Code that supports all LLM providers, not just Anthropic. It ships with a plugin and MCP server architecture identical in shape to Claude Code, making it straightforward to:

- Override the system prompt with a custom pedagogical instruction set
- Connect MCP servers that expose scoped resources (files, tools, course APIs)
- Extend behaviour through plugins (syntax highlighting, submission grading, etc.)
- Run as a CLI inside any shell — including a PTY streamed to a browser

### 1.2  EduClaw goals

- Wrap claw-code-parity so every student session gets an AI tutor that knows their identity and class context
- Isolate each student completely at the OS level — process, filesystem, network, and compute
- Preserve the full claw-code-parity experience: shell commands, Python REPL, file editing, plugins
- Stream the TUI to the browser in real-time using xterm.js over WebSocket
- Persist per-student files across sessions while shared course content remains read-only
- Auto-dismiss idle sessions with graceful teardown; data always survives
- Keep MCP servers and plugins as shared infrastructure — one instance serving all isolated sessions

---

## 2  High-Level Architecture

### 2.1  Layer diagram

| Layer |
|-------|
| Browser  (xterm.js + Monaco editor) |
| Axum web server  ·  WebSocket PTY bridge  ·  REST API  ·  Auth |
| Session broker  ·  Idle watchdog  ·  Namespace spawner  ·  UID allocator |
| **Linux user namespace  ·  PID · NET · UTS · IPC namespaces  ·  seccomp BPF  ·  cgroups v2** |
| OverlayFS merged view  →  lower: /classes/  (shared, RO)  +  upper: /data/students/{id}/  (isolated, RW) |
| claw-code-parity CLI  +  Student system prompt  +  MCP clients  inside namespace |
| MCP servers (shared host process)  ·  StudentContext · ClassFiles · StudentFiles · EvalEngine · Shell-in-namespace |

### 2.2  Data flow summary

1. Student opens browser → loads xterm.js UI, authenticates via REST → receives JWT with student_id + class_ids
2. Browser opens WebSocket to `/ws/session/{session_id}`
3. Session broker spawns isolated namespace, mounts OverlayFS, execs bash, attaches PTY
4. claw-code-parity starts inside namespace; broker injects student system prompt as first context message
5. PTY bytes stream bidirectionally: keystrokes in, VT100 output out, rendered live by xterm.js
6. Student works freely — shell, Python, editors — all inside the sandbox; AI tutor responds in the same terminal
7. MCP servers on the host receive requests scoped by student_id extracted from the session JWT
8. On idle timeout or logout: SIGTERM → overlay unmount → namespace release → uid slot freed

---

## 3  Repository & Codebase Structure

Fork claw-code-parity and add the following structure alongside it:

```
educlaw/
├── claw-code-parity/          ← git submodule (upstream, unmodified)
├── educlaw-server/            ← Rust: Axum web server + session broker
│   ├── src/
│   │   ├── main.rs
│   │   ├── auth.rs            ← JWT issue + validate
│   │   ├── session/
│   │   │   ├── broker.rs      ← spawn, track, reap sessions
│   │   │   ├── namespace.rs   ← Linux namespace + OverlayFS logic
│   │   │   ├── pty.rs         ← PTY open, read/write, resize
│   │   │   └── watchdog.rs    ← idle timeout Tokio task
│   │   ├── ws.rs              ← WebSocket handler (PTY bridge)
│   │   ├── api/
│   │   │   ├── students.rs    ← CRUD for student registry
│   │   │   ├── classes.rs     ← class + enrollment management
│   │   │   └── sessions.rs    ← session list, kick, audit
│   │   └── db.rs              ← sqlx SQLite pool + migrations
│   └── Cargo.toml
├── educlaw-mcp/               ← Rust: shared MCP server process
│   ├── src/
│   │   ├── main.rs
│   │   ├── student_context.rs
│   │   ├── class_files.rs
│   │   ├── student_files.rs
│   │   ├── eval_engine.rs
│   │   └── shell_in_ns.rs     ← runs cmds inside student namespace
│   └── Cargo.toml
├── educlaw-web/               ← Browser frontend
│   ├── index.html
│   ├── terminal.js            ← xterm.js + WebSocket glue
│   └── api.js                 ← login, session, REST calls
├── data/
│   ├── classes/               ← shared RO course content (git managed)
│   └── students/              ← per-student upper dirs (gitignored)
├── prompts/
│   └── student_system.md      ← claw-code-parity system prompt override
└── docker-compose.yml         ← optional convenience wrapper
```

---

## 4  Student Registry & Identity System

### 4.1  Database schema

A single SQLite database (`educlaw.db`) holds all persistent state. sqlx with compile-time query checking keeps it safe.

#### 4.1.1  students table

| Column | Type | Notes |
|--------|------|-------|
| student_id | UUID | Primary key, stable across sessions |
| username | TEXT UNIQUE | Login handle |
| password_hash | TEXT | Argon2id hash |
| display_name | TEXT | Shown in Claude greeting |
| role | TEXT | student \| teacher \| admin |
| class_ids | JSON | Array of enrolled class UUIDs |
| uid_slot | INTEGER NULL | Host uid mapping while session active (NULL = idle) |
| created_at | TIMESTAMP | Registration time |
| last_seen | TIMESTAMP | Updated on every login |

#### 4.1.2  sessions table (ephemeral audit)

| Column | Type | Notes |
|--------|------|-------|
| session_id | UUID | Ephemeral, created at login, gone after teardown |
| student_id | UUID FK | Links to students table |
| state | TEXT | active \| idle_warning \| terminating |
| namespace_pid | INTEGER | PID of init process inside namespace |
| pty_fd | INTEGER | PTY master fd for this session |
| overlay_mount | TEXT | Merged mount path inside session tmpdir |
| last_input_at | TIMESTAMP | Updated on every keypress |
| started_at | TIMESTAMP | Session creation time |
| ended_at | TIMESTAMP NULL | NULL while active |

### 4.2  Identity flow into the agent

When a session starts, the broker queries the student record and injects it into the **claw-code-parity** process as its system prompt context, before the student types anything:

```markdown
# prompts/student_system.md (injected at session start)
You are EduClaw, an expert teaching assistant embedded in a student's terminal.

## Student context
- Name:       {{display_name}}
- Student ID: {{student_id}}
- Role:       {{role}}
- Classes:    {{class_names | join(', ')}}
- Session:    {{session_id}}

## Your responsibilities
- Guide, never solve. Ask questions before giving answers.
- Reference course materials in /classes/{{primary_class}}/ when relevant.
- Remind the student of their work in /home/student/ (their private space).
- If the student is stuck, give a hint scoped to the current assignment.
- All file paths you suggest must be inside the student's environment.
- You may run shell commands via the Shell MCP tool — they execute in the
  student's isolated namespace and cannot affect other students.
```

The broker performs template substitution using the student record from SQLite and passes the rendered prompt to **claw-code-parity** via the **`--system-prompt-file`** flag (or equivalent plugin hook).

---

## 5  Session Lifecycle & Isolation

### 5.1  Step-by-step lifecycle

| Phase | Component | What happens |
|-------|-----------|--------------|
| 1. Auth | POST /api/login | Validate creds → issue JWT with student_id, role, class_ids |
| 2. Session request | POST /api/session | Session broker allocates ephemeral session_id + uid slot |
| 3. Namespace spawn | Broker task | fork() → unshare(PID\|NET\|UTS\|IPC\|USER) → write uid_map |
| 4. OverlayFS mount | Broker task | mount(lower=/classes, upper=/data/students/{id}, work=…) |
| 5. Shell exec | Broker task | exec /bin/bash inside merged rootfs, attach PTY |
| 6. Claude Code inject | Shell init | claude CLI starts inside namespace with student system prompt |
| 7. WS bridge up | Axum WS handler | Browser connects; PTY bytes fan-out to all open tabs |
| 8. Idle watchdog | Tokio task | Checks last_input every 30s; warns at T-60s; kills at T |
| 9. Logout / timeout | Broker task | SIGTERM → umount overlay → drop namespace → free uid slot |
| 10. Files persist | OverlayFS upper | /data/students/{id}/ remains on disk for next session |

### 5.2  Isolation matrix

| Layer | Mechanism | What it prevents |
|-------|-----------|-----------------|
| Filesystem | OverlayFS per student | Writes go to /data/students/{id}/upper only; lower=/classes/ is read-only shared |
| Process | PID namespace | Student processes cannot see or signal host or other student processes |
| Network | NET namespace | No network access by default; teacher can open egress per class policy |
| Identity | User namespace + uid map | uid 0 inside → unprivileged uid on host; no privilege escalation possible |
| Compute | cgroups v2 | Per-session CPU, memory, and I/O caps prevent resource exhaustion |
| Syscalls | seccomp BPF allowlist | Only safe syscalls permitted; execve of new processes blocked by default |
| MCP scope | JWT claim enforcement | All MCP server requests carry student_id; server enforces path prefix match before any I/O |
| Claude prompt | System instruction override | Agent receives student identity context injected by session broker before any user turn |

### 5.3  Key Rust implementation: namespace spawner

> **Why no Docker?** All isolation is done with raw Linux kernel primitives via the nix crate — no daemon dependency, no image layers, sub-100ms session startup.

```rust
// session/namespace.rs
use nix::sched::{unshare, CloneFlags};
use nix::mount::{mount, MsFlags};
use std::fs;

pub fn spawn_student_namespace(student_id: &str, uid_slot: u32) -> Result<Pid> {
    // 1. Fork — child will become the namespace init
    match unsafe { nix::unistd::fork() }? {
        ForkResult::Parent { child } => {
            // write uid_map: uid_slot inside → unprivileged host uid
            fs::write(format!("/proc/{child}/uid_map"),
                      format!("0 {uid_slot} 1\n"))?;
            fs::write(format!("/proc/{child}/setgroups"), "deny")?;
            fs::write(format!("/proc/{child}/gid_map"),
                      format!("0 {uid_slot} 1\n"))?;
            Ok(child)
        }
        ForkResult::Child => {
            // 2. Create all namespaces
            unshare(CloneFlags::CLONE_NEWPID  |
                    CloneFlags::CLONE_NEWNET  |
                    CloneFlags::CLONE_NEWIPC  |
                    CloneFlags::CLONE_NEWUTS  |
                    CloneFlags::CLONE_NEWUSER)?;

            // 3. Mount OverlayFS
            let upper = format!("/data/students/{student_id}/upper");
            let work  = format!("/data/students/{student_id}/work");
            let merged = format!("/tmp/ns-{student_id}/merged");
            fs::create_dir_all(&upper)?;
            fs::create_dir_all(&work)?;
            fs::create_dir_all(&merged)?;
            mount(
                Some("overlay"),
                merged.as_str(),
                Some("overlay"),
                MsFlags::empty(),
                Some(format!(
                    "lowerdir=/data/classes,upperdir={upper},workdir={work}"
                ).as_str()),
            )?;

            // 4. exec bash — PTY is already open on parent side
            execvp("/bin/bash", &["/bin/bash"])?;
            unreachable!()
        }
    }
}
```

### 5.4  Idle watchdog

```rust
// session/watchdog.rs
pub async fn watchdog(session_id: Uuid, broker: Arc<SessionBroker>) {
    let timeout   = Duration::from_secs(15 * 60); // 15 min idle
    let warn_at   = timeout - Duration::from_secs(60);
    let mut warned = false;
    loop {
        sleep(Duration::from_secs(30)).await;
        let elapsed = broker.last_input(session_id).elapsed();
        if elapsed >= timeout {
            broker.terminate(session_id, TermReason::IdleTimeout).await;
            return;
        }
        if elapsed >= warn_at && !warned {
            broker.send_to_pty(session_id,
                b"\r\n[EduClaw] Session idle — saving your work. ",
            ).await;
            broker.send_to_pty(session_id,
                b"You have 60 seconds before disconnect.\r\n",
            ).await;
            warned = true;
        }
    }
}
```

---

## 6  MCP Servers & Plugin Strategy

### 6.1  Shared MCP server process

All MCP servers run as a **single shared host process** (educlaw-mcp). They are not isolated — they serve all student sessions simultaneously. Isolation is enforced by **scoping every MCP request using the student_id extracted from the session JWT.**

> **Key insight:** The claw-code-parity instance runs inside the namespace, but its MCP client calls cross back to the host via a Unix socket or loopback TCP. The MCP server validates student_id on every request — no student can reach another student's files, even through MCP.

| MCP Server | Scope | Responsibility |
|-----------|-------|---------------|
| StudentContext MCP | Shared (all sessions) | Exposes read-only student profile, class list, enrolled courses to Claude Code agent |
| ClassFiles MCP | Shared (all sessions) | Serves /classes/ read-only tree — course materials, assignments, reference docs |
| StudentFiles MCP | Shared (all sessions) | Scoped file R/W inside /data/students/{id}/ for the active session — no cross-student access |
| EvalEngine MCP | Shared (all sessions) | Runs test suites against student code, returns structured results + hints to agent |
| ProgressTracker MCP | Shared (all sessions) | Reads/writes student learning progress, quiz scores, completion flags |
| TeacherBroadcast MCP | Shared (teacher only) | Teacher pushes hints or corrections to specific or all sessions |
| Shell MCP | Instance (per session) | Runs shell commands inside the student's namespace — strictly scoped to merged overlay |

### 6.2  MCP request scoping (enforced in Rust)

```rust
// educlaw-mcp/src/student_files.rs
async fn read_file(req: McpRequest, ctx: &SessionCtx) -> McpResult {
    let path = req.params.get("path")?;
    // ENFORCE: path must be inside this student's upper dir
    let base = PathBuf::from(format!("/data/students/{}/upper", ctx.student_id));
    let resolved = base.join(path).canonicalize()?;
    if !resolved.starts_with(&base) {
        return Err(McpError::Forbidden("path outside student directory"));
    }
    Ok(fs::read_to_string(resolved)?.into())
}
```

### 6.3  claw-code-parity plugin: EduClaw hook

A single plugin installed into claw-code-parity **before** packaging handles:

- Injecting the student system prompt at startup (reads `EDUCLAW_STUDENT_JSON` env var set by broker)
- Connecting to the educlaw-mcp Unix socket (set via `EDUCLAW_MCP_SOCKET` env var)
- Disabling any upstream plugins that allow network access or arbitrary code execution outside the namespace
- Intercepting the `/help` command to show EduClaw-specific guidance

---

## 7  Python Environment Inside the Isolation

### 7.1  How Python works inside the namespace

Python runs natively inside the student's namespace — it is just a binary on the merged filesystem view. Because the **lower layer (`/data/classes/`)** includes the host's **`/usr/bin/python3`** and the entire site-packages tree, every student gets full Python immediately with no installation step. Student-installed packages (`pip install --user`) land in their **upper layer** and never bleed across sessions.

> **Key point:** Pre-install all shared libraries into the lower layer image once. Every new student session inherits them instantly via OverlayFS — zero per-student setup cost.

### 7.2  Pre-installed library set (lower layer)

The following libraries are baked into `/data/classes/usr/local/lib/python3.x/site-packages/` as part of the shared lower layer. Students see them immediately; the agent can use them in any code it writes or suggests.

| Library | Category | Educational use |
|---------|----------|----------------|
| numpy | Math / arrays | Numerical computing, matrix ops, linear algebra foundations |
| sympy | Symbolic math | Algebraic manipulation, calculus, equation solving — great for math classes |
| matplotlib | Plotting | 2D/3D graphs; agent saves PNG to student dir, browser renders inline |
| scipy | Science / stats | Statistics, signal processing, optimization, differential equations |
| pandas | Data analysis | Tabular data, CSV processing, data science coursework |
| jupyter (kernel only) | Notebook protocol | Optional: ipykernel so agent can run notebook cells |
| pymupdf (fitz) | PDF extraction | Fast PDF→text/image extraction; preserves layout better than pdfminer |
| pdfminer.six | PDF text | Fallback PDF text extractor; handles complex encoding edge cases |
| markdownify | HTML→MD | Convert HTML class materials to clean Markdown for agent context |
| python-docx | DOCX→text | Extract text from Word documents in the class materials folder |
| Pillow | Image processing | Resize/convert images for browser preview; used by matplotlib pipeline |
| rich | Terminal output | Pretty-print tables, tracebacks, progress bars inside student PTY |

### 7.3  PDF & document extraction pipeline

Teachers drop raw course materials (PDFs, DOCX, HTML) into `/data/classes/{class_id}/raw/`. A conversion pipeline — triggered manually by the teacher or automatically on file drop — extracts them into clean `.md` or `.txt` files alongside the originals. The agent can then read these lightweight files directly into its context window without needing to parse binary formats.

```python
# Conversion pipeline (runs on host, not inside namespace)
# educlaw-mcp/src/converter.py  (called from Rust via std::process::Command)

import fitz          # pymupdf — fastest PDF extractor
import markdownify
from docx import Document
from pathlib import Path

def pdf_to_md(src: Path, dst: Path):
    doc = fitz.open(src)
    pages = []
    for page in doc:
        # Extract text preserving reading order
        text = page.get_text('markdown')  # fitz native MD output
        # Extract images — save as {stem}_fig_{n}.png
        for i, img in enumerate(page.get_images(full=True)):
            xref = img[0]
            pix  = fitz.Pixmap(doc, xref)
            pix.save(dst.parent / f'{dst.stem}_fig_{i}.png')
            text += f'\n![]({dst.stem}_fig_{i}.png)\n'
        pages.append(text)
    dst.write_text('\n\n---\n\n'.join(pages))

def docx_to_md(src: Path, dst: Path):
    doc = Document(src)
    lines = [para.text for para in doc.paragraphs]
    dst.write_text('\n'.join(lines))
```

After conversion the directory looks like:

```
/data/classes/math101/
├── raw/
│   ├── chapter3.pdf          ← original (served to browser as-is)
│   └── chapter3.docx
├── chapter3.md               ← auto-generated, agent reads this
├── chapter3_fig_0.png        ← extracted figures
├── chapter3_fig_1.png
└── chapter3_docx.md          ← from DOCX
```

When a student asks about chapter 3, the agent reads **`chapter3.md`** for text context and sends a **`show_image`** WS message to the browser for any figures — all without the student typing a command.

### 7.4  Agent + Python example flow (math class)

```python
# Student types in terminal:
# 'can you show me how to solve a quadratic equation?'

# Agent (claw-code-parity) generates and runs via Shell MCP:
import sympy as sp

x = sp.Symbol('x')
eq = x**2 - 5*x + 6
roots = sp.solve(eq, x)
print(f'Roots of x² - 5x + 6 = 0 are: {roots}')
# → Roots of x² - 5x + 6 = 0 are: [2, 3]

# Agent then plots it:
import numpy as np
import matplotlib
matplotlib.use('Agg')   # no display needed — saves to file
import matplotlib.pyplot as plt

xs = np.linspace(0, 5, 300)
plt.plot(xs, xs**2 - 5*xs + 6)
plt.axhline(0, color='k', linewidth=0.5)
plt.title('x² - 5x + 6')
plt.savefig('/home/student/quadratic.png', dpi=120)

# Agent then sends WS message:
# { type: 'show_image', path: '/home/student/quadratic.png', scope: 'student' }
# → browser opens the PNG inline in the file panel
```

---

## 8  File Browser & Agent–Browser Communication

### 8.1  Browser layout

The browser UI is split into two panels. The **left panel** is the xterm.js terminal (the full claw-code-parity TUI). The **right panel** is the file browser — a collapsible tree showing both the student's files and the class materials, with a content viewer below it. The agent can drive both panels via the same WebSocket connection.

| Left panel — Terminal | Right panel — File browser + viewer |
|-----------------------|--------------------------------------|
| xterm.js full TUI | ▾ My files (/home/student/) |
| claw-code-parity agent | ▾ Class: math101/ |
| Python REPL / shell | ───────────────── |
| All student input | [image / PDF / code viewer] |

### 8.2  File browser REST API

| Endpoint | Scope | Description |
|----------|-------|-------------|
| GET /api/files/{sid}/tree | Student files | JSON tree of /data/students/{sid}/upper/ — names, sizes, types |
| GET /api/files/{sid}/read | Student files | Serve any file from student upper dir; MIME-typed response |
| GET /api/files/class/{cid}/tree | Class materials | JSON tree of /data/classes/{cid}/ — read-only |
| GET /api/files/class/{cid}/read | Class materials | Serve class file (PDF, image, MD, code) with correct MIME type |
| GET /api/files/{sid}/preview | Smart preview | Returns {type, content}: text rendered as MD, images as base64, PDFs as text via extraction pipeline |
| POST /api/files/class/{cid}/convert | PDF→MD | Teacher triggers PDF→Markdown conversion; result saved alongside original |
| DELETE /api/files/{sid}/path | Student files | Student or agent deletes a file from the upper dir |

> **Security:** All `/api/files/{sid}/...` endpoints validate that the JWT `student_id` matches the `{sid}` path parameter. A student can never request another student's file tree. Class files are read-only for all non-admin roles.

### 8.3  Agent–browser signalling protocol (WsEnvelope)

The WebSocket carries two kinds of traffic on the same connection. Raw binary frames are PTY bytes — xterm.js writes them directly. Text frames are **JSON envelopes** that the browser dispatches to the UI layer. The agent writes envelopes to stdout using a special escape prefix that the PTY bridge intercepts and re-frames as JSON text before forwarding to the browser — so the terminal itself never sees the control messages.

```rust
// ws.rs — intercept agent control messages in PTY output stream
const CTRL_PREFIX: &[u8] = b"\x1b]EduClaw;";
const CTRL_SUFFIX: &[u8] = b"\x07";

fn split_pty_output(raw: &[u8]) -> (Vec<u8>, Option<serde_json::Value>) {
    if let Some(start) = find_subsequence(raw, CTRL_PREFIX) {
        if let Some(end) = find_subsequence(&raw[start..], CTRL_SUFFIX) {
            let json_bytes = &raw[start + CTRL_PREFIX.len()..start + end];
            let terminal_bytes = [&raw[..start], &raw[start+end+CTRL_SUFFIX.len()..]].concat();
            let envelope = serde_json::from_slice(json_bytes).ok();
            return (terminal_bytes, envelope);
        }
    }
    (raw.to_vec(), None)
}
```

The agent (claw-code-parity plugin) emits these envelopes using a helper that writes the OSC escape sequence to stdout — invisible to xterm.js, intercepted by the PTY bridge, forwarded as a JSON text frame to the browser.

| Envelope type | Direction | Effect in browser |
|--------------|-----------|-------------------|
| `{type: 'pty'}` | Terminal bytes | Raw PTY output — xterm.js writes directly; always binary frame |
| `{type: 'open_file'}` | Agent→browser | Agent sends path + scope ('student'\|'class'); browser opens in file panel |
| `{type: 'show_image'}` | Agent→browser | Agent sends path; browser fetches via /api/files and renders inline |
| `{type: 'file_tree'}` | Agent→browser | Agent requests refresh of file panel tree after writing a new file |
| `{type: 'notify'}` | Agent→browser | Toast notification in UI (e.g. 'Chapter 3 notes opened in side panel') |
| `{type: 'resize'}` | Browser→PTY | Browser sends {cols, rows} on terminal resize; server calls TIOCSWINSZ |
| `{type: 'ping'}` | Browser→server | Keepalive; server updates last_input_at to prevent idle timeout |

### 8.4  Browser envelope dispatcher (JS)

```javascript
// educlaw-web/terminal.js
ws.onmessage = (e) => {
  if (typeof e.data === 'string') {
    // JSON control envelope from agent
    const env = JSON.parse(e.data);
    switch (env.type) {
      case 'open_file':
        openFilePanel(env.path, env.scope);     // opens in right panel
        break;
      case 'show_image':
        showImageInPanel(env.path, env.scope);  // renders <img> inline
        break;
      case 'file_tree':
        refreshFileTree(env.scope);             // re-fetches /api/files tree
        break;
      case 'notify':
        showToast(env.message);                 // top-right toast notification
        break;
    }
  } else {
    // Binary frame — raw PTY bytes
    term.write(new Uint8Array(e.data));
  }
};
```

### 8.5  End-to-end example: student asks to see chapter 3

| Step |
|------|
| 1  Student types: 'show me the notes for chapter 3' |
| 2  claw-code-parity agent reads chapter3.md via ClassFiles MCP (scoped to student's enrolled class) |
| 3  Agent summarises key points in the terminal (PTY stream → xterm.js) |
| 4  Agent emits OSC envelope: `{ type: 'open_file', path: 'math101/chapter3.md', scope: 'class' }` |
| 5  PTY bridge strips envelope from byte stream, forwards as JSON text frame to browser |
| 6  Browser dispatcher calls `openFilePanel()` → fetches `/api/files/class/math101/read?path=chapter3.md` |
| 7  Right panel renders the Markdown with syntax highlighting |
| 8  Agent also emits: `{ type: 'show_image', path: 'math101/chapter3_fig_0.png', scope: 'class' }` |
| 9  Browser fetches image via `/api/files/class/math101/read?path=chapter3_fig_0.png` |
| 10 Image renders inline in the viewer panel — student sees diagram without leaving the terminal |

---

## 9  WebSocket PTY Bridge (Updated)

### 9.1  Axum WebSocket handler with envelope splitting

```rust
// ws.rs
async fn ws_handler(
    ws: WebSocketUpgrade,
    Path(session_id): Path<Uuid>,
    State(broker): State<Arc<SessionBroker>>,
    claims: JwtClaims,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move {
        let (mut sink, mut stream) = socket.split();
        let pty_tx = broker.pty_tx(session_id).unwrap();
        let mut pty_rx = broker.pty_rx(session_id).unwrap();

        // PTY → browser (with envelope interception)
        tokio::spawn(async move {
            let mut buf = [0u8; 4096];
            while let Some(bytes) = pty_rx.recv().await {
                let (pty_bytes, envelope) = split_pty_output(&bytes);
                // Forward raw terminal bytes
                if !pty_bytes.is_empty() {
                    sink.send(Message::Binary(pty_bytes)).await.ok();
                }
                // Forward control envelope as JSON text frame
                if let Some(env) = envelope {
                    sink.send(Message::Text(env.to_string())).await.ok();
                }
            }
        });

        // Browser → PTY or server commands
        while let Some(Ok(msg)) = stream.next().await {
            match msg {
                Message::Binary(data) => {
                    broker.update_last_input(session_id);
                    pty_tx.send(data).await.ok();
                }
                Message::Text(json) => {
                    handle_browser_command(session_id, &json, &broker).await;
                }
                _ => {}
            }
        }
    })
}
```

---

## 10  Rust Crate Dependency Plan

| Crate | Purpose | Role in EduClaw |
|-------|---------|----------------|
| tokio | Async runtime | All async tasks, session watchdogs, PTY I/O loops |
| axum | Web framework | HTTP API, WebSocket endpoints, auth middleware |
| tokio-tungstenite | WebSocket | Full-duplex WS: PTY stream + agent→browser control channel |
| portable-pty | PTY management | Spawn PTY, resize events, read/write byte streams |
| nix | Linux syscalls | unshare(), mount(), uid_map, seccomp, pivot_root |
| seccompiler | Seccomp BPF | Build and load syscall allowlist per student session |
| overlayfs / nix::mount | Filesystem | Mount OverlayFS inside user namespace (lower/upper/work) |
| serde / serde_json | Serialization | Student records, session state, WsEnvelope payloads |
| sqlx + SQLite | Database | Student registry, session audit log, file metadata |
| uuid | IDs | Student UUIDs, session tokens, ephemeral mount IDs |
| jsonwebtoken | Auth tokens | JWT session tokens carrying student_id + role claims |
| tower / tower-http | Middleware | Rate limiting, CORS, auth layer, request tracing, static file serve |
| tracing | Observability | Structured logs per session, span-based audit trail |
| mime_guess | MIME types | Detect content type for file browser serve endpoint |
| base64 | Encoding | Encode images/binary files for WS file-preview messages |
| pyo3 (optional) | Python bridge | Call Python conversion scripts from Rust for PDF→MD pipeline |

---

## 11  Teacher & Admin Interface

### 11.1  Admin REST API (Axum)

- `POST /api/admin/students` — create student account (name, username, password, class_ids)
- `GET  /api/admin/students` — list all students with last_seen and active session flag
- `PUT  /api/admin/students/{id}` — update class enrollments, reset password
- `GET  /api/admin/sessions` — live session list with idle time, namespace PID, uid slot
- `DELETE /api/admin/sessions/{id}` — force-terminate a session
- `POST /api/admin/classes` — create class, set course content path in /data/classes/
- `POST /api/admin/classes/{id}/convert` — trigger PDF/DOCX→MD conversion for all raw materials

### 11.2  Teacher broadcast

Teachers can push a message to one or all active sessions via the TeacherBroadcast MCP server. The message appears inline in the student's terminal as a highlighted banner — same PTY, same stream — so no new UI is needed. Teachers can also send a **`show_image`** or **`open_file`** envelope to push a diagram or document to all student panels simultaneously.

> **Privacy note:** Teachers see session metadata (idle time, session ID, class) but cannot read the student's terminal output or files directly. File review happens through the StudentFiles MCP with explicit audit logging.

---

## 12  Deployment

### 12.1  Requirements

- Linux kernel 5.10+ (user namespace + OverlayFS + cgroups v2 support)
- Rust 1.78+ (async, nix, sqlx)
- Python 3.10+ with pymupdf, markdownify, python-docx installed system-wide (for conversion pipeline)
- No Docker required on the host — raw syscalls only
- educlaw-server must run as a process with `CAP_SYS_ADMIN` for mount syscalls (or use `newuidmap`/`newgidmap` helpers for fully rootless)

### 12.2  Directory layout on host

```
/srv/educlaw/
├── educlaw-server          ← compiled binary
├── educlaw-mcp             ← compiled binary
├── educlaw.db              ← SQLite student registry
├── educlaw-web/            ← static browser assets
├── prompts/
│   └── student_system.md
├── data/
│   ├── classes/            ← shared RO course content
│   │   └── math101/
│   │       ├── raw/        ← teacher uploads (PDF, DOCX)
│   │       ├── chapter3.md ← auto-generated MD
│   │       └── chapter3_fig_0.png
│   └── students/           ← per-student upper dirs (persisted)
│       └── {student_id}/
│           ├── upper/      ← student writable layer
│           └── work/       ← OverlayFS work dir
└── config.toml
```

### 12.3  config.toml

```toml
[server]
bind            = "0.0.0.0:8080"
jwt_secret      = "<random 256-bit hex>"

[session]
idle_timeout_s  = 900   # 15 minutes
warn_before_s   = 60
max_sessions    = 200
uid_range_start = 100000
uid_range_end   = 200000

[paths]
classes_dir     = "/srv/educlaw/data/classes"
students_dir    = "/srv/educlaw/data/students"
prompt_file     = "/srv/educlaw/prompts/student_system.md"
mcp_socket      = "/run/educlaw-mcp.sock"

[file_browser]
max_file_size_mb     = 50
image_extensions     = ["png", "jpg", "jpeg", "gif", "svg", "webp"]
text_extensions      = ["md", "txt", "py", "rs", "js", "json", "csv"]
pdf_extraction_tool  = "pymupdf"   # or 'pdfminer'
```

---

## 13  Build Phases

### Phase 1 — Foundation  (weeks 1–3)

1. Fork claw-code-parity; add EduClaw plugin skeleton, OSC envelope emitter, and `--system-prompt-file` support
2. Set up educlaw-server Rust project: Axum, sqlx, JWT auth, student CRUD API
3. Implement namespace spawner (`namespace.rs`) with user namespace + uid mapping
4. Test OverlayFS mount with pre-installed Python libs in lower layer on a dev Linux VM
5. Basic PTY open + WebSocket bridge: binary frames for terminal, text frames for envelopes

### Phase 2 — Session management  (weeks 4–6)

1. Session broker: spawn, register, lookup by student_id, fan-out PTY to multiple WS
2. Idle watchdog Tokio task with configurable timeout and in-terminal warning
3. Graceful teardown: SIGTERM → umount → uid slot release → DB update
4. Resize event handling: TIOCSWINSZ from WS JSON message
5. xterm.js browser UI: login page → session open → split-panel layout (terminal + file browser)

### Phase 3 — MCP & AI integration  (weeks 7–9)

1. educlaw-mcp process: StudentContext, ClassFiles, StudentFiles, EvalEngine servers
2. `student_id` enforcement on every MCP request (path prefix, JWT claim match)
3. Shell-in-namespace MCP: executes commands inside student namespace
4. Student system prompt template injection wired into claw-code-parity plugin
5. Python lib lower layer image: numpy, sympy, matplotlib, scipy, pandas, rich

### Phase 4 — File browser & agent→browser signals  (weeks 10–11)

1. File browser REST API: `/api/files` tree, read, preview endpoints
2. WsEnvelope dispatcher in browser JS: `open_file`, `show_image`, `file_tree`, `notify`
3. PTY bridge OSC envelope interceptor in Rust (`split_pty_output`)
4. PDF→MD conversion pipeline: pymupdf + markdownify, triggered via admin API
5. Image preview: matplotlib PNG output auto-shown in file panel via `show_image` envelope

### Phase 5 — Admin & hardening  (weeks 12–14)

1. Admin REST API: class management, enrollment, force-terminate, live session view, PDF conversion trigger
2. TeacherBroadcast MCP + teacher ability to push `open_file`/`show_image` to all sessions
3. seccomp BPF allowlist via `seccompiler` crate
4. cgroups v2 resource caps per session (CPU %, memory limit, I/O weight)
5. Structured audit logging (`tracing` crate) for every session and file access event
6. Load testing: 100 concurrent sessions, Python workloads, file serving

---

> **Everything that makes claw-code-parity great is preserved.** Students get the full power of an AI-assisted coding environment — shell, Python, file editing, plugin ecosystem, and multi-model AI. The agent sees class materials, knows who the student is, can run Python to explain math live, and can push any file, image, or document directly into the browser panel — all without the student typing a single command.


target/
.claw/sessions/
claw
aut.txt
claw-code-parity/