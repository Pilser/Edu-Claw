// educlaw-server/src/session/watchdog.rs
//
// Idle-session watchdog — Phase 2 (weeks 4-6).
//
// TODO Phase 2:
//   - Spawn a per-session Tokio task on session creation.
//   - Poll db::sessions::touch_last_input every 30 s.
//   - At (idle_timeout - warn_before_s): write "[EduClaw] Session idle —
//     60 s to disconnect\r\n" to PTY via pty_bridge broadcast channel.
//   - At idle_timeout: set session state → "terminating", SIGTERM the
//     namespace process, call namespace::umount_overlay, set state → "ended".
//   - Configurable via env: IDLE_TIMEOUT_S (default 900), WARN_BEFORE_S (default 60).
