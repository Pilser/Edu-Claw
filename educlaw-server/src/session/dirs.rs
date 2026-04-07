// educlaw-server/src/session/dirs.rs
//
// Per-session filesystem layout definition.

use std::path::PathBuf;

/// Filesystem layout for one session.
///
/// Persistent paths (survive across sessions):
///   lower  = /data/subjects/{cohort_id}/          ← cohort course materials (read-only via OverlayFS)
///                                                    cohort_id is a slug (e.g. "primary-2b"), not a UUID
///   upper  = /data/students/{username}/            ← student's home dir (read-write, persisted on disk)
///                                                    username is unique and human-readable to the agent
///
/// Ephemeral paths (created fresh per session, under workspace_root/{session_id}/):
///   work/    ← OverlayFS work dir (must be on the same filesystem as upper/)
///   merged/  ← unified view — agent's working root (EDUCLAW_WORKSPACE)
///   config/  ← per-session CLAW_CONFIG_HOME
///   plugin/  ← per-session copy of educlaw-plugin
#[derive(Debug, Clone)]
pub struct SessionDirs {
    pub root: PathBuf,
    /// Cohort course content — read-only lower layer of OverlayFS.
    /// Must already exist on disk before mount_overlay is called.
    pub lower: PathBuf,
    /// Student's persistent home — read-write upper layer of OverlayFS.
    /// Created if it doesn't exist yet (first session for this student).
    pub upper: PathBuf,
    pub work: PathBuf,
    pub merged: PathBuf,
    pub config: PathBuf,
    pub plugin: PathBuf,
}

impl SessionDirs {
    /// Build paths for a session.
    ///
    /// * `workspace_root` — base dir for ephemeral session dirs
    ///   (e.g. `/var/lib/educlaw/sessions`)
    /// * `session_id`     — unique session UUID
    /// * `lower`          — cohort content dir (must already exist)
    ///   (e.g. `/data/classes/{cohort_id}`)
    /// * `upper`          — student persistent dir (created on first session)
    ///   (e.g. `/data/students/{student_id}`)
    pub fn new(
        workspace_root: &str,
        session_id: &str,
        lower: PathBuf,
        upper: PathBuf,
    ) -> Self {
        let root = PathBuf::from(workspace_root).join(session_id);
        Self {
            lower,
            upper,
            work:   root.join("work"),
            merged: root.join("merged"),
            config: root.join("config"),
            plugin: root.join("plugin"),
            root,
        }
    }

    /// Build only the ephemeral paths for a session that already exists.
    /// Use this in WebSocket handlers and session teardown where the mount
    /// was already done by `create_session` — `lower`/`upper` are not needed.
    pub fn for_existing_session(workspace_root: &str, session_id: &str) -> Self {
        let root = PathBuf::from(workspace_root).join(session_id);
        // lower/upper are not valid in this context — callers must not access them.
        Self {
            lower:  PathBuf::new(),
            upper:  PathBuf::new(),
            work:   root.join("work"),
            merged: root.join("merged"),
            config: root.join("config"),
            plugin: root.join("plugin"),
            root,
        }
    }

    /// Create dirs that need to exist before OverlayFS is mounted.
    ///
    /// * `upper/`  — student persistent home; safe to create if first session.
    /// * `work/`   — OverlayFS scratch; ephemeral.
    /// * `merged/` — mount point; ephemeral.
    /// * `config/` — claw config home; ephemeral.
    /// * `plugin/` — plugin copy; ephemeral.
    ///
    /// `lower/` is *not* created here — it must be pre-populated cohort content.
    /// Returns an error if `lower` does not exist.
    pub fn create_all(&self) -> std::io::Result<()> {
        if !self.lower.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!(
                    "cohort content dir does not exist: {}",
                    self.lower.display()
                ),
            ));
        }
        for dir in [
            &self.upper,
            &self.work,
            &self.merged,
            &self.config,
            &self.plugin,
        ] {
            std::fs::create_dir_all(dir)?;
        }
        Ok(())
    }
}
