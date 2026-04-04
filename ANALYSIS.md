# EduClaw — Feasibility Analysis vs claw-code-parity

## What we read

| Source | Content |
|---|---|
| `gen_educlaw.js` | 13-section architecture plan: sessions, isolation, MCP, Python env, file browser, deployment |
| `claw-code-parity/README.md` | Project backstory — clean-room Python rewrite + Rust port in progress |
| `claw-code-parity/PARITY.md` | 9-lane parity checkpoint: 48 599 Rust LOC, 9 crates, 40 exposed tool specs |
| `claw-code-parity/ROADMAP.md` | "Clawable harness" roadmap — automation-first, machine-readable state |
| `rust/crates/` | 9 Rust crates: `runtime`, `tools`, `plugins`, `commands`, `api`, `rusty-claude-cli`, `mock-anthropic-service`, `compat-harness`, `telemetry` |

---

## What claw-code-parity actually is (right now)

This is **not** a drop-in multi-model CLI you can `pip install` or `cargo install` and immediately use as a pedagogical shell. Here is the honest state:

### What is real and working
- **Rust CLI binary** (`claw`) built with Tokio + serde_json — compiles and runs
- **40 tool specs** in `rust/crates/tools/src/lib.rs` (`bash`, `read_file`, `write_file`, `edit_file`, `glob_search`, `grep_search`, `WebFetch`, `TodoWrite`, `Agent`, `NotebookEdit`, `REPL`, `LSP`, `MCP`, `Task*`, `Team*`, `Cron*`, etc.)
- **Plugin system** — full `PluginManager` with install/enable/disable/uninstall, hooks (`PreToolUse`, `PostToolUse`), lifecycle commands, manifest validation
- **MCP client bridge** — `mcp_tool_bridge.rs` (406 LOC) registry-backed, supports stdio/SSE/HTTP/WS transports
- **Permission enforcer** — workspace boundary checks, bash read-only heuristics, JWT-style permission policies
- **File ops** — binary detection, size limits, symlink escape guards, path traversal prevention
- **Sandbox** — `sandbox.rs` (385 LOC) probes real `unshare` capability
- **Session/worker state machine** — `worker_boot.rs`, `session_control.rs` with lifecycle states
- **Mock parity harness** — 10 scripted scenarios, deterministic Anthropic-compatible mock service
- **Python `src/` tree** — 67 modules porting the TypeScript harness (commands, tools, hooks, plugins, MCP, session, permissions, etc.)

### What is still shallow or pending
- `AskUserQuestion` returns pending stub, no real interactive UI wiring
- `RemoteTrigger` is stub
- End-to-end MCP runtime lifecycle (beyond registry bridge) is incomplete
- Session compaction, token counting, cost tracking not parity-matched
- CI is not green on every commit
- No PTY bridge, no WebSocket server, no HTTP API — the CLI is a terminal agent, not a web-hosted one

---

## EduClaw plan vs repo reality — gap table

| EduClaw plan requires | In claw-code-parity? | Gap / action needed |
|---|---|---|
| `claw` CLI runnable inside PTY | Yes — `claw` binary exists | None for the CLI itself |
| `--system-prompt-file` or env-var prompt injection | Not exposed yet | Add a flag / read `EDUCLAW_STUDENT_JSON` env var — small plugin hook or CLI arg |
| Plugin hook at startup (inject student context) | Plugin lifecycle `Init` hooks exist | Write the EduClaw plugin (`plugin.json` + init script) |
| MCP client connects to Unix socket / loopback | MCP bridge supports stdio + SSE + HTTP + WS | Pick `stdio` or `http` transport for `educlaw-mcp` — already supported |
| Permission scoping per student | `PermissionEnforcer` + workspace boundary checks exist | Map student upper dir as workspace root when spawning session |
| Sandbox / namespace isolation | `sandbox.rs` probes `unshare` but does **not** call `nix::sched::unshare` itself | EduClaw plan's `namespace.rs` must be written in `educlaw-server` (separate Rust crate), not inside claw-code-parity |
| OverlayFS mount | Not in claw-code-parity at all | Lives in `educlaw-server/src/session/namespace.rs` — independent of this repo |
| PTY open + WebSocket bridge | Not in claw-code-parity | Lives in `educlaw-server/src/ws.rs` + `pty.rs` — must be written fresh |
| Axum HTTP API + JWT auth | Not in claw-code-parity | Lives in `educlaw-server` — must be written |
| xterm.js browser frontend | Not in claw-code-parity | Lives in `educlaw-web/` — must be written |
| SQLite student registry | Not in claw-code-parity | Lives in `educlaw-server/src/db.rs` — must be written |
| `educlaw-mcp` MCP server process | Not in claw-code-parity | `educlaw-mcp` must be written; connects to claw-code-parity as MCP client |
| Idle watchdog Tokio task | Not in claw-code-parity | Lives in `educlaw-server` — must be written |
| Python PDF→MD conversion pipeline | Not in claw-code-parity | Python script on host — independent |

