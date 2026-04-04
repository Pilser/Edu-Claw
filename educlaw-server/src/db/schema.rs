// educlaw-server/src/db/schema.rs
//
// Inline DDL migrations — run once at server startup.
// Column names and types follow the plan exactly.

use sqlx::SqlitePool;

pub async fn init_db(pool: &SqlitePool) -> anyhow::Result<()> {
    // ── students ─────────────────────────────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS students (
            id             TEXT PRIMARY KEY,
            username       TEXT NOT NULL UNIQUE,
            password_hash  TEXT NOT NULL,
            display_name   TEXT NOT NULL,
            role           TEXT NOT NULL DEFAULT 'student',
            class_ids      TEXT NOT NULL DEFAULT '[]',
            uid_slot       INTEGER,
            created_at     TEXT NOT NULL DEFAULT (datetime('now')),
            last_seen      TEXT
        )",
    )
    .execute(pool)
    .await?;

    // ── sessions (ephemeral audit) ────────────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS sessions (
            id              TEXT PRIMARY KEY,
            student_id      TEXT NOT NULL REFERENCES students(id),
            course          TEXT NOT NULL,
            assignment      TEXT,
            state           TEXT NOT NULL DEFAULT 'active',
            namespace_pid   INTEGER,
            overlay_mount   TEXT NOT NULL,
            last_input_at   TEXT,
            started_at      TEXT NOT NULL DEFAULT (datetime('now')),
            ended_at        TEXT
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_sessions_student ON sessions(student_id)",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_sessions_state ON sessions(state)",
    )
    .execute(pool)
    .await?;

    tracing::info!("database schema initialised");
    Ok(())
}
