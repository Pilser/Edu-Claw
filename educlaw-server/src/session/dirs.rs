// educlaw-server/src/session/dirs.rs
//
// Per-session filesystem layout definition.

use std::path::PathBuf;

/// Filesystem layout rooted under `<workspace_root>/<session_id>/`:
///
///   lower/    ← read-only base layer (course skeleton, shared tools)
///   upper/    ← student writes (OverlayFS copy-on-write layer)
///   work/     ← OverlayFS work dir (must be on same fs as upper/)
///   merged/   ← unified view — student's working directory
///   config/   ← per-session CLAW_CONFIG_HOME
///   plugin/   ← per-session copy of educlaw-plugin
#[derive(Debug, Clone)]
pub struct SessionDirs {
    pub root: PathBuf,
    pub lower: PathBuf,
    pub upper: PathBuf,
    pub work: PathBuf,
    pub merged: PathBuf,
    pub config: PathBuf,
    pub plugin: PathBuf,
}

impl SessionDirs {
    pub fn new(workspace_root: &str, session_id: &str) -> Self {
        let root = PathBuf::from(workspace_root).join(session_id);
        Self {
            lower:  root.join("lower"),
            upper:  root.join("upper"),
            work:   root.join("work"),
            merged: root.join("merged"),
            config: root.join("config"),
            plugin: root.join("plugin"),
            root,
        }
    }

    /// Create all six directories on disk.
    pub fn create_all(&self) -> std::io::Result<()> {
        for dir in [
            &self.lower,
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
