// educlaw-server/src/session/namespace.rs
//
// OverlayFS management (via fuse-overlayfs) and claw session spawn.
// Uses the unshare(mount+pid) approach — no Docker, sub-100ms startup.

use std::path::Path;

use tokio::process::Command;

use crate::error::{AppError, Result};
use crate::session::dirs::SessionDirs;

// ── OverlayFS ──────────────────────────────────────────────────────────────

/// Mount fuse-overlayfs over the session's lower/upper/work dirs into merged/.
pub async fn mount_overlay(dirs: &SessionDirs) -> Result<()> {
    let output = Command::new("fuse-overlayfs")
        .arg("-o")
        .arg(format!(
            "lowerdir={},upperdir={},workdir={}",
            dirs.lower.display(),
            dirs.upper.display(),
            dirs.work.display(),
        ))
        .arg(dirs.merged.display().to_string())
        .output()
        .await
        .map_err(|e| AppError::Session(format!("fuse-overlayfs failed to start: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::Session(format!(
            "fuse-overlayfs mount failed: {stderr}"
        )));
    }
    Ok(())
}

/// Unmount fuse-overlayfs (best-effort — does not propagate errors).
pub async fn umount_overlay(merged: &Path) {
    let _ = Command::new("fusermount")
        .arg("-u")
        .arg(merged.display().to_string())
        .output()
        .await;
}

// ── Plugin install ─────────────────────────────────────────────────────────

/// Recursively copy the educlaw-plugin source tree into the session plugin dir.
pub fn install_plugin(plugin_src: &str, dest: &Path) -> std::io::Result<()> {
    copy_dir_recursive(Path::new(plugin_src), dest)
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        if path.is_dir() {
            copy_dir_recursive(&path, &dst.join(&name))?;
        } else {
            std::fs::copy(&path, dst.join(&name))?;
        }
    }
    Ok(())
}

// ── Session spawn ──────────────────────────────────────────────────────────
//
// Primary spawn path: pty_bridge.rs uses portable-pty which handles PTY
// attachment. This function is the namespace-only wrapper, useful for
// headless sessions or future broker-managed spawns.

#[derive(Debug)]
pub struct SpawnParams {
    pub session_id: String,
    pub student_json: String,
    pub dirs: SessionDirs,
    pub claw_bin: String,
    pub model: Option<String>,
    /// Extra env vars (API keys, etc.) to forward into the session.
    pub env: Vec<(String, String)>,
}

/// Spawn `claw` inside `unshare --mount --pid --fork`.
/// Returns the OS PID of the outer wrapper process.
pub async fn spawn_claw_session(params: &SpawnParams) -> Result<u32> {
    let mut claw_args = vec![params.claw_bin.clone()];
    if let Some(ref model) = params.model {
        claw_args.push("--model".to_string());
        claw_args.push(model.clone());
    }
    claw_args.push("--add-dir".to_string());
    claw_args.push(params.dirs.plugin.display().to_string());
    let claw_cmd = claw_args.join(" ");

    let mut cmd = Command::new("unshare");
    cmd.args(["--mount", "--pid", "--fork", "sh", "-c", &claw_cmd]);
    cmd.current_dir(&params.dirs.merged);
    cmd.env("CLAW_CONFIG_HOME", &params.dirs.config);
    cmd.env("EDUCLAW_STUDENT_JSON", &params.student_json);
    cmd.env("EDUCLAW_SESSION_ID", &params.session_id);
    cmd.env("EDUCLAW_WORKSPACE", &params.dirs.merged);
    for (key, value) in &params.env {
        cmd.env(key, value);
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| AppError::Session(format!("failed to spawn claw: {e}")))?;

    let pid = child
        .id()
        .ok_or_else(|| AppError::Session("spawned process has no PID".to_string()))?;

    // Let Tokio reap the child when it exits; PTY bridge manages the actual I/O.
    tokio::spawn(async move {
        let _ = child.wait().await;
    });

    Ok(pid)
}
