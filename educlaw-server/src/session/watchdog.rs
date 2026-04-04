// educlaw-server/src/session/watchdog.rs
//
// Per-session idle watchdog — spawned as a Tokio task in api/sessions.rs.
//
// Timeline (defaults: idle_timeout=900s, warn_before=60s):
//   every 30 s : check elapsed since last keypress via broker
//   at T-60 s  : write ANSI warning banner to PTY
//   at T       : set DB state → "ended", write goodbye, umount overlay
//
// The task exits when:
//   - The session times out (handled here).
//   - The entry is removed from broker externally (DELETE /sessions/:id).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use sqlx::SqlitePool;
use tokio::time::sleep;

use crate::db::sessions;
use crate::session::{
    broker::{now_secs, SessionBroker},
    namespace::umount_overlay,
};

// ── Config ────────────────────────────────────────────────────────────────

pub struct WatchdogConfig {
    pub session_id: String,
    /// Path to the OverlayFS merged mount (for umount on timeout).
    pub overlay_mount: PathBuf,
    pub idle_timeout: Duration,
    pub warn_before: Duration,
}

// ── Task ──────────────────────────────────────────────────────────────────

pub async fn run_watchdog(
    config: WatchdogConfig,
    broker: Arc<SessionBroker>,
    pool: SqlitePool,
) {
    let mut warned = false;

    loop {
        sleep(Duration::from_secs(30)).await;

        // Session removed from broker — terminated externally, stop.
        let last_secs = match broker.last_input_secs(&config.session_id).await {
            Some(t) => t,
            None => break,
        };

        let elapsed = Duration::from_secs(now_secs().saturating_sub(last_secs));

        // Hard timeout: terminate
        if elapsed >= config.idle_timeout {
            let _ = sessions::set_session_state(&pool, &config.session_id, "ended").await;

            broker
                .write_to_pty(
                    &config.session_id,
                    b"\r\n\x1b[33m[EduClaw] Session timed out. Your files are saved.\x1b[0m\r\n"
                        .to_vec(),
                )
                .await;

            broker.remove_entry(&config.session_id).await;
            umount_overlay(&config.overlay_mount).await;

            tracing::info!(
                session_id = %config.session_id,
                "watchdog: idle timeout — session terminated"
            );
            break;
        }

        // Warning threshold
        let warn_threshold = config.idle_timeout.saturating_sub(config.warn_before);
        if elapsed >= warn_threshold && !warned {
            let secs_left = (config.idle_timeout - elapsed).as_secs();
            let msg = format!(
                "\r\n\x1b[33m[EduClaw] Session idle \u{2014} {} s to disconnect.\x1b[0m\r\n",
                secs_left
            );
            broker
                .write_to_pty(&config.session_id, msg.into_bytes())
                .await;
            warned = true;
            tracing::debug!(
                session_id = %config.session_id,
                secs_left,
                "watchdog: idle warning sent"
            );
        }

        // Check if an external DELETE already ended the session in DB.
        match sessions::get_session(&pool, &config.session_id).await {
            Ok(Some(s)) if s.state == "ended" => break,
            Ok(None) => break,
            _ => {}
        }
    }
}
