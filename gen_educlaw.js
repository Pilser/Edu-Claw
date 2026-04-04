const {
  Document, Packer, Paragraph, TextRun, Table, TableRow, TableCell,
  HeadingLevel, AlignmentType, BorderStyle, WidthType, ShadingType,
  LevelFormat, PageNumber, TabStopType, TabStopPosition,
  Header, Footer, PageBreak
} = require('docx');
const fs = require('fs');

const TEAL    = "0D7A6F";
const TEAL_LT = "E0F4F2";
const BLUE    = "1A4B8C";
const BLUE_LT = "E8EEF8";
const AMB_LT  = "FFF8E6";
const AMB     = "B87A00";
const RED_LT  = "FFF0F0";
const RED     = "C0392B";
const GRY     = "F4F4F4";
const GRY_DK  = "444444";
const BLK     = "1A1A1A";
const WHT     = "FFFFFF";

const border0  = { style: BorderStyle.NONE,   size: 0, color: "FFFFFF" };
const borderTh = { style: BorderStyle.SINGLE, size: 4, color: TEAL };
const borderSm = { style: BorderStyle.SINGLE, size: 1, color: "CCCCCC" };
const allBorderSm = { top: borderSm, bottom: borderSm, left: borderSm, right: borderSm };
const allBorder0  = { top: border0,  bottom: border0,  left: border0,  right: border0  };

function h1(text) {
  return new Paragraph({
    heading: HeadingLevel.HEADING_1,
    spacing: { before: 360, after: 160 },
    children: [new TextRun({ text, font: "Arial", size: 32, bold: true, color: BLUE })]
  });
}
function h2(text) {
  return new Paragraph({
    heading: HeadingLevel.HEADING_2,
    spacing: { before: 280, after: 120 },
    children: [new TextRun({ text, font: "Arial", size: 26, bold: true, color: TEAL })]
  });
}
function h3(text) {
  return new Paragraph({
    heading: HeadingLevel.HEADING_3,
    spacing: { before: 200, after: 100 },
    children: [new TextRun({ text, font: "Arial", size: 22, bold: true, color: GRY_DK })]
  });
}
function body(text, opts = {}) {
  return new Paragraph({
    spacing: { before: 60, after: 100 },
    children: [new TextRun({ text, font: "Arial", size: 22, color: BLK, ...opts })]
  });
}
function bodyMixed(runs) {
  return new Paragraph({
    spacing: { before: 60, after: 100 },
    children: runs.map(r => new TextRun({ font: "Arial", size: 22, color: BLK, ...r }))
  });
}
function bullet(text, level = 0) {
  return new Paragraph({
    numbering: { reference: "bullets", level },
    spacing: { before: 40, after: 60 },
    children: [new TextRun({ text, font: "Arial", size: 22, color: BLK })]
  });
}
function bulletMixed(runs, level = 0) {
  return new Paragraph({
    numbering: { reference: "bullets", level },
    spacing: { before: 40, after: 60 },
    children: runs.map(r => new TextRun({ font: "Arial", size: 22, color: BLK, ...r }))
  });
}
function numbered(text, level = 0) {
  return new Paragraph({
    numbering: { reference: "numbers", level },
    spacing: { before: 40, after: 60 },
    children: [new TextRun({ text, font: "Arial", size: 22, color: BLK })]
  });
}
function code(text) {
  return new Paragraph({
    spacing: { before: 80, after: 80 },
    shading: { fill: "1E1E1E", type: ShadingType.CLEAR },
    indent: { left: 360 },
    children: [new TextRun({ text, font: "Courier New", size: 18, color: "D4D4D4" })]
  });
}
function blank(space = 120) {
  return new Paragraph({ spacing: { before: space, after: 0 }, children: [] });
}
function hr() {
  return new Paragraph({
    spacing: { before: 160, after: 160 },
    border: { bottom: { style: BorderStyle.SINGLE, size: 4, color: TEAL } },
    children: []
  });
}

function callout(label, text, bgColor, labelColor) {
  return new Table({
    width: { size: 9360, type: WidthType.DXA },
    columnWidths: [9360],
    rows: [new TableRow({ children: [new TableCell({
      borders: allBorder0,
      shading: { fill: bgColor, type: ShadingType.CLEAR },
      margins: { top: 160, bottom: 160, left: 200, right: 200 },
      children: [
        new Paragraph({ spacing: { before: 0, after: 60 }, children: [
          new TextRun({ text: label + "  ", font: "Arial", size: 20, bold: true, color: labelColor }),
          new TextRun({ text, font: "Arial", size: 20, color: BLK })
        ]})
      ]
    })]})],
  });
}

function twoCol(left, right, leftW = 4400, rightW = 4960) {
  return new Table({
    width: { size: 9360, type: WidthType.DXA },
    columnWidths: [leftW, rightW],
    rows: [new TableRow({ children: [
      new TableCell({ borders: allBorder0, width: { size: leftW, type: WidthType.DXA },
        margins: { top: 40, bottom: 40, left: 0, right: 200 }, children: left }),
      new TableCell({ borders: allBorder0, width: { size: rightW, type: WidthType.DXA },
        margins: { top: 40, bottom: 40, left: 200, right: 0 }, children: right }),
    ]})],
  });
}

function headerTable(cols) {
  // cols: [{text, width, bg}]
  const colWidths = cols.map(c => c.width);
  return new Table({
    width: { size: 9360, type: WidthType.DXA },
    columnWidths: colWidths,
    rows: [new TableRow({ children: cols.map(c => new TableCell({
      borders: allBorderSm,
      shading: { fill: c.bg || BLUE, type: ShadingType.CLEAR },
      width: { size: c.width, type: WidthType.DXA },
      margins: { top: 100, bottom: 100, left: 140, right: 140 },
      children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [
        new TextRun({ text: c.text, font: "Arial", size: 20, bold: true, color: WHT })
      ]})]
    }))})],
  });
}

function dataRow(cells, widths, shade = false) {
  return new TableRow({ children: cells.map((c, i) => new TableCell({
    borders: allBorderSm,
    shading: { fill: shade ? GRY : WHT, type: ShadingType.CLEAR },
    width: { size: widths[i], type: WidthType.DXA },
    margins: { top: 80, bottom: 80, left: 140, right: 140 },
    children: [new Paragraph({ children: [
      new TextRun({ text: c.text || c, font: "Arial", size: 20, color: c.color || BLK, bold: c.bold || false })
    ]})]
  }))});
}

function sectionBanner(text) {
  return new Table({
    width: { size: 9360, type: WidthType.DXA },
    columnWidths: [9360],
    rows: [new TableRow({ children: [new TableCell({
      borders: allBorder0,
      shading: { fill: BLUE, type: ShadingType.CLEAR },
      margins: { top: 180, bottom: 180, left: 280, right: 280 },
      children: [new Paragraph({ alignment: AlignmentType.LEFT, children: [
        new TextRun({ text, font: "Arial", size: 28, bold: true, color: WHT })
      ]})]
    })]})],
  });
}

const crateWidths = [2800, 1800, 4760];
const crateRows = [
  ["tokio", "Async runtime", "All async tasks, session watchdogs, PTY I/O loops"],
  ["axum", "Web framework", "HTTP API, WebSocket endpoints, auth middleware"],
  ["tokio-tungstenite", "WebSocket", "Full-duplex WS: PTY stream + agent→browser control channel"],
  ["portable-pty", "PTY management", "Spawn PTY, resize events, read/write byte streams"],
  ["nix", "Linux syscalls", "unshare(), mount(), uid_map, seccomp, pivot_root"],
  ["seccompiler", "Seccomp BPF", "Build and load syscall allowlist per student session"],
  ["overlayfs / nix::mount", "Filesystem", "Mount OverlayFS inside user namespace (lower/upper/work)"],
  ["serde / serde_json", "Serialization", "Student records, session state, WsEnvelope payloads"],
  ["sqlx + SQLite", "Database", "Student registry, session audit log, file metadata"],
  ["uuid", "IDs", "Student UUIDs, session tokens, ephemeral mount IDs"],
  ["jsonwebtoken", "Auth tokens", "JWT session tokens carrying student_id + role claims"],
  ["tower / tower-http", "Middleware", "Rate limiting, CORS, auth layer, request tracing, static file serve"],
  ["tracing", "Observability", "Structured logs per session, span-based audit trail"],
  ["mime_guess", "MIME types", "Detect content type for file browser serve endpoint"],
  ["base64", "Encoding", "Encode images/binary files for WS file-preview messages"],
  ["pyo3 (optional)", "Python bridge", "Call Python conversion scripts from Rust for PDF→MD pipeline"],
];

