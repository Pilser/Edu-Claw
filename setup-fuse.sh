#!/usr/bin/env bash
# setup-fuse.sh — Check and configure fuse-overlayfs for EduClaw session isolation.
# Run once as a normal user (sudo will be invoked where needed).
set -euo pipefail

BOLD='\033[1m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; RED='\033[0;31m'; NC='\033[0m'

info()    { echo -e "${GREEN}[info]${NC}  $*"; }
warn()    { echo -e "${YELLOW}[warn]${NC}  $*"; }
error()   { echo -e "${RED}[error]${NC} $*"; }
section() { echo -e "\n${BOLD}── $* ──${NC}"; }

# ── 1. Check fuse-overlayfs is installed ─────────────────────────────────────
section "Checking fuse-overlayfs"

if ! command -v fuse-overlayfs &>/dev/null; then
    error "fuse-overlayfs is NOT installed."
    echo
    echo -e "  Install it with one of the following:"
    echo -e "    ${BOLD}Debian / Ubuntu:${NC}  sudo apt install fuse-overlayfs"
    echo -e "    ${BOLD}Fedora / RHEL:${NC}    sudo dnf install fuse-overlayfs"
    echo -e "    ${BOLD}Arch:${NC}             sudo pacman -S fuse-overlayfs"
    echo
    echo "  After installing, re-run this script."
    exit 1
fi

info "fuse-overlayfs is installed: $(command -v fuse-overlayfs)"

# ── 2. Enable unprivileged user namespaces (Debian/Ubuntu) ───────────────────
section "Enabling unprivileged user namespaces"

CURRENT_USERNS=$(sysctl -n kernel.unprivileged_userns_clone 2>/dev/null || echo "N/A")

if [[ "$CURRENT_USERNS" == "1" ]]; then
    info "kernel.unprivileged_userns_clone already set to 1 (live)"
else
    if [[ "$CURRENT_USERNS" == "N/A" ]]; then
        info "kernel.unprivileged_userns_clone not present on this kernel — skipping sysctl step"
    else
        warn "Setting kernel.unprivileged_userns_clone=1 (was: $CURRENT_USERNS)"
        sudo sysctl -w kernel.unprivileged_userns_clone=1
    fi
fi

SYSCTL_CONF=/etc/sysctl.d/99-educlaw.conf
if [[ "$CURRENT_USERNS" != "N/A" ]]; then
    if grep -qsF 'unprivileged_userns_clone=1' "$SYSCTL_CONF" 2>/dev/null; then
        info "$SYSCTL_CONF already configured"
    else
        warn "Writing $SYSCTL_CONF to persist across reboots"
        echo 'kernel.unprivileged_userns_clone=1' | sudo tee "$SYSCTL_CONF" > /dev/null
        info "Written: $SYSCTL_CONF"
    fi
fi

# ── 3. Add current user to the fuse group (if it exists) ─────────────────────
section "Checking fuse group"

if getent group fuse &>/dev/null; then
    if id -nG "$USER" | grep -qw fuse; then
        info "$USER is already in the fuse group"
    else
        warn "Adding $USER to fuse group (re-login required after this)"
        sudo usermod -aG fuse "$USER"
        info "Done. You must log out and back in (or run: newgrp fuse) for the group to take effect."
    fi
else
    info "fuse group does not exist on this system — skipping"
fi

# ── 3b. Grant cap_sys_admin to fuse-overlayfs binary ─────────────────────────
section "Setting fuse-overlayfs capability (cap_sys_admin)"

FUSE_OFS_BIN=$(command -v fuse-overlayfs)
CURRENT_CAP=$(getcap "$FUSE_OFS_BIN" 2>/dev/null || echo "")

if echo "$CURRENT_CAP" | grep -q "cap_sys_admin"; then
    info "cap_sys_admin already set on $FUSE_OFS_BIN"
else
    warn "Granting cap_sys_admin+ep to $FUSE_OFS_BIN"
    sudo setcap cap_sys_admin+ep "$FUSE_OFS_BIN"
    info "Done: $(getcap "$FUSE_OFS_BIN")"
fi

# ── 4. Allow user mounts in /etc/fuse.conf ───────────────────────────────────
section "Configuring /etc/fuse.conf"

FUSE_CONF=/etc/fuse.conf
if grep -qsE '^user_allow_other' "$FUSE_CONF" 2>/dev/null; then
    info "user_allow_other already set in $FUSE_CONF"
else
    warn "Adding user_allow_other to $FUSE_CONF"
    echo 'user_allow_other' | sudo tee -a "$FUSE_CONF" > /dev/null
    info "Done: $FUSE_CONF"
fi

# ── 5. Quick smoke-test ───────────────────────────────────────────────────────
section "Smoke test"

TMPDIR_BASE=$(mktemp -d)
LOWER="$TMPDIR_BASE/lower"
UPPER="$TMPDIR_BASE/upper"
WORK="$TMPDIR_BASE/work"
MERGED="$TMPDIR_BASE/merged"
mkdir -p "$LOWER" "$UPPER" "$WORK" "$MERGED"
echo "ok" > "$LOWER/test.txt"

MOUNT_ERR=$(fuse-overlayfs \
    -o "lowerdir=$LOWER,upperdir=$UPPER,workdir=$WORK" \
    "$MERGED" 2>&1)
MOUNT_RC=$?

if [[ $MOUNT_RC -eq 0 ]]; then
    RESULT=$(cat "$MERGED/test.txt" 2>/dev/null || echo "")
    fusermount -u "$MERGED" 2>/dev/null || true
    rm -rf "$TMPDIR_BASE"
    if [[ "$RESULT" == "ok" ]]; then
        info "Smoke test PASSED — fuse-overlayfs mounts work correctly"
    else
        error "Mount succeeded but read failed (unexpected)"
        exit 1
    fi
else
    rm -rf "$TMPDIR_BASE"
    error "Smoke test FAILED — fuse-overlayfs: $MOUNT_ERR"
    echo
    echo "  Possible fixes:"
    echo "    • Re-run this script (setcap should have been applied above)"
    echo "    • Log out and back in if you were just added to the fuse group"
    echo "    • Or run:  sudo setcap cap_sys_admin+ep \$(command -v fuse-overlayfs)"
    exit 1
fi

# ── Done ──────────────────────────────────────────────────────────────────────
echo
echo -e "${BOLD}${GREEN}All checks passed. EduClaw can mount fuse-overlayfs sessions.${NC}"
