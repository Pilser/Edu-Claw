# EduClaw — Implementation Progress

> Tracks what is built, what is in progress, and what is pending.
> Update this file as work completes.

---

## Phase 1 — Foundation (weeks 1–3)

| # | Task | Status | Notes |
|---|------|--------|-------|
| 1.1 | Fork claw-code-parity; add EduClaw plugin skeleton | ✅ Done | `educlaw-plugin/` with `init.sh`, `shutdown.sh`, `pre_tool_use.sh` |
| 1.2 | Plugin: OSC envelope emitter | ⬜ Pending | Phase 4 |
| 1.3 | Plugin: `--system-prompt-file` injection | ✅ Done | `init.sh` writes per-session `settings.json` with Argon2id system prompt |
| 1.4 | `educlaw-server`: Axum + sqlx + JWT auth | ✅ Done | `main.rs`, `auth.rs`, `api/` |
| 1.5 | DB schema: `students` table (plan-aligned) | ✅ Done | `db/schema.rs` — `username`, `password_hash`, `display_name`, `role`, `class_ids`, `uid_slot`, `last_seen` |
| 1.6 | DB schema: `sessions` table (plan-aligned) | ✅ Done | `db/schema.rs` — `state`, `namespace_pid`, `overlay_mount`, `last_input_at` |
| 1.7 | Student CRUD API | ✅ Done | `api/students.rs` — POST/GET `/students`, GET `/students/:id` |
| 1.8 | Student login endpoint | ✅ Done | `api/auth_routes.rs` — `POST /auth/login` with Argon2id verify → JWT |
| 1.9 | JWT: carries `student_id + role + class_ids` | ✅ Done | `auth.rs` — `Claims` struct matches plan |
| 1.10 | OverlayFS mount via `fuse-overlayfs` | ✅ Done | `session/namespace.rs` — `mount_overlay` / `umount_overlay` |
| 1.11 | Session dir layout (lower/upper/work/merged/config/plugin) | ✅ Done | `session/dirs.rs` |
| 1.12 | PTY open + WebSocket bridge (binary frames) | ✅ Done | `pty_bridge.rs` via `portable-pty` |
| 1.13 | Namespace spawn (`unshare --mount --pid`) | ✅ Done | `session/namespace.rs` — `spawn_claw_session` |

---

## Phase 2 — Session management (weeks 4–6)

| # | Task | Status | Notes |
|---|------|--------|-------|
| 2.1 | Session broker: spawn, register, lookup | ✅ Done | `session/broker.rs` — `Arc<SessionBroker>` in AppState; per-session `SessionEntry` with `last_input_at` Arc + `pty_tx` Option |
| 2.2 | Idle watchdog Tokio task | ✅ Done | `session/watchdog.rs` — `run_watchdog` with 30s poll, warn at T-60s, terminate at T; spawned in `api/sessions.rs` |
| 2.3 | Graceful teardown: SIGTERM → umount → DB update | ✅ Done | `DELETE /sessions/:id` + watchdog both call `remove_entry` + `umount_overlay` + set_session_state "ended" |
| 2.4 | `last_input_at` tracking | ✅ Done | `pty_bridge.rs` updates `Arc<AtomicU64>` on every keypress; broker holds the Arc; watchdog reads it lock-free |
| 2.5 | Resize event: TIOCSWINSZ from WS JSON | ✅ Done | `pty_bridge.rs` — `ClientMessage::Resize` |
| 2.6 | xterm.js browser UI (split-panel layout) | ✅ Done | `educlaw-web/` — `index.html`, `terminal.js`, `api.js`, `style.css`; served via `tower-http ServeDir` |

---

## Phase 3 — MCP & AI integration (weeks 7–9)

| # | Task | Status | Notes |
|---|------|--------|-------|
| 3.1 | `educlaw-mcp` process skeleton | ⬜ Pending | |
| 3.2 | StudentContext MCP server | ⬜ Pending | |
| 3.3 | ClassFiles MCP server | ⬜ Pending | |
| 3.4 | StudentFiles MCP server | ⬜ Pending | |
| 3.5 | EvalEngine MCP server | ⬜ Pending | |
| 3.6 | Shell-in-namespace MCP | ⬜ Pending | |
| 3.7 | `student_id` enforcement on every MCP request | ⬜ Pending | |
| 3.8 | System prompt template injection wired | ✅ Done | `init.sh` draws from `EDUCLAW_STUDENT_JSON` |
| 3.9 | Python lib lower layer image | ⬜ Pending | numpy, sympy, matplotlib, scipy, pandas, rich |

---

## Phase 4 — File browser & agent→browser signals (weeks 10–11)

| # | Task | Status | Notes |
|---|------|--------|-------|
| 4.1 | File browser REST API (7 endpoints) | ⬜ Pending | |
| 4.2 | WsEnvelope dispatcher in browser JS | ⬜ Pending | |
| 4.3 | PTY bridge OSC envelope interceptor (`split_pty_output`) | ⬜ Pending | |
| 4.4 | PDF→MD conversion pipeline | ⬜ Pending | pymupdf + markdownify |
| 4.5 | Image preview via `show_image` envelope | ⬜ Pending | |

---

## Phase 5 — Admin & hardening (weeks 12–14)

| # | Task | Status | Notes |
|---|------|--------|-------|
| 5.1 | Admin REST API (classes, enrollment, sessions) | ⬜ Pending | |
| 5.2 | TeacherBroadcast MCP | ⬜ Pending | |
| 5.3 | seccomp BPF allowlist (`seccompiler` crate) | ⬜ Pending | |
| 5.4 | cgroups v2 resource caps per session | ⬜ Pending | |
| 5.5 | Structured audit logging | ✅ Partial | `tracing` wired; no span-based audit trail yet |
| 5.6 | Load testing (100 concurrent sessions) | ⬜ Pending | |

---

## Known divergences from plan (to fix)

| Item | Plan | Current | Priority |
|------|------|---------|----------|
| Namespace isolation | `nix` crate raw syscalls (uid_map, CLONE_NEWUSER) | `unshare` CLI wrapper | Phase 2 |
| `uid_slot` allocation | Tracked per active session, freed on teardown | Column exists, never set | Phase 2 |
| OverlayFS lower layer | `/data/classes/` global shared read-only tree | Empty `lower/` per session | Phase 3 |
| `last_input_at` | Updated on every keypress; drives watchdog | Column exists, not called from WS | Phase 2 |
| Student classes table | `class_ids` in students, separate classes DB table | `class_ids` JSON stored in students only | Phase 3 |

---

## File structure

```
educlaw-server/src/
  main.rs              ← slim entry point
  error.rs             ← AppError enum + IntoResponse
  auth.rs              ← AuthConfig, Claims, Argon2id helpers
  pty_bridge.rs        ← portable-pty + WebSocket relay
  db/
    mod.rs
    schema.rs          ← CREATE TABLE DDL + init_db()
    students.rs        ← Student struct + CRUD
    sessions.rs        ← Session struct + CRUD
  api/
    mod.rs             ← AppState + routes() assembler
    auth_routes.rs     ← POST /auth/login, POST /auth/token
    students.rs        ← /students CRUD
    sessions.rs        ← /sessions lifecycle
    ws.rs              ← /sessions/:id/ws WebSocket
  session/
    mod.rs
    dirs.rs            ← SessionDirs struct
    namespace.rs       ← fuse-overlayfs + spawn_claw_session
    watchdog.rs        ← Phase 2 stub
```
