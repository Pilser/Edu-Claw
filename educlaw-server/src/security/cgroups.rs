// educlaw-server/src/security/cgroups.rs
//
// cgroups v2 resource caps per student session (Phase 5.4).
//
// At session creation we place the namespace PID into a dedicated cgroup:
//   /sys/fs/cgroup/educlaw/session-{id}/
//
// Limits applied (configurable via env):
//   EDUCLAW_CGROUP_MEMORY_MAX  — default 512m  ("512M" in bytes string)
//   EDUCLAW_CGROUP_CPU_MAX     — default "50000 100000"  (50% of one CPU)
//   EDUCLAW_CGROUP_PIDS_MAX    — default 256   (max processes in session)
//
// NOTE: requires cgroups v2 on the host (Linux 5.10+).
//       Best-effort: all errors are logged but not fatal.

use std::path::{Path, PathBuf};

use tracing::{info, warn};

const CGROUP_ROOT: &str = "/sys/fs/cgroup/educlaw";

fn session_cgroup(session_id: &str) -> PathBuf {
    Path::new(CGROUP_ROOT).join(format!("session-{session_id}"))
}

/// Create a cgroup for the session and add `pid` to it.
/// Must be called after the namespace process has been spawned.
pub async fn setup_session_cgroup(session_id: &str, pid: u32) {
    let cg = session_cgroup(session_id);

    if let Err(e) = tokio::fs::create_dir_all(&cg).await {
        // cgroups v2 delegation requires CAP_SYS_ADMIN or a pre-delegated
        // /sys/fs/cgroup/educlaw/ dir owned by the server user.
        // This is best-effort — log at debug so it doesn't pollute normal runs.
        tracing::debug!(session_id, %e, "cgroup: skipping (no write permission — pre-create /sys/fs/cgroup/educlaw/ or run with CAP_SYS_ADMIN)");
        return;
    }

    // Resource limits from environment (with sane defaults).
    let memory_max = std::env::var("EDUCLAW_CGROUP_MEMORY_MAX")
        .unwrap_or_else(|_| "536870912".to_string()); // 512 MiB
    let cpu_max = std::env::var("EDUCLAW_CGROUP_CPU_MAX")
        .unwrap_or_else(|_| "50000 100000".to_string()); // 50% of one CPU
    let pids_max = std::env::var("EDUCLAW_CGROUP_PIDS_MAX")
        .unwrap_or_else(|_| "256".to_string());

    write_cgroup_file(&cg, "memory.max", &memory_max).await;
    write_cgroup_file(&cg, "cpu.max", &cpu_max).await;
    write_cgroup_file(&cg, "pids.max", &pids_max).await;

    // Add the process to the cgroup.
    write_cgroup_file(&cg, "cgroup.procs", &pid.to_string()).await;

    info!(session_id, pid, "cgroup created with memory/cpu/pid limits");
}

/// Remove the session cgroup on teardown (best-effort).
/// The cgroup must be empty (no processes) before it can be removed.
pub async fn teardown_session_cgroup(session_id: &str) {
    let cg = session_cgroup(session_id);
    if cg.exists() {
        if let Err(e) = tokio::fs::remove_dir(&cg).await {
            // Non-fatal: processes may still be in the cgroup; kernel will
            // clean it up once they all exit.
            warn!(session_id, %e, "cgroup: remove_dir failed (processes may still be running)");
        } else {
            info!(session_id, "cgroup removed");
        }
    }
}

async fn write_cgroup_file(cg: &Path, filename: &str, value: &str) {
    let path = cg.join(filename);
    if let Err(e) = tokio::fs::write(&path, value).await {
        warn!(
            path = %path.display(),
            %e,
            "cgroup: failed to write {filename}"
        );
    }
}