const pythonLibRows = [
  ["numpy", "Math / arrays", "Numerical computing, matrix ops, linear algebra foundations"],
  ["sympy", "Symbolic math", "Algebraic manipulation, calculus, equation solving — great for math classes"],
  ["matplotlib", "Plotting", "2D/3D graphs; agent saves PNG to student dir, browser renders inline"],
  ["scipy", "Science / stats", "Statistics, signal processing, optimization, differential equations"],
  ["pandas", "Data analysis", "Tabular data, CSV processing, data science coursework"],
  ["jupyter (kernel only)", "Notebook protocol", "Optional: ipykernel so agent can run notebook cells"],
  ["pymupdf (fitz)", "PDF extraction", "Fast PDF→text/image extraction; preserves layout better than pdfminer"],
  ["pdfminer.six", "PDF text", "Fallback PDF text extractor; handles complex encoding edge cases"],
  ["markdownify", "HTML→MD", "Convert HTML class materials to clean Markdown for agent context"],
  ["python-docx", "DOCX→text", "Extract text from Word documents in the class materials folder"],
  ["Pillow", "Image processing", "Resize/convert images for browser preview; used by matplotlib pipeline"],
  ["rich", "Terminal output", "Pretty-print tables, tracebacks, progress bars inside student PTY"],
];

const fileBrowserApiRows = [
  ["GET /api/files/{sid}/tree", "Student files", "JSON tree of /data/students/{sid}/upper/ — names, sizes, types"],
  ["GET /api/files/{sid}/read", "Student files", "Serve any file from student upper dir; MIME-typed response"],
  ["GET /api/files/class/{cid}/tree", "Class materials", "JSON tree of /data/classes/{cid}/ — read-only"],
  ["GET /api/files/class/{cid}/read", "Class materials", "Serve class file (PDF, image, MD, code) with correct MIME type"],
  ["GET /api/files/{sid}/preview", "Smart preview", "Returns {type, content}: text rendered as MD, images as base64, PDFs as text via extraction pipeline"],
  ["POST /api/files/class/{cid}/convert", "PDF→MD", "Teacher triggers PDF→Markdown conversion; result saved alongside original"],
  ["DELETE /api/files/{sid}/path", "Student files", "Student or agent deletes a file from the upper dir"],
];

const wsEnvelopeRows = [
  ["{type: 'pty'}", "Terminal bytes", "Raw PTY output — xterm.js writes directly; always binary frame"],
  ["{type: 'open_file'}", "Agent→browser", "Agent sends path + scope ('student'|'class'); browser opens in file panel"],
  ["{type: 'show_image'}", "Agent→browser", "Agent sends path; browser fetches via /api/files and renders inline"],
  ["{type: 'file_tree'}", "Agent→browser", "Agent requests refresh of file panel tree after writing a new file"],
  ["{type: 'notify'}", "Agent→browser", "Toast notification in UI (e.g. 'Chapter 3 notes opened in side panel')"],
  ["{type: 'resize'}", "Browser→PTY", "Browser sends {cols, rows} on terminal resize; server calls TIOCSWINSZ"],
  ["{type: 'ping'}", "Browser→server", "Keepalive; server updates last_input_at to prevent idle timeout"],
];

const lifecycleWidths = [2200, 2000, 5160];
const lifecycleRows = [
  ["1. Auth", "POST /api/login", "Validate creds → issue JWT with student_id, role, class_ids"],
  ["2. Session request", "POST /api/session", "Session broker allocates ephemeral session_id + uid slot"],
  ["3. Namespace spawn", "Broker task", "fork() → unshare(PID|NET|UTS|IPC|USER) → write uid_map"],
  ["4. OverlayFS mount", "Broker task", "mount(lower=/classes, upper=/data/students/{id}, work=…)"],
  ["5. Shell exec", "Broker task", "exec /bin/bash inside merged rootfs, attach PTY"],
  ["6. Claude Code inject", "Shell init", "claude CLI starts inside namespace with student system prompt"],
  ["7. WS bridge up", "Axum WS handler", "Browser connects; PTY bytes fan-out to all open tabs"],
  ["8. Idle watchdog", "Tokio task", "Checks last_input every 30s; warns at T-60s; kills at T"],
  ["9. Logout / timeout", "Broker task", "SIGTERM → umount overlay → drop namespace → free uid slot"],
  ["10. Files persist", "OverlayFS upper", "/data/students/{id}/ remains on disk for next session"],
];

const studentFields = [
  ["student_id", "UUID", "Primary key, stable across sessions"],
  ["username", "TEXT UNIQUE", "Login handle"],
  ["password_hash", "TEXT", "Argon2id hash"],
  ["display_name", "TEXT", "Shown in Claude greeting"],
  ["role", "TEXT", "student | teacher | admin"],
  ["class_ids", "JSON", "Array of enrolled class UUIDs"],
  ["uid_slot", "INTEGER NULL", "Host uid mapping while session active (NULL = idle)"],
  ["created_at", "TIMESTAMP", "Registration time"],
  ["last_seen", "TIMESTAMP", "Updated on every login"],
];

const sessionFields = [
  ["session_id", "UUID", "Ephemeral, created at login, gone after teardown"],
  ["student_id", "UUID FK", "Links to students table"],
  ["state", "TEXT", "active | idle_warning | terminating"],
  ["namespace_pid", "INTEGER", "PID of init process inside namespace"],
  ["pty_fd", "INTEGER", "PTY master fd for this session"],
  ["overlay_mount", "TEXT", "Merged mount path inside session tmpdir"],
  ["last_input_at", "TIMESTAMP", "Updated on every keypress"],
  ["started_at", "TIMESTAMP", "Session creation time"],
  ["ended_at", "TIMESTAMP NULL", "NULL while active"],
];

const mcpRows = [
  ["StudentContext MCP", "Shared (all sessions)", "Exposes read-only student profile, class list, enrolled courses to Claude Code agent"],
  ["ClassFiles MCP", "Shared (all sessions)", "Serves /classes/ read-only tree — course materials, assignments, reference docs"],
  ["StudentFiles MCP", "Shared (all sessions)", "Scoped file R/W inside /data/students/{id}/ for the active session — no cross-student access"],
  ["EvalEngine MCP", "Shared (all sessions)", "Runs test suites against student code, returns structured results + hints to agent"],
  ["ProgressTracker MCP", "Shared (all sessions)", "Reads/writes student learning progress, quiz scores, completion flags"],
  ["TeacherBroadcast MCP", "Shared (teacher only)", "Teacher pushes hints or corrections to specific or all sessions"],
  ["Shell MCP", "Instance (per session)", "Runs shell commands inside the student's namespace — strictly scoped to merged overlay"],
];

const isolationRows = [
  ["Filesystem", "OverlayFS per student", "Writes go to /data/students/{id}/upper only; lower=/classes/ is read-only shared"],
  ["Process", "PID namespace", "Student processes cannot see or signal host or other student processes"],
  ["Network", "NET namespace", "No network access by default; teacher can open egress per class policy"],
  ["Identity", "User namespace + uid map", "uid 0 inside → unprivileged uid on host; no privilege escalation possible"],
  ["Compute", "cgroups v2", "Per-session CPU, memory, and I/O caps prevent resource exhaustion"],
  ["Syscalls", "seccomp BPF allowlist", "Only safe syscalls permitted; execve of new processes blocked by default"],
  ["MCP scope", "JWT claim enforcement", "All MCP server requests carry student_id; server enforces path prefix match before any I/O"],
  ["Claude prompt", "System instruction override", "Agent receives student identity context injected by session broker before any user turn"],
];

