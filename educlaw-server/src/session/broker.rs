// educlaw-server/src/session/broker.rs
//
// In-memory session registry shared by:
//   - api/sessions.rs  — creates entry when session is born
//   - session/watchdog.rs — reads last_input_at, sends idle warnings
//   - api/ws.rs        — sets/clears pty_tx on connect/disconnect
//   - pty_bridge.rs    — updates last_input_at on every keypress

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::sync::{mpsc, Mutex};

// ── Entry ─────────────────────────────────────────────────────────────────

struct SessionEntry {
    /// Unix epoch seconds of the last keypress. Updated lock-free by pty_bridge.
    last_input_at: Arc<AtomicU64>,
    /// Present while a WebSocket is connected; None when browser is gone.
    pty_tx: Option<mpsc::Sender<Vec<u8>>>,
}

// ── Broker ────────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct SessionBroker {
    sessions: Mutex<HashMap<String, SessionEntry>>,
}

impl SessionBroker {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    // ── Lifecycle called by api/sessions.rs ───────────────────────────────

    /// Called when a new session is created. Initialises last_input_at.
    pub async fn create_entry(&self, session_id: &str, last_input_at: Arc<AtomicU64>) {
        self.sessions.lock().await.insert(
            session_id.to_string(),
            SessionEntry { last_input_at, pty_tx: None },
        );
    }

    /// Called by terminate_session API and by the watchdog on timeout.
    pub async fn remove_entry(&self, session_id: &str) {
        self.sessions.lock().await.remove(session_id);
    }

    // ── Called by api/ws.rs on WS connect / disconnect ───────────────────

    /// Return the last_input_at Arc so pty_bridge can update it directly.
    pub async fn get_last_input_arc(&self, session_id: &str) -> Option<Arc<AtomicU64>> {
        self.sessions
            .lock()
            .await
            .get(session_id)
            .map(|e| Arc::clone(&e.last_input_at))
    }

    /// Set or clear the PTY sender (Some on connect, None on disconnect).
    pub async fn set_pty_sender(&self, session_id: &str, tx: Option<mpsc::Sender<Vec<u8>>>) {
        let mut map = self.sessions.lock().await;
        if let Some(entry) = map.get_mut(session_id) {
            entry.pty_tx = tx;
        }
    }

    // ── Called by watchdog ────────────────────────────────────────────────

    /// Returns None if the session has been removed (already terminated).
    pub async fn last_input_secs(&self, session_id: &str) -> Option<u64> {
        self.sessions
            .lock()
            .await
            .get(session_id)
            .map(|e| e.last_input_at.load(Ordering::Relaxed))
    }

    /// List all live entries as (session_id, last_input_at_secs).
    /// Used by the admin API to show idle times without hitting the DB.
    pub async fn list_entries(&self) -> Vec<(String, u64)> {
        self.sessions
            .lock()
            .await
            .iter()
            .map(|(id, e)| (id.clone(), e.last_input_at.load(Ordering::Relaxed)))
            .collect()
    }

    /// Send raw bytes to the PTY (e.g. watchdog idle warning banner).
    /// Silently no-ops if the browser is disconnected or session is gone.
    pub async fn write_to_pty(&self, session_id: &str, data: Vec<u8>) {
        // Clone the sender under the lock, then send outside the lock.
        let tx = {
            let map = self.sessions.lock().await;
            map.get(session_id)
                .and_then(|e| e.pty_tx.clone())
        };
        if let Some(tx) = tx {
            let _ = tx.send(data).await;
        }
    }
}

// ── Helper ────────────────────────────────────────────────────────────────

/// Current Unix epoch in whole seconds.
pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