---

## Verdict: will your plan succeed?

**Yes — the plan is sound and the foundation is real.** Here is why:

### Strong positives
1. **The plugin system is production-grade.** `PluginManager` (install/enable/disable, hooks, lifecycle, tool manifest) is fully implemented. The EduClaw plugin — which injects the student prompt and connects MCP — can be written as a standard external plugin with a `plugin.json`. No fork required.
2. **MCP client is already wired.** The `McpToolBridge` (Lane 7) speaks stdio, SSE, HTTP, and WS. Your `educlaw-mcp` server just needs to implement the MCP protocol over one of those transports.
3. **Permission enforcer exists.** Mapping each student session's workspace root to `/data/students/{id}/upper` gives you filesystem scoping for free via the existing `check_file_write()` boundary enforcement.
4. **The `sandbox.rs` already probes `unshare` capability.** Your `namespace.rs` builds on the same kernel feature — the Rust plumbing (`nix` crate) is already a dependency in the workspace, so your namespace spawner fits naturally.
5. **The codebase is Rust + Tokio throughout.** Your plan's `educlaw-server` (Axum + tokio + sqlx + portable-pty) uses the same async runtime and is trivially interoperable.

### Key architectural clarification
The plan correctly treats `claw-code-parity` as a **submodule / dependency**, not the thing you modify. The EduClaw server is a separate Rust workspace that:
- Spawns `claw` as a child process inside the student's namespace
- Opens a PTY on its side, streams bytes over WebSocket
- Manages session lifecycle, auth, DB, MCP server

You do **not** need to fork claw-code-parity to build EduClaw. The plugin hook + an env var for student context is enough.

### One real risk
The roadmap explicitly says claw-code-parity is **not yet** a full runtime-equivalent replacement for the original TypeScript Claude Code. Specifically:
- No real `AskUserQuestion` UI wiring (interactive confirm prompts block)
- CI not always green
- MCP end-to-end lifecycle still has gaps

**Mitigation:** For EduClaw's Phase 1–2, you only need bash, file read/write, and MCP tool dispatch — all of which are the most complete parts of the repo. The shallow areas (interactive prompts, cron, team) are not required for the MVP.

---

## Recommended first steps

1. **Write the EduClaw plugin** (`educlaw-plugin/plugin.json` + `init.sh`) — reads `EDUCLAW_STUDENT_JSON`, writes the rendered system prompt file, connects `educlaw-mcp` via MCP stdio transport
2. **Start `educlaw-server`** — Axum skeleton with JWT auth, `sqlx` SQLite, and a basic PTY-over-WebSocket bridge using `portable-pty` + `tokio-tungstenite`
3. **Test namespace spawn on a Linux VM** — the `nix` crate `unshare` + OverlayFS mount is the highest-risk item; validate it early
4. **Wire `educlaw-mcp`** — implement `StudentContext` and `StudentFiles` MCP servers first; `ClassFiles` and `EvalEngine` can follow
5. **Build the xterm.js frontend last** — the terminal + file panel split is straightforward once the WebSocket bridge is stable

---

## Folder layout after clone

```
/home/pilser/Edu-Claw/
├── EduClaw_Architecture_Plan_1.docx   ← original plan doc
├── gen_educlaw.js                     ← doc generator
├── ANALYSIS.md                        ← this file
└── claw-code-parity/                  ← cloned repo
    ├── rust/                          ← 9-crate Rust workspace (main implementation)
    │   ├── crates/runtime/            ← session, sandbox, MCP bridge, permissions, LSP
    │   ├── crates/tools/              ← 40 tool specs + execute_tool() dispatch
    │   ├── crates/plugins/            ← PluginManager, hooks, lifecycle
    │   ├── crates/rusty-claude-cli/   ← `claw` binary entrypoint
    │   └── crates/mock-anthropic-service/ ← test harness
    ├── src/                           ← Python porting workspace (67 modules)
    ├── PARITY.md                      ← 9-lane checkpoint with commit hashes
    └── ROADMAP.md                     ← clawable harness roadmap
```
