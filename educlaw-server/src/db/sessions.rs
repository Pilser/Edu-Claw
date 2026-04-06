// educlaw-server/src/db/sessions.rs
//
// Session record.
//   id, student_id, state, namespace_pid,
//   overlay_mount, last_input_at, started_at, ended_at
//
// state values: active | idle_warning | terminating | ended

use sqlx::SqlitePool;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub struct Session {
    pub id: String,
    pub student_id: String,
    /// active | idle_warning | terminating | ended
    pub state: String,
    /// PID of init process inside the namespace (set after spawn).
    pub namespace_pid: Option<i64>,
    /// Absolute path to the OverlayFS merged mount point.
    pub overlay_mount: String,
    /// Updated on every keypress; used by the idle watchdog.
    pub last_input_at: Option<String>,
    pub started_at: String,
    pub ended_at: Option<String>,
}

// ── Writes ────────────────────────────────────────────────────────────────

pub async fn insert_session(pool: &SqlitePool, s: &Session) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO sessions
             (id, student_id, state, overlay_mount)
         VALUES (?, ?, ?, ?)",
    )
    .bind(&s.id)
    .bind(&s.student_id)
    .bind(&s.state)
    .bind(&s.overlay_mount)
    .execute(pool)
    .await?;
    Ok(())
}

/// Transition session to the next state.
/// Setting state = "ended" also stamps ended_at.
pub async fn set_session_state(
    pool: &SqlitePool,
    id: &str,
    state: &str,
) -> Result<(), sqlx::Error> {
    if state == "ended" {
        sqlx::query(
            "UPDATE sessions SET state = ?, ended_at = datetime('now') WHERE id = ?",
        )
        .bind(state)
        .bind(id)
        .execute(pool)
        .await?;
    } else {
        sqlx::query("UPDATE sessions SET state = ? WHERE id = ?")
            .bind(state)
            .bind(id)
            .execute(pool)
            .await?;
    }
    Ok(())
}

pub async fn set_namespace_pid(
    pool: &SqlitePool,
    id: &str,
    pid: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE sessions SET namespace_pid = ? WHERE id = ?")
        .bind(pid)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Called on every keypress to reset idle timer.
pub async fn touch_last_input(pool: &SqlitePool, id: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE sessions SET last_input_at = datetime('now') WHERE id = ?",
    )
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

// ── Reads ─────────────────────────────────────────────────────────────────

pub async fn get_session(pool: &SqlitePool, id: &str) -> Result<Option<Session>, sqlx::Error> {
    sqlx::query_as::<_, Session>("SELECT * FROM sessions WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
}

pub async fn list_active_sessions(pool: &SqlitePool) -> Result<Vec<Session>, sqlx::Error> {
    sqlx::query_as::<_, Session>(
        "SELECT * FROM sessions WHERE state = 'active' ORDER BY started_at DESC",
    )
    .fetch_all(pool)
    .await
}


