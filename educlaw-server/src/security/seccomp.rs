// educlaw-server/src/security/seccomp.rs
//
// Builds a seccomp-BPF deny-list filter for student session processes.
//
// Strategy: default-ALLOW + block the most dangerous syscalls.
// This is less restrictive than a strict allowlist but safe to deploy
// without risking breaking legitimate Python/bash usage.
//
// Blocked syscalls (kill process on attempt):
//   ptrace, process_vm_readv/writev — no cross-process memory access
//   reboot, kexec_load/file_load    — no system-level operations
//   syslog                           — no kernel ring-buffer reads
//   mknod, mknodat                   — no device file creation
//   *_module (create/init/finit/delete) — no kernel module loading
//   perf_event_open                  — no performance counter side-channel
//   bpf                              — no eBPF program injection
//   userfaultfd                      — no page-fault side-channel

use anyhow::Result;

#[cfg(target_os = "linux")]
use seccompiler::{BpfProgram, SeccompAction, SeccompFilter, SeccompRule};

/// Build the student session BPF program (compile once, apply in pre_exec).
/// Returns `None` on non-Linux platforms so callers can skip it gracefully.
#[cfg(target_os = "linux")]
pub fn build_student_filter() -> Result<BpfProgram> {
    use std::collections::BTreeMap;

    let blocked_syscalls: &[i64] = &[
        libc::SYS_ptrace,
        libc::SYS_process_vm_readv,
        libc::SYS_process_vm_writev,
        libc::SYS_reboot,
        libc::SYS_kexec_load,
        libc::SYS_kexec_file_load,
        libc::SYS_syslog,
        libc::SYS_mknod,
        libc::SYS_mknodat,
        libc::SYS_create_module,
        libc::SYS_init_module,
        libc::SYS_finit_module,
        libc::SYS_delete_module,
        libc::SYS_perf_event_open,
        libc::SYS_bpf,
        libc::SYS_userfaultfd,
    ];

    let mut rules: BTreeMap<i64, Vec<SeccompRule>> = BTreeMap::new();
    for &syscall in blocked_syscalls {
        // Empty rule vec = match on any arguments → block unconditionally.
        rules.insert(syscall, vec![]);
    }

    let filter = SeccompFilter::new(
        rules,
        SeccompAction::Allow,       // default: allow all unlisted syscalls
        SeccompAction::KillProcess, // on match: kill the offending process
        std::env::consts::ARCH.try_into()?,
    )?;

    Ok(filter.try_into()?)
}

#[cfg(not(target_os = "linux"))]
pub fn build_student_filter() -> Result<()> {
    Ok(())
}
