// educlaw-server/src/db/students.rs
//
// Student record.
//   id, username, password_hash, display_name, role, cohort_id, uid_slot,
//   created_at, last_seen

use sqlx::SqlitePool;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub struct Student {
    pub id: String,
    pub username: String,
    /// Argon2id PHC string — never serialised into API responses.
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub display_name: String,
    /// "student" | "teacher" | "admin"
    pub role: String,
    /// ID of the cohort this student belongs to (e.g. "primary-2b").
    pub cohort_id: String,
    /// Host uid mapping while a session is active; NULL when idle.
    pub uid_slot: Option<i64>,
    pub created_at: String,
    pub last_seen: Option<String>,
}

// ── Writes ────────────────────────────────────────────────────────────────

pub async fn insert_student(pool: &SqlitePool, s: &Student) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO students
             (id, username, password_hash, display_name, role, cohort_id)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&s.id)
    .bind(&s.username)
    .bind(&s.password_hash)
    .bind(&s.display_name)
    .bind(&s.role)
    .bind(&s.cohort_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn touch_last_seen(pool: &SqlitePool, id: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE students SET last_seen = datetime('now') WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn update_student(
    pool: &SqlitePool,
    id: &str,
    display_name: Option<&str>,
    cohort_id: Option<&str>,
    password_hash: Option<&str>,
) -> Result<(), sqlx::Error> {
    if let Some(n) = display_name {
        sqlx::query("UPDATE students SET display_name = ? WHERE id = ?")
            .bind(n).bind(id).execute(pool).await?;
    }
    if let Some(c) = cohort_id {
        sqlx::query("UPDATE students SET cohort_id = ? WHERE id = ?")
            .bind(c).bind(id).execute(pool).await?;
    }
    if let Some(h) = password_hash {
        sqlx::query("UPDATE students SET password_hash = ? WHERE id = ?")
            .bind(h).bind(id).execute(pool).await?;
    }
    Ok(())
}

pub async fn delete_student(pool: &SqlitePool, id: &str) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM students WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn set_uid_slot(
    pool: &SqlitePool,
    id: &str,
    slot: Option<i64>,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE students SET uid_slot = ? WHERE id = ?")
        .bind(slot)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

// ── Reads ─────────────────────────────────────────────────────────────────

pub async fn get_student(pool: &SqlitePool, id: &str) -> Result<Option<Student>, sqlx::Error> {
    sqlx::query_as::<_, Student>("SELECT * FROM students WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
}

pub async fn get_student_by_username(
    pool: &SqlitePool,
    username: &str,
) -> Result<Option<Student>, sqlx::Error> {
    sqlx::query_as::<_, Student>("SELECT * FROM students WHERE username = ?")
        .bind(username)
        .fetch_optional(pool)
        .await
}

pub async fn list_students(pool: &SqlitePool) -> Result<Vec<Student>, sqlx::Error> {
    sqlx::query_as::<_, Student>("SELECT * FROM students ORDER BY created_at DESC")
        .fetch_all(pool)
        .await
}

