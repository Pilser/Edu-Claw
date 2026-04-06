// educlaw-server/src/db/schema.rs
//
// Inline DDL migrations — run once at server startup.
// Column names and types follow the plan exactly.

use sqlx::SqlitePool;

pub async fn init_db(pool: &SqlitePool) -> anyhow::Result<()> {
    // ── cohorts ───────────────────────────────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS cohorts (
            id         TEXT PRIMARY KEY,
            title      TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;

    // ── subjects ──────────────────────────────────────────────────────────────
    // Scoped per cohort: (cohort_id, slug) is the natural unique key.
    // Disk layout: {EDUCLAW_SUBJECTS_ROOT}/{cohort_id}/{slug}/
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS subjects (
            id         TEXT PRIMARY KEY,
            cohort_id  TEXT NOT NULL,
            slug       TEXT NOT NULL,
            title      TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;
    sqlx::query(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_subjects_cohort_slug ON subjects(cohort_id, slug)",
    )
    .execute(pool)
    .await?;
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_subjects_cohort ON subjects(cohort_id)",
    )
    .execute(pool)
    .await?;

    // ── assignments ───────────────────────────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS assignments (
            id          TEXT PRIMARY KEY,
            subject_id  TEXT NOT NULL REFERENCES subjects(id),
            title       TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            due_date    TEXT,
            created_by  TEXT NOT NULL,
            created_at  TEXT NOT NULL DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;

    // ── students ──────────────────────────────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS students (
            id             TEXT PRIMARY KEY,
            username       TEXT NOT NULL UNIQUE,
            password_hash  TEXT NOT NULL,
            display_name   TEXT NOT NULL,
            role           TEXT NOT NULL DEFAULT 'student',
            cohort_id      TEXT NOT NULL DEFAULT '',
            uid_slot       INTEGER,
            created_at     TEXT NOT NULL DEFAULT (datetime('now')),
            last_seen      TEXT
        )",
    )
    .execute(pool)
    .await?;

    // ── sessions ──────────────────────────────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS sessions (
            id              TEXT PRIMARY KEY,
            student_id      TEXT NOT NULL REFERENCES students(id),
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

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_assignments_subject ON assignments(subject_id)",
    )
    .execute(pool)
    .await?;

    tracing::info!("database schema initialised");
    Ok(())
}