const doc = new Document({
  numbering: {
    config: [
      { reference: "bullets", levels: [{
        level: 0, format: LevelFormat.BULLET, text: "•", alignment: AlignmentType.LEFT,
        style: { paragraph: { indent: { left: 720, hanging: 360 } } }
      },{
        level: 1, format: LevelFormat.BULLET, text: "◦", alignment: AlignmentType.LEFT,
        style: { paragraph: { indent: { left: 1080, hanging: 360 } } }
      }]},
      { reference: "numbers", levels: [{
        level: 0, format: LevelFormat.DECIMAL, text: "%1.", alignment: AlignmentType.LEFT,
        style: { paragraph: { indent: { left: 720, hanging: 360 } } }
      }]},
    ]
  },
  styles: {
    default: { document: { run: { font: "Arial", size: 22 } } },
    paragraphStyles: [
      { id: "Heading1", name: "Heading 1", basedOn: "Normal", next: "Normal", quickFormat: true,
        run: { size: 32, bold: true, font: "Arial", color: BLUE },
        paragraph: { spacing: { before: 360, after: 160 }, outlineLevel: 0 } },
      { id: "Heading2", name: "Heading 2", basedOn: "Normal", next: "Normal", quickFormat: true,
        run: { size: 26, bold: true, font: "Arial", color: TEAL },
        paragraph: { spacing: { before: 280, after: 120 }, outlineLevel: 1 } },
      { id: "Heading3", name: "Heading 3", basedOn: "Normal", next: "Normal", quickFormat: true,
        run: { size: 22, bold: true, font: "Arial", color: GRY_DK },
        paragraph: { spacing: { before: 200, after: 100 }, outlineLevel: 2 } },
    ]
  },
  sections: [{
    properties: {
      page: {
        size: { width: 12240, height: 15840 },
        margin: { top: 1440, right: 1440, bottom: 1440, left: 1440 }
      }
    },
    headers: {
      default: new Header({ children: [
        new Table({
          width: { size: 9360, type: WidthType.DXA },
          columnWidths: [6500, 2860],
          rows: [new TableRow({ children: [
            new TableCell({ borders: allBorder0, width: { size: 6500, type: WidthType.DXA },
              margins: { top: 60, bottom: 60, left: 0, right: 0 },
              children: [new Paragraph({ children: [new TextRun({ text: "EduClaw — Student-Assisted Learning Platform", font: "Arial", size: 18, bold: true, color: BLUE })] })] }),
            new TableCell({ borders: allBorder0, width: { size: 2860, type: WidthType.DXA },
              margins: { top: 60, bottom: 60, left: 0, right: 0 },
              children: [new Paragraph({ alignment: AlignmentType.RIGHT, children: [new TextRun({ text: "Architecture & Implementation Plan", font: "Arial", size: 18, color: GRY_DK })] })] }),
          ]})],
        }),
        new Paragraph({ border: { bottom: { style: BorderStyle.SINGLE, size: 4, color: TEAL } }, children: [] })
      ]})
    },
    footers: {
      default: new Footer({ children: [
        new Paragraph({ border: { top: { style: BorderStyle.SINGLE, size: 4, color: TEAL } },
          alignment: AlignmentType.CENTER,
          children: [
            new TextRun({ text: "EduClaw  ·  Confidential  ·  Page ", font: "Arial", size: 18, color: GRY_DK }),
            new TextRun({ children: [PageNumber.CURRENT], font: "Arial", size: 18, color: GRY_DK }),
          ]})
      ]})
    },
    children: [

      // ─── COVER ──────────────────────────────────────────────────────────────
      new Table({
        width: { size: 9360, type: WidthType.DXA }, columnWidths: [9360],
        rows: [new TableRow({ children: [new TableCell({
          borders: allBorder0,
          shading: { fill: BLUE, type: ShadingType.CLEAR },
          margins: { top: 800, bottom: 800, left: 600, right: 600 },
          children: [
            new Paragraph({ alignment: AlignmentType.LEFT, spacing: { before: 0, after: 120 }, children: [
              new TextRun({ text: "EduClaw", font: "Arial", size: 72, bold: true, color: WHT })
            ]}),
            new Paragraph({ alignment: AlignmentType.LEFT, spacing: { before: 0, after: 200 }, children: [
              new TextRun({ text: "Student-Assisted Learning Platform", font: "Arial", size: 36, color: "B3D4FF" })
            ]}),
            new Paragraph({ alignment: AlignmentType.LEFT, spacing: { before: 0, after: 80 }, children: [
              new TextRun({ text: "Architecture & Full Implementation Plan", font: "Arial", size: 26, color: "D0E8FF" })
            ]}),
            new Paragraph({ alignment: AlignmentType.LEFT, spacing: { before: 200, after: 0 }, children: [
              new TextRun({ text: "Built on claw-code-parity  ·  Rust + Tokio + Axum  ·  Linux namespaces + OverlayFS", font: "Arial", size: 20, color: "8BBCEE" })
            ]}),
          ]
        })]})],
      }),

      blank(400),

      // Intro summary
      new Table({
        width: { size: 9360, type: WidthType.DXA }, columnWidths: [9360],
        rows: [new TableRow({ children: [new TableCell({
          borders: { top: borderTh, bottom: borderTh, left: { style: BorderStyle.SINGLE, size: 20, color: TEAL }, right: border0 },
          shading: { fill: TEAL_LT, type: ShadingType.CLEAR },
          margins: { top: 200, bottom: 200, left: 240, right: 240 },
          children: [
            new Paragraph({ spacing: { before: 0, after: 80 }, children: [
              new TextRun({ text: "EduClaw ", font: "Arial", size: 24, bold: true, color: TEAL }),
              new TextRun({ text: "is a self-hosted, open-source platform that wraps ", font: "Arial", size: 24, color: BLK }),
              new TextRun({ text: "claw-code-parity", font: "Arial", size: 24, bold: true, color: BLUE }),
              new TextRun({ text: " (the multi-model Claude Code equivalent) into a pedagogical shell environment. Every student receives a fully isolated Linux session — namespaced, sandboxed, and scoped — while sharing the same read-only course materials. AI assistance flows through the same terminal, context-aware of who the student is, what class they are in, and what they are working on.", font: "Arial", size: 24, color: BLK }),
            ]}),
          ]
        })]})],
      }),

      blank(300),

      // ─── SECTION 1 ──────────────────────────────────────────────────────────
      sectionBanner("1  Project Overview & Goals"),
      blank(160),

      h2("1.1  What is claw-code-parity?"),
      bodyMixed([
        { text: "claw-code-parity", bold: true },
        { text: " (github.com/ultraworkers/claw-code-parity) is an open-source re-implementation of Claude Code that supports all LLM providers, not just Anthropic. It ships with a plugin and MCP server architecture identical in shape to Claude Code, making it straightforward to:" },
      ]),
      bullet("Override the system prompt with a custom pedagogical instruction set"),
      bullet("Connect MCP servers that expose scoped resources (files, tools, course APIs)"),
      bullet("Extend behaviour through plugins (syntax highlighting, submission grading, etc.)"),
      bullet("Run as a CLI inside any shell — including a PTY streamed to a browser"),
      blank(80),

      h2("1.2  EduClaw goals"),
      bullet("Wrap claw-code-parity so every student session gets an AI tutor that knows their identity and class context"),
      bullet("Isolate each student completely at the OS level — process, filesystem, network, and compute"),
      bullet("Preserve the full claw-code-parity experience: shell commands, Python REPL, file editing, plugins"),
      bullet("Stream the TUI to the browser in real-time using xterm.js over WebSocket"),
      bullet("Persist per-student files across sessions while shared course content remains read-only"),
      bullet("Auto-dismiss idle sessions with graceful teardown; data always survives"),
      bullet("Keep MCP servers and plugins as shared infrastructure — one instance serving all isolated sessions"),
      blank(200),

      // ─── SECTION 2 ──────────────────────────────────────────────────────────
      sectionBanner("2  High-Level Architecture"),
      blank(160),

      h2("2.1  Layer diagram"),
      new Table({
        width: { size: 9360, type: WidthType.DXA }, columnWidths: [9360],
        rows: [
          ...["Browser  (xterm.js + Monaco editor)", "Axum web server  ·  WebSocket PTY bridge  ·  REST API  ·  Auth", "Session broker  ·  Idle watchdog  ·  Namespace spawner  ·  UID allocator", "Linux user namespace  ·  PID · NET · UTS · IPC namespaces  ·  seccomp BPF  ·  cgroups v2", "OverlayFS merged view  →  lower: /classes/  (shared, RO)  +  upper: /data/students/{id}/  (isolated, RW)", "claw-code-parity CLI  +  Student system prompt  +  MCP clients  inside namespace", "MCP servers (shared host process)  ·  StudentContext · ClassFiles · StudentFiles · EvalEngine · Shell-in-namespace"].map((t, i) => new TableRow({ children: [new TableCell({
            borders: allBorder0,
            shading: { fill: i % 2 === 0 ? BLUE_LT : GRY, type: ShadingType.CLEAR },
            margins: { top: 120, bottom: 120, left: 200, right: 200 },
            children: [new Paragraph({ children: [new TextRun({ text: t, font: "Arial", size: 20, bold: i === 3, color: i === 3 ? TEAL : BLK })] })]
          })]})
        )],
      }),

      blank(160),
      h2("2.2  Data flow summary"),
      numbered("Student opens browser → loads xterm.js UI, authenticates via REST → receives JWT with student_id + class_ids"),
      numbered("Browser opens WebSocket to /ws/session/{session_id}"),
      numbered("Session broker spawns isolated namespace, mounts OverlayFS, execs bash, attaches PTY"),
      numbered("claw-code-parity starts inside namespace; broker injects student system prompt as first context message"),
      numbered("PTY bytes stream bidirectionally: keystrokes in, VT100 output out, rendered live by xterm.js"),
      numbered("Student works freely — shell, Python, editors — all inside the sandbox; AI tutor responds in the same terminal"),
      numbered("MCP servers on the host receive requests scoped by student_id extracted from the session JWT"),
      numbered("On idle timeout or logout: SIGTERM → overlay unmount → namespace release → uid slot freed"),
      blank(200),

      // ─── SECTION 3 ──────────────────────────────────────────────────────────
      sectionBanner("3  Repository & Codebase Structure"),
      blank(160),
      body("Fork claw-code-parity and add the following structure alongside it:"),
      blank(60),

      ...[
        "educlaw/",
        "├── claw-code-parity/          ← git submodule (upstream, unmodified)",
        "├── educlaw-server/            ← Rust: Axum web server + session broker",
        "│   ├── src/",
        "│   │   ├── main.rs",
        "│   │   ├── auth.rs            ← JWT issue + validate",
        "│   │   ├── session/",
        "│   │   │   ├── broker.rs      ← spawn, track, reap sessions",
        "│   │   │   ├── namespace.rs   ← Linux namespace + OverlayFS logic",
        "│   │   │   ├── pty.rs         ← PTY open, read/write, resize",
        "│   │   │   └── watchdog.rs    ← idle timeout Tokio task",
        "│   │   ├── ws.rs              ← WebSocket handler (PTY bridge)",
        "│   │   ├── api/",
        "│   │   │   ├── students.rs    ← CRUD for student registry",
        "│   │   │   ├── classes.rs     ← class + enrollment management",
        "│   │   │   └── sessions.rs    ← session list, kick, audit",
        "│   │   └── db.rs              ← sqlx SQLite pool + migrations",
        "│   └── Cargo.toml",
        "├── educlaw-mcp/               ← Rust: shared MCP server process",
        "│   ├── src/",
        "│   │   ├── main.rs",
        "│   │   ├── student_context.rs",
        "│   │   ├── class_files.rs",
        "│   │   ├── student_files.rs",
        "│   │   ├── eval_engine.rs",
        "│   │   └── shell_in_ns.rs     ← runs cmds inside student namespace",
        "│   └── Cargo.toml",
        "├── educlaw-web/               ← Browser frontend",
        "│   ├── index.html",
        "│   ├── terminal.js            ← xterm.js + WebSocket glue",
        "│   └── api.js                 ← login, session, REST calls",
        "├── data/",
        "│   ├── classes/               ← shared RO course content (git managed)",
        "│   └── students/              ← per-student upper dirs (gitignored)",
        "├── prompts/",
        "│   └── student_system.md      ← claw-code-parity system prompt override",
        "└── docker-compose.yml         ← optional convenience wrapper",
      ].map(l => code(l)),

      blank(200),

      // ─── SECTION 4 ──────────────────────────────────────────────────────────
      sectionBanner("4  Student Registry & Identity System"),
      blank(160),

      h2("4.1  Database schema"),
      body("A single SQLite database (educlaw.db) holds all persistent state. sqlx with compile-time query checking keeps it safe."),
      blank(80),

      h3("4.1.1  students table"),
      new Table({
        width: { size: 9360, type: WidthType.DXA }, columnWidths: [2200, 1800, 5360],
        rows: [
          new TableRow({ children: ["Column", "Type", "Notes"].map((t, i) => new TableCell({
            borders: allBorderSm,
            shading: { fill: BLUE, type: ShadingType.CLEAR },
            width: { size: [2200,1800,5360][i], type: WidthType.DXA },
            margins: { top: 80, bottom: 80, left: 140, right: 140 },
            children: [new Paragraph({ children: [new TextRun({ text: t, font: "Arial", size: 20, bold: true, color: WHT })]})]
          }))},),
          ...studentFields.map(([col, type, note], i) => dataRow(
            [{ text: col, bold: true, color: TEAL }, type, note], [2200,1800,5360], i % 2 === 1
          )),
        ],
      }),

      blank(140),
      h3("4.1.2  sessions table (ephemeral audit)"),
      new Table({
        width: { size: 9360, type: WidthType.DXA }, columnWidths: [2400, 1800, 5160],
        rows: [
          new TableRow({ children: ["Column", "Type", "Notes"].map((t, i) => new TableCell({
            borders: allBorderSm,
            shading: { fill: BLUE, type: ShadingType.CLEAR },
            width: { size: [2400,1800,5160][i], type: WidthType.DXA },
            margins: { top: 80, bottom: 80, left: 140, right: 140 },
            children: [new Paragraph({ children: [new TextRun({ text: t, font: "Arial", size: 20, bold: true, color: WHT })]})]
          }))},),
          ...sessionFields.map(([col, type, note], i) => dataRow(
            [{ text: col, bold: true, color: TEAL }, type, note], [2400,1800,5160], i % 2 === 1
          )),
        ],
      }),

      blank(140),
      h2("4.2  Identity flow into the agent"),
      bodyMixed([
        { text: "When a session starts, the broker queries the student record and injects it into the " },
        { text: "claw-code-parity", bold: true },
        { text: " process as its system prompt context, before the student types anything:" },
      ]),
      blank(80),
      code("# prompts/student_system.md (injected at session start)"),
      code("You are EduClaw, an expert teaching assistant embedded in a student's terminal."),
      code(""),
      code("## Student context"),
      code("- Name:       {{display_name}}"),
      code("- Student ID: {{student_id}}"),
      code("- Role:       {{role}}"),
      code("- Classes:    {{class_names | join(', ')}}"),
      code("- Session:    {{session_id}}"),
      code(""),
      code("## Your responsibilities"),
      code("- Guide, never solve. Ask questions before giving answers."),
      code("- Reference course materials in /classes/{{primary_class}}/ when relevant."),
      code("- Remind the student of their work in /home/student/ (their private space)."),
      code("- If the student is stuck, give a hint scoped to the current assignment."),
      code("- All file paths you suggest must be inside the student's environment."),
      code("- You may run shell commands via the Shell MCP tool — they execute in the"),
      code("  student's isolated namespace and cannot affect other students."),
      blank(80),
      bodyMixed([
        { text: "The broker performs template substitution using the student record from SQLite and passes the rendered prompt to " },
        { text: "claw-code-parity", bold: true },
        { text: " via the " },
        { text: "--system-prompt-file", bold: true, color: TEAL },
        { text: " flag (or equivalent plugin hook)." },
      ]),
      blank(200),

      // ─── SECTION 5 ──────────────────────────────────────────────────────────
      sectionBanner("5  Session Lifecycle & Isolation"),
      blank(160),

      h2("5.1  Step-by-step lifecycle"),
      new Table({
        width: { size: 9360, type: WidthType.DXA }, columnWidths: [2200, 2000, 5160],
        rows: [
          new TableRow({ children: ["Phase", "Component", "What happens"].map((t, i) => new TableCell({
            borders: allBorderSm,
            shading: { fill: BLUE, type: ShadingType.CLEAR },
            width: { size: [2200,2000,5160][i], type: WidthType.DXA },
            margins: { top: 80, bottom: 80, left: 140, right: 140 },
            children: [new Paragraph({ children: [new TextRun({ text: t, font: "Arial", size: 20, bold: true, color: WHT })]})]
          }))},),
          ...lifecycleRows.map(([phase, comp, what], i) => dataRow(
            [{ text: phase, bold: true, color: TEAL }, comp, what], [2200,2000,5160], i % 2 === 1
          )),
        ],
      }),

      blank(160),
      h2("5.2  Isolation matrix"),
      new Table({
        width: { size: 9360, type: WidthType.DXA }, columnWidths: [1800, 2400, 5160],
        rows: [
          new TableRow({ children: ["Layer", "Mechanism", "What it prevents"].map((t, i) => new TableCell({
            borders: allBorderSm,
            shading: { fill: TEAL, type: ShadingType.CLEAR },
            width: { size: [1800,2400,5160][i], type: WidthType.DXA },
            margins: { top: 80, bottom: 80, left: 140, right: 140 },
            children: [new Paragraph({ children: [new TextRun({ text: t, font: "Arial", size: 20, bold: true, color: WHT })]})]
          }))},),
          ...isolationRows.map(([layer, mech, what], i) => dataRow(
            [{ text: layer, bold: true, color: BLUE }, mech, what], [1800,2400,5160], i % 2 === 1
          )),
        ],
      }),

      blank(160),
      h2("5.3  Key Rust implementation: namespace spawner"),
      blank(60),
      callout("Why no Docker?", "All isolation is done with raw Linux kernel primitives via the nix crate — no daemon dependency, no image layers, sub-100ms session startup.", AMB_LT, AMB),
      blank(100),
      code("// session/namespace.rs"),
      code("use nix::sched::{unshare, CloneFlags};"),
      code("use nix::mount::{mount, MsFlags};"),
      code("use std::fs;"),
      code(""),
      code("pub fn spawn_student_namespace(student_id: &str, uid_slot: u32) -> Result<Pid> {"),
      code("    // 1. Fork — child will become the namespace init"),
      code("    match unsafe { nix::unistd::fork() }? {"),
      code("        ForkResult::Parent { child } => {"),
      code("            // write uid_map: uid_slot inside → unprivileged host uid"),
      code("            fs::write(format!(\"/proc/{child}/uid_map\"),"),
      code("                      format!(\"0 {uid_slot} 1\\n\"))?;"),
      code("            fs::write(format!(\"/proc/{child}/setgroups\"), \"deny\")?;"),
      code("            fs::write(format!(\"/proc/{child}/gid_map\"),"),
      code("                      format!(\"0 {uid_slot} 1\\n\"))?;"),
      code("            Ok(child)"),
      code("        }"),
      code("        ForkResult::Child => {"),
      code("            // 2. Create all namespaces"),
      code("            unshare(CloneFlags::CLONE_NEWPID  |"),
      code("                    CloneFlags::CLONE_NEWNET  |"),
      code("                    CloneFlags::CLONE_NEWIPC  |"),
      code("                    CloneFlags::CLONE_NEWUTS  |"),
      code("                    CloneFlags::CLONE_NEWUSER)?;"),
      code(""),
      code("            // 3. Mount OverlayFS"),
      code("            let upper = format!(\"/data/students/{student_id}/upper\");"),
      code("            let work  = format!(\"/data/students/{student_id}/work\");"),
      code("            let merged = format!(\"/tmp/ns-{student_id}/merged\");"),
      code("            fs::create_dir_all(&upper)?;"),
      code("            fs::create_dir_all(&work)?;"),
      code("            fs::create_dir_all(&merged)?;"),
      code("            mount("),
      code("                Some(\"overlay\"),"),
      code("                merged.as_str(),"),
      code("                Some(\"overlay\"),"),
      code("                MsFlags::empty(),"),
      code("                Some(format!("),
      code("                    \"lowerdir=/data/classes,upperdir={upper},workdir={work}\""),
      code("                ).as_str()),"),
      code("            )?;"),
      code(""),
      code("            // 4. exec bash — PTY is already open on parent side"),
      code("            execvp(\"/bin/bash\", &[\"/bin/bash\"])?;"),
      code("            unreachable!()"),
      code("        }"),
      code("    }"),
      code("}"),
      blank(160),

      h2("5.4  Idle watchdog"),
      code("// session/watchdog.rs"),
      code("pub async fn watchdog(session_id: Uuid, broker: Arc<SessionBroker>) {"),
      code("    let timeout   = Duration::from_secs(15 * 60); // 15 min idle"),
      code("    let warn_at   = timeout - Duration::from_secs(60);"),
      code("    let mut warned = false;"),
      code("    loop {"),
      code("        sleep(Duration::from_secs(30)).await;"),
      code("        let elapsed = broker.last_input(session_id).elapsed();"),
      code("        if elapsed >= timeout {"),
      code("            broker.terminate(session_id, TermReason::IdleTimeout).await;"),
      code("            return;"),
      code("        }"),
      code("        if elapsed >= warn_at && !warned {"),
      code("            broker.send_to_pty(session_id,"),
      code("                b\"\\r\\n[EduClaw] Session idle — saving your work. \","),
      code("            ).await;"),
      code("            broker.send_to_pty(session_id,"),
      code("                b\"You have 60 seconds before disconnect.\\r\\n\","),
      code("            ).await;"),
      code("            warned = true;"),
      code("        }"),
      code("    }"),
      code("}"),
      blank(200),

      // ─── SECTION 6 ──────────────────────────────────────────────────────────
      sectionBanner("6  MCP Servers & Plugin Strategy"),
      blank(160),

      h2("6.1  Shared MCP server process"),
      bodyMixed([
        { text: "All MCP servers run as a " },
        { text: "single shared host process ", bold: true },
        { text: "(educlaw-mcp). They are not isolated — they serve all student sessions simultaneously. Isolation is enforced by " },
        { text: "scoping every MCP request using the student_id extracted from the session JWT.", bold: true },
      ]),
      blank(80),
      callout("Key insight:", "The claw-code-parity instance runs inside the namespace, but its MCP client calls cross back to the host via a Unix socket or loopback TCP. The MCP server validates student_id on every request — no student can reach another student's files, even through MCP.", TEAL_LT, TEAL),
      blank(120),

      new Table({
        width: { size: 9360, type: WidthType.DXA }, columnWidths: [2400, 2000, 4960],
        rows: [
          new TableRow({ children: ["MCP Server", "Scope", "Responsibility"].map((t, i) => new TableCell({
            borders: allBorderSm,
            shading: { fill: BLUE, type: ShadingType.CLEAR },
            width: { size: [2400,2000,4960][i], type: WidthType.DXA },
            margins: { top: 80, bottom: 80, left: 140, right: 140 },
            children: [new Paragraph({ children: [new TextRun({ text: t, font: "Arial", size: 20, bold: true, color: WHT })]})]
          }))},),
          ...mcpRows.map(([name, scope, resp], i) => dataRow(
            [{ text: name, bold: true, color: TEAL }, scope, resp], [2400,2000,4960], i % 2 === 1
          )),
        ],
      }),

      blank(140),
      h2("6.2  MCP request scoping (enforced in Rust)"),
      code("// educlaw-mcp/src/student_files.rs"),
      code("async fn read_file(req: McpRequest, ctx: &SessionCtx) -> McpResult {"),
      code("    let path = req.params.get(\"path\")?;"),
      code("    // ENFORCE: path must be inside this student's upper dir"),
      code("    let base = PathBuf::from(format!(\"/data/students/{}/upper\", ctx.student_id));"),
      code("    let resolved = base.join(path).canonicalize()?;"),
      code("    if !resolved.starts_with(&base) {"),
      code("        return Err(McpError::Forbidden(\"path outside student directory\"));"),
      code("    }"),
      code("    Ok(fs::read_to_string(resolved)?.into())"),
      code("}"),
      blank(140),

      h2("6.3  claw-code-parity plugin: EduClaw hook"),
      bodyMixed([
        { text: "A single plugin installed into claw-code-parity " },
        { text: "before", bold: true },
        { text: " packaging handles:" },
      ]),
      bullet("Injecting the student system prompt at startup (reads EDUCLAW_STUDENT_JSON env var set by broker)"),
      bullet("Connecting to the educlaw-mcp Unix socket (set via EDUCLAW_MCP_SOCKET env var)"),
      bullet("Disabling any upstream plugins that allow network access or arbitrary code execution outside the namespace"),
      bullet("Intercepting the /help command to show EduClaw-specific guidance"),
      blank(200),

      // ─── SECTION 7 ──────────────────────────────────────────────────────────
      sectionBanner("7  Python Environment Inside the Isolation"),
      blank(160),

      h2("7.1  How Python works inside the namespace"),
      bodyMixed([
        { text: "Python runs natively inside the student's namespace — it is just a binary on the merged filesystem view. Because the " },
        { text: "lower layer (/data/classes/)", bold: true },
        { text: " includes the host's " },
        { text: "/usr/bin/python3", bold: true, color: TEAL },
        { text: " and the entire site-packages tree, every student gets full Python immediately with no installation step. Student-installed packages (pip install --user) land in their " },
        { text: "upper layer", bold: true },
        { text: " and never bleed across sessions." },
      ]),
      blank(80),
      callout("Key point:", "Pre-install all shared libraries into the lower layer image once. Every new student session inherits them instantly via OverlayFS — zero per-student setup cost.", TEAL_LT, TEAL),
      blank(120),

      h2("7.2  Pre-installed library set (lower layer)"),
      body("The following libraries are baked into /data/classes/usr/local/lib/python3.x/site-packages/ as part of the shared lower layer. Students see them immediately; the agent can use them in any code it writes or suggests."),
      blank(80),

      new Table({
        width: { size: 9360, type: WidthType.DXA }, columnWidths: [2200, 1800, 5360],
        rows: [
          new TableRow({ children: ["Library", "Category", "Educational use"].map((t, i) => new TableCell({
            borders: allBorderSm,
            shading: { fill: BLUE, type: ShadingType.CLEAR },
            width: { size: [2200,1800,5360][i], type: WidthType.DXA },
            margins: { top: 80, bottom: 80, left: 140, right: 140 },
            children: [new Paragraph({ children: [new TextRun({ text: t, font: "Arial", size: 20, bold: true, color: WHT })]})]
          }))},),
          ...pythonLibRows.map(([lib, cat, use_], i) => dataRow(
            [{ text: lib, bold: true, color: TEAL }, cat, use_], [2200,1800,5360], i % 2 === 1
          )),
        ],
      }),

      blank(140),
      h2("7.3  PDF & document extraction pipeline"),
      bodyMixed([
        { text: "Teachers drop raw course materials (PDFs, DOCX, HTML) into /data/classes/{class_id}/raw/. A conversion pipeline — triggered manually by the teacher or automatically on file drop — extracts them into clean " },
        { text: ".md", bold: true, color: TEAL },
        { text: " or " },
        { text: ".txt", bold: true, color: TEAL },
        { text: " files alongside the originals. The agent can then read these lightweight files directly into its context window without needing to parse binary formats." },
      ]),
      blank(80),
      code("# Conversion pipeline (runs on host, not inside namespace)"),
      code("# educlaw-mcp/src/converter.py  (called from Rust via std::process::Command)"),
      code(""),
      code("import fitz          # pymupdf — fastest PDF extractor"),
      code("import markdownify"),
      code("from docx import Document"),
      code("from pathlib import Path"),
      code(""),
      code("def pdf_to_md(src: Path, dst: Path):"),
      code("    doc = fitz.open(src)"),
      code("    pages = []"),
      code("    for page in doc:"),
      code("        # Extract text preserving reading order"),
      code("        text = page.get_text('markdown')  # fitz native MD output"),
      code("        # Extract images — save as {stem}_fig_{n}.png"),
      code("        for i, img in enumerate(page.get_images(full=True)):"),
      code("            xref = img[0]"),
      code("            pix  = fitz.Pixmap(doc, xref)"),
      code("            pix.save(dst.parent / f'{dst.stem}_fig_{i}.png')"),
      code("            text += f'\\n![]({dst.stem}_fig_{i}.png)\\n'"),
      code("        pages.append(text)"),
      code("    dst.write_text('\\n\\n---\\n\\n'.join(pages))"),
      code(""),
      code("def docx_to_md(src: Path, dst: Path):"),
      code("    doc = Document(src)"),
      code("    lines = [para.text for para in doc.paragraphs]"),
      code("    dst.write_text('\\n'.join(lines))"),
      blank(80),
      bodyMixed([
        { text: "After conversion the directory looks like:" },
      ]),
      code("/data/classes/math101/"),
      code("├── raw/"),
      code("│   ├── chapter3.pdf          ← original (served to browser as-is)"),
      code("│   └── chapter3.docx"),
      code("├── chapter3.md               ← auto-generated, agent reads this"),
      code("├── chapter3_fig_0.png        ← extracted figures"),
      code("├── chapter3_fig_1.png"),
      code("└── chapter3_docx.md          ← from DOCX"),
      blank(80),
      bodyMixed([
        { text: "When a student asks about chapter 3, the agent reads " },
        { text: "chapter3.md", bold: true, color: TEAL },
        { text: " for text context and sends a " },
        { text: "show_image", bold: true, color: TEAL },
        { text: " WS message to the browser for any figures — all without the student typing a command." },
      ]),
      blank(140),

      h2("7.4  Agent + Python example flow (math class)"),
      code("# Student types in terminal:"),
      code("# 'can you show me how to solve a quadratic equation?'"),
      code(""),
      code("# Agent (claw-code-parity) generates and runs via Shell MCP:"),
      code("import sympy as sp"),
      code(""),
      code("x = sp.Symbol('x')"),
      code("eq = x**2 - 5*x + 6"),
      code("roots = sp.solve(eq, x)"),
      code("print(f'Roots of x² - 5x + 6 = 0 are: {roots}')"),
      code("# → Roots of x² - 5x + 6 = 0 are: [2, 3]"),
      code(""),
      code("# Agent then plots it:"),
      code("import numpy as np"),
      code("import matplotlib"),
      code("matplotlib.use('Agg')   # no display needed — saves to file"),
      code("import matplotlib.pyplot as plt"),
      code(""),
      code("xs = np.linspace(0, 5, 300)"),
      code("plt.plot(xs, xs**2 - 5*xs + 6)"),
      code("plt.axhline(0, color='k', linewidth=0.5)"),
      code("plt.title('x² - 5x + 6')"),
      code("plt.savefig('/home/student/quadratic.png', dpi=120)"),
      code(""),
      code("# Agent then sends WS message:"),
      code("# { type: 'show_image', path: '/home/student/quadratic.png', scope: 'student' }"),
      code("# → browser opens the PNG inline in the file panel"),
      blank(200),

      // ─── SECTION 8 ──────────────────────────────────────────────────────────
      sectionBanner("8  File Browser & Agent–Browser Communication"),
      blank(160),

      h2("8.1  Browser layout"),
      bodyMixed([
        { text: "The browser UI is split into two panels. The " },
        { text: "left panel", bold: true },
        { text: " is the xterm.js terminal (the full claw-code-parity TUI). The " },
        { text: "right panel", bold: true },
        { text: " is the file browser — a collapsible tree showing both the student's files and the class materials, with a content viewer below it. The agent can drive both panels via the same WebSocket connection." },
      ]),
      blank(80),
      new Table({
        width: { size: 9360, type: WidthType.DXA }, columnWidths: [4680, 4680],
        rows: [new TableRow({ children: [
          new TableCell({
            borders: allBorderSm,
            shading: { fill: BLUE, type: ShadingType.CLEAR },
            width: { size: 4680, type: WidthType.DXA },
            margins: { top: 80, bottom: 80, left: 140, right: 140 },
            children: [new Paragraph({ children: [new TextRun({ text: "Left panel — Terminal", font: "Arial", size: 20, bold: true, color: WHT })]})]
          }),
          new TableCell({
            borders: allBorderSm,
            shading: { fill: TEAL, type: ShadingType.CLEAR },
            width: { size: 4680, type: WidthType.DXA },
            margins: { top: 80, bottom: 80, left: 140, right: 140 },
            children: [new Paragraph({ children: [new TextRun({ text: "Right panel — File browser + viewer", font: "Arial", size: 20, bold: true, color: WHT })]})]
          }),
        ]})],
      }),
      new Table({
        width: { size: 9360, type: WidthType.DXA }, columnWidths: [4680, 4680],
        rows: [new TableRow({ children: [
          new TableCell({
            borders: allBorderSm,
            shading: { fill: "FFFFFF", type: ShadingType.CLEAR },
            width: { size: 4680, type: WidthType.DXA },
            margins: { top: 100, bottom: 100, left: 140, right: 140 },
            children: [
              new Paragraph({ spacing: { before: 40, after: 40 }, children: [new TextRun({ text: "xterm.js full TUI", font: "Courier New", size: 18, color: BLK })]}),
              new Paragraph({ spacing: { before: 40, after: 40 }, children: [new TextRun({ text: "claw-code-parity agent", font: "Courier New", size: 18, color: BLK })]}),
              new Paragraph({ spacing: { before: 40, after: 40 }, children: [new TextRun({ text: "Python REPL / shell", font: "Courier New", size: 18, color: BLK })]}),
              new Paragraph({ spacing: { before: 40, after: 40 }, children: [new TextRun({ text: "All student input", font: "Courier New", size: 18, color: BLK })]}),
            ]
          }),
          new TableCell({
            borders: allBorderSm,
            shading: { fill: "FFFFFF", type: ShadingType.CLEAR },
            width: { size: 4680, type: WidthType.DXA },
            margins: { top: 100, bottom: 100, left: 140, right: 140 },
            children: [
              new Paragraph({ spacing: { before: 40, after: 40 }, children: [new TextRun({ text: "▾ My files (/home/student/)", font: "Courier New", size: 18, color: BLK })]}),
              new Paragraph({ spacing: { before: 40, after: 40 }, children: [new TextRun({ text: "▾ Class: math101/", font: "Courier New", size: 18, color: BLK })]}),
              new Paragraph({ spacing: { before: 40, after: 40 }, children: [new TextRun({ text: "─────────────────", font: "Courier New", size: 18, color: BLK })]}),
              new Paragraph({ spacing: { before: 40, after: 40 }, children: [new TextRun({ text: "[image / PDF / code viewer]", font: "Courier New", size: 18, color: BLK })]}),
            ]
          }),
        ]})],
      }),

      blank(140),
      h2("8.2  File browser REST API"),
      new Table({
        width: { size: 9360, type: WidthType.DXA }, columnWidths: [3000, 1600, 4760],
        rows: [
          new TableRow({ children: ["Endpoint", "Scope", "Description"].map((t, i) => new TableCell({
            borders: allBorderSm,
            shading: { fill: BLUE, type: ShadingType.CLEAR },
            width: { size: [3000,1600,4760][i], type: WidthType.DXA },
            margins: { top: 80, bottom: 80, left: 140, right: 140 },
            children: [new Paragraph({ children: [new TextRun({ text: t, font: "Arial", size: 20, bold: true, color: WHT })]})]
          }))},),
          ...fileBrowserApiRows.map(([ep, scope, desc], i) => dataRow(
            [{ text: ep, bold: true, color: TEAL }, scope, desc], [3000,1600,4760], i % 2 === 1
          )),
        ],
      }),
      blank(80),
      callout("Security:", "All /api/files/{sid}/... endpoints validate that the JWT student_id matches the {sid} path parameter. A student can never request another student's file tree. Class files are read-only for all non-admin roles.", BLUE_LT, BLUE),

      blank(140),
      h2("8.3  Agent–browser signalling protocol (WsEnvelope)"),
      bodyMixed([
        { text: "The WebSocket carries two kinds of traffic on the same connection. Raw binary frames are PTY bytes — xterm.js writes them directly. Text frames are " },
        { text: "JSON envelopes", bold: true },
        { text: " that the browser dispatches to the UI layer. The agent writes envelopes to stdout using a special escape prefix that the PTY bridge intercepts and re-frames as JSON text before forwarding to the browser — so the terminal itself never sees the control messages." },
      ]),
      blank(80),
      code("// ws.rs — intercept agent control messages in PTY output stream"),
      code("const CTRL_PREFIX: &[u8] = b\"\\x1b]EduClaw;\";"),
      code("const CTRL_SUFFIX: &[u8] = b\"\\x07\";"),
      code(""),
      code("fn split_pty_output(raw: &[u8]) -> (Vec<u8>, Option<serde_json::Value>) {"),
      code("    if let Some(start) = find_subsequence(raw, CTRL_PREFIX) {"),
      code("        if let Some(end) = find_subsequence(&raw[start..], CTRL_SUFFIX) {"),
      code("            let json_bytes = &raw[start + CTRL_PREFIX.len()..start + end];"),
      code("            let terminal_bytes = [&raw[..start], &raw[start+end+CTRL_SUFFIX.len()..]].concat();"),
      code("            let envelope = serde_json::from_slice(json_bytes).ok();"),
      code("            return (terminal_bytes, envelope);"),
      code("        }"),
      code("    }"),
      code("    (raw.to_vec(), None)"),
      code("}"),
      blank(80),
      bodyMixed([
        { text: "The agent (claw-code-parity plugin) emits these envelopes using a helper that writes the OSC escape sequence to stdout — invisible to xterm.js, intercepted by the PTY bridge, forwarded as a JSON text frame to the browser." },
      ]),
      blank(80),

      new Table({
        width: { size: 9360, type: WidthType.DXA }, columnWidths: [2200, 2000, 5160],
        rows: [
          new TableRow({ children: ["Envelope type", "Direction", "Effect in browser"].map((t, i) => new TableCell({
            borders: allBorderSm,
            shading: { fill: TEAL, type: ShadingType.CLEAR },
            width: { size: [2200,2000,5160][i], type: WidthType.DXA },
            margins: { top: 80, bottom: 80, left: 140, right: 140 },
            children: [new Paragraph({ children: [new TextRun({ text: t, font: "Arial", size: 20, bold: true, color: WHT })]})]
          }))},),
          ...wsEnvelopeRows.map(([type_, dir, effect], i) => dataRow(
            [{ text: type_, bold: true, color: TEAL }, dir, effect], [2200,2000,5160], i % 2 === 1
          )),
        ],
      }),

      blank(140),
      h2("8.4  Browser envelope dispatcher (JS)"),
      code("// educlaw-web/terminal.js"),
      code("ws.onmessage = (e) => {"),
      code("  if (typeof e.data === 'string') {"),
      code("    // JSON control envelope from agent"),
      code("    const env = JSON.parse(e.data);"),
      code("    switch (env.type) {"),
      code("      case 'open_file':"),
      code("        openFilePanel(env.path, env.scope);     // opens in right panel"),
      code("        break;"),
      code("      case 'show_image':"),
      code("        showImageInPanel(env.path, env.scope);  // renders <img> inline"),
      code("        break;"),
      code("      case 'file_tree':"),
      code("        refreshFileTree(env.scope);             // re-fetches /api/files tree"),
      code("        break;"),
      code("      case 'notify':"),
      code("        showToast(env.message);                 // top-right toast notification"),
      code("        break;"),
      code("    }"),
      code("  } else {"),
      code("    // Binary frame — raw PTY bytes"),
      code("    term.write(new Uint8Array(e.data));"),
      code("  }"),
      code("};"),
      blank(80),

      h2("8.5  End-to-end example: student asks to see chapter 3"),
      new Table({
        width: { size: 9360, type: WidthType.DXA }, columnWidths: [9360],
        rows: [
          ...[
            "1  Student types: 'show me the notes for chapter 3'",
            "2  claw-code-parity agent reads chapter3.md via ClassFiles MCP (scoped to student's enrolled class)",
            "3  Agent summarises key points in the terminal (PTY stream → xterm.js)",
            "4  Agent emits OSC envelope: { type: 'open_file', path: 'math101/chapter3.md', scope: 'class' }",
            "5  PTY bridge strips envelope from byte stream, forwards as JSON text frame to browser",
            "6  Browser dispatcher calls openFilePanel() → fetches /api/files/class/math101/read?path=chapter3.md",
            "7  Right panel renders the Markdown with syntax highlighting",
            "8  Agent also emits: { type: 'show_image', path: 'math101/chapter3_fig_0.png', scope: 'class' }",
            "9  Browser fetches image via /api/files/class/math101/read?path=chapter3_fig_0.png",
            "10 Image renders inline in the viewer panel — student sees diagram without leaving the terminal",
          ].map((t, i) => new TableRow({ children: [new TableCell({
            borders: allBorderSm,
            shading: { fill: i % 2 === 0 ? BLUE_LT : "FFFFFF", type: ShadingType.CLEAR },
            margins: { top: 100, bottom: 100, left: 160, right: 160 },
            children: [new Paragraph({ children: [new TextRun({ text: t, font: "Arial", size: 20, color: BLK, bold: t.startsWith("1 ") || t.startsWith("5 ") || t.startsWith("10") ? false : false })]})]
          })]})
        )],
      }),

      blank(200),

      // ─── SECTION 9 ──────────────────────────────────────────────────────────
      sectionBanner("9  WebSocket PTY Bridge (Updated)"),
      blank(160),

      h2("9.1  Axum WebSocket handler with envelope splitting"),
      code("// ws.rs"),
      code("async fn ws_handler("),
      code("    ws: WebSocketUpgrade,"),
      code("    Path(session_id): Path<Uuid>,"),
      code("    State(broker): State<Arc<SessionBroker>>,"),
      code("    claims: JwtClaims,"),
      code(") -> impl IntoResponse {"),
      code("    ws.on_upgrade(move |socket| async move {"),
      code("        let (mut sink, mut stream) = socket.split();"),
      code("        let pty_tx = broker.pty_tx(session_id).unwrap();"),
      code("        let mut pty_rx = broker.pty_rx(session_id).unwrap();"),
      code(""),
      code("        // PTY → browser (with envelope interception)"),
      code("        tokio::spawn(async move {"),
      code("            let mut buf = [0u8; 4096];"),
      code("            while let Some(bytes) = pty_rx.recv().await {"),
      code("                let (pty_bytes, envelope) = split_pty_output(&bytes);"),
      code("                // Forward raw terminal bytes"),
      code("                if !pty_bytes.is_empty() {"),
      code("                    sink.send(Message::Binary(pty_bytes)).await.ok();"),
      code("                }"),
      code("                // Forward control envelope as JSON text frame"),
      code("                if let Some(env) = envelope {"),
      code("                    sink.send(Message::Text(env.to_string())).await.ok();"),
      code("                }"),
      code("            }"),
      code("        });"),
      code(""),
      code("        // Browser → PTY or server commands"),
      code("        while let Some(Ok(msg)) = stream.next().await {"),
      code("            match msg {"),
      code("                Message::Binary(data) => {"),
      code("                    broker.update_last_input(session_id);"),
      code("                    pty_tx.send(data).await.ok();"),
      code("                }"),
      code("                Message::Text(json) => {"),
      code("                    handle_browser_command(session_id, &json, &broker).await;"),
      code("                }"),
      code("                _ => {}"),
      code("            }"),
      code("        }"),
      code("    })"),
      code("}"),
      blank(200),

      // ─── SECTION 10 ─────────────────────────────────────────────────────────
      sectionBanner("10  Rust Crate Dependency Plan"),
      blank(160),

      new Table({
        width: { size: 9360, type: WidthType.DXA }, columnWidths: crateWidths,
        rows: [
          new TableRow({ children: ["Crate", "Purpose", "Role in EduClaw"].map((t, i) => new TableCell({
            borders: allBorderSm,
            shading: { fill: BLUE, type: ShadingType.CLEAR },
            width: { size: crateWidths[i], type: WidthType.DXA },
            margins: { top: 80, bottom: 80, left: 140, right: 140 },
            children: [new Paragraph({ children: [new TextRun({ text: t, font: "Arial", size: 20, bold: true, color: WHT })]})]
          }))},),
          ...crateRows.map(([crate, purpose, role], i) => dataRow(
            [{ text: crate, bold: true, color: TEAL }, purpose, role], crateWidths, i % 2 === 1
          )),
        ],
      }),
      blank(200),

      // ─── SECTION 11 ─────────────────────────────────────────────────────────
      sectionBanner("11  Teacher & Admin Interface"),
      blank(160),

      h2("11.1  Admin REST API (Axum)"),
      bullet("POST /api/admin/students — create student account (name, username, password, class_ids)"),
      bullet("GET  /api/admin/students — list all students with last_seen and active session flag"),
      bullet("PUT  /api/admin/students/{id} — update class enrollments, reset password"),
      bullet("GET  /api/admin/sessions — live session list with idle time, namespace PID, uid slot"),
      bullet("DELETE /api/admin/sessions/{id} — force-terminate a session"),
      bullet("POST /api/admin/classes — create class, set course content path in /data/classes/"),
      bullet("POST /api/admin/classes/{id}/convert — trigger PDF/DOCX→MD conversion for all raw materials"),
      blank(80),

      h2("11.2  Teacher broadcast"),
      bodyMixed([
        { text: "Teachers can push a message to one or all active sessions via the TeacherBroadcast MCP server. The message appears inline in the student's terminal as a highlighted banner — same PTY, same stream — so no new UI is needed. Teachers can also send a " },
        { text: "show_image", bold: true, color: TEAL },
        { text: " or " },
        { text: "open_file", bold: true, color: TEAL },
        { text: " envelope to push a diagram or document to all student panels simultaneously." },
      ]),
      blank(80),
      callout("Privacy note:", "Teachers see session metadata (idle time, session ID, class) but cannot read the student's terminal output or files directly. File review happens through the StudentFiles MCP with explicit audit logging.", BLUE_LT, BLUE),
      blank(200),

      // ─── SECTION 12 ─────────────────────────────────────────────────────────
      sectionBanner("12  Deployment"),
      blank(160),

      h2("12.1  Requirements"),
      bullet("Linux kernel 5.10+ (user namespace + OverlayFS + cgroups v2 support)"),
      bullet("Rust 1.78+ (async, nix, sqlx)"),
      bullet("Python 3.10+ with pymupdf, markdownify, python-docx installed system-wide (for conversion pipeline)"),
      bullet("No Docker required on the host — raw syscalls only"),
      bullet("educlaw-server must run as a process with CAP_SYS_ADMIN for mount syscalls (or use newuidmap/newgidmap helpers for fully rootless)"),
      blank(80),

      h2("12.2  Directory layout on host"),
      code("/srv/educlaw/"),
      code("├── educlaw-server          ← compiled binary"),
      code("├── educlaw-mcp             ← compiled binary"),
      code("├── educlaw.db              ← SQLite student registry"),
      code("├── educlaw-web/            ← static browser assets"),
      code("├── prompts/"),
      code("│   └── student_system.md"),
      code("├── data/"),
      code("│   ├── classes/            ← shared RO course content"),
      code("│   │   └── math101/"),
      code("│   │       ├── raw/        ← teacher uploads (PDF, DOCX)"),
      code("│   │       ├── chapter3.md ← auto-generated MD"),
      code("│   │       └── chapter3_fig_0.png"),
      code("│   └── students/           ← per-student upper dirs (persisted)"),
      code("│       └── {student_id}/"),
      code("│           ├── upper/      ← student writable layer"),
      code("│           └── work/       ← OverlayFS work dir"),
      code("└── config.toml"),
      blank(80),

      h2("12.3  config.toml"),
      code("[server]"),
      code("bind            = \"0.0.0.0:8080\""),
      code("jwt_secret      = \"<random 256-bit hex>\""),
      code(""),
      code("[session]"),
      code("idle_timeout_s  = 900   # 15 minutes"),
      code("warn_before_s   = 60"),
      code("max_sessions    = 200"),
      code("uid_range_start = 100000"),
      code("uid_range_end   = 200000"),
      code(""),
      code("[paths]"),
      code("classes_dir     = \"/srv/educlaw/data/classes\""),
      code("students_dir    = \"/srv/educlaw/data/students\""),
      code("prompt_file     = \"/srv/educlaw/prompts/student_system.md\""),
      code("mcp_socket      = \"/run/educlaw-mcp.sock\""),
      code(""),
      code("[file_browser]"),
      code("max_file_size_mb     = 50"),
      code("image_extensions     = [\"png\", \"jpg\", \"jpeg\", \"gif\", \"svg\", \"webp\"]"),
      code("text_extensions      = [\"md\", \"txt\", \"py\", \"rs\", \"js\", \"json\", \"csv\"]"),
      code("pdf_extraction_tool  = \"pymupdf\"   # or 'pdfminer'"),
      blank(200),

      // ─── SECTION 13 ─────────────────────────────────────────────────────────
      sectionBanner("13  Build Phases (Updated)"),
      blank(160),

      h2("Phase 1 — Foundation  (weeks 1–3)"),
      numbered("Fork claw-code-parity; add EduClaw plugin skeleton, OSC envelope emitter, and --system-prompt-file support"),
      numbered("Set up educlaw-server Rust project: Axum, sqlx, JWT auth, student CRUD API"),
      numbered("Implement namespace spawner (namespace.rs) with user namespace + uid mapping"),
      numbered("Test OverlayFS mount with pre-installed Python libs in lower layer on a dev Linux VM"),
      numbered("Basic PTY open + WebSocket bridge: binary frames for terminal, text frames for envelopes"),
      blank(80),

      h2("Phase 2 — Session management  (weeks 4–6)"),
      numbered("Session broker: spawn, register, lookup by student_id, fan-out PTY to multiple WS"),
      numbered("Idle watchdog Tokio task with configurable timeout and in-terminal warning"),
      numbered("Graceful teardown: SIGTERM → umount → uid slot release → DB update"),
      numbered("Resize event handling: TIOCSWINSZ from WS JSON message"),
      numbered("xterm.js browser UI: login page → session open → split-panel layout (terminal + file browser)"),
      blank(80),

      h2("Phase 3 — MCP & AI integration  (weeks 7–9)"),
      numbered("educlaw-mcp process: StudentContext, ClassFiles, StudentFiles, EvalEngine servers"),
      numbered("student_id enforcement on every MCP request (path prefix, JWT claim match)"),
      numbered("Shell-in-namespace MCP: executes commands inside student namespace"),
      numbered("Student system prompt template injection wired into claw-code-parity plugin"),
      numbered("Python lib lower layer image: numpy, sympy, matplotlib, scipy, pandas, rich"),
      blank(80),

      h2("Phase 4 — File browser & agent→browser signals  (weeks 10–11)"),
      numbered("File browser REST API: /api/files tree, read, preview endpoints"),
      numbered("WsEnvelope dispatcher in browser JS: open_file, show_image, file_tree, notify"),
      numbered("PTY bridge OSC envelope interceptor in Rust (split_pty_output)"),
      numbered("PDF→MD conversion pipeline: pymupdf + markdownify, triggered via admin API"),
      numbered("Image preview: matplotlib PNG output auto-shown in file panel via show_image envelope"),
      blank(80),

      h2("Phase 5 — Admin & hardening  (weeks 12–14)"),
      numbered("Admin REST API: class management, enrollment, force-terminate, live session view, PDF conversion trigger"),
      numbered("TeacherBroadcast MCP + teacher ability to push open_file/show_image to all sessions"),
      numbered("seccomp BPF allowlist via seccompiler crate"),
      numbered("cgroups v2 resource caps per session (CPU %, memory limit, I/O weight)"),
      numbered("Structured audit logging (tracing crate) for every session and file access event"),
      numbered("Load testing: 100 concurrent sessions, Python workloads, file serving"),
      blank(160),

      // Final note
      new Table({
        width: { size: 9360, type: WidthType.DXA }, columnWidths: [9360],
        rows: [new TableRow({ children: [new TableCell({
          borders: { top: borderTh, bottom: borderTh, left: { style: BorderStyle.SINGLE, size: 20, color: TEAL }, right: border0 },
          shading: { fill: TEAL_LT, type: ShadingType.CLEAR },
          margins: { top: 200, bottom: 200, left: 240, right: 240 },
          children: [
            new Paragraph({ spacing: { before: 0, after: 80 }, children: [
              new TextRun({ text: "Everything that makes claw-code-parity great is preserved. ", font: "Arial", size: 22, bold: true, color: TEAL }),
              new TextRun({ text: "Students get the full power of an AI-assisted coding environment — shell, Python, file editing, plugin ecosystem, and multi-model AI. The agent sees class materials, knows who the student is, can run Python to explain math live, and can push any file, image, or document directly into the browser panel — all without the student typing a single command.", font: "Arial", size: 22, color: BLK }),
            ]}),
          ]
        })]})],
      }),

    ]
  }]
});

Packer.toBuffer(doc).then(buf => {
  fs.writeFileSync('/mnt/user-data/outputs/EduClaw_Architecture_Plan.docx', buf);
  console.log('Done');
});
