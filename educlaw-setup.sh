#!/usr/bin/env bash
# EduClaw — production setup CLI
# Usage: sudo ./educlaw-setup.sh [command]
#
# Commands (run in order on a fresh machine):
#   dirs        Create all required directories
#   env         Write /srv/educlaw/.env from the dev .env (generates new secrets)
#   plugin      Copy plugin hooks to /srv/educlaw/educlaw-plugin/
#   service     Install and enable the systemd unit (requires root)
#   status      Show what is installed and what is missing
#   all         Run: dirs → env → plugin → service
#
# After running this script, copy the built files:
#   sudo cp educlaw-server/target/release/educlaw-server  /srv/educlaw/
#   sudo cp educlaw-mcp/target/release/educlaw-mcp        /srv/educlaw/
#   sudo cp claw                                           /srv/educlaw/
#   cp -r educlaw-web/dist/.                         /srv/educlaw/educlaw-web/dist/

set -euo pipefail

# ── Config ────────────────────────────────────────────────────────────────
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SRV=/srv/educlaw
VAR=/var/lib/educlaw
CGROUP_ROOT=/sys/fs/cgroup/educlaw
SERVICE_FILE=/etc/systemd/system/educlaw.service
DEPLOY_USER="${SUDO_USER:-$(whoami)}"

# ── Colours ───────────────────────────────────────────────────────────────
GREEN='\033[0;32m'; YELLOW='\033[1;33m'; RED='\033[0;31m'; NC='\033[0m'
ok()   { echo -e "${GREEN}✔${NC} $*"; }
warn() { echo -e "${YELLOW}⚠${NC}  $*"; }
fail() { echo -e "${RED}✖${NC} $*"; }
info() { echo -e "  $*"; }

# ── Helpers ───────────────────────────────────────────────────────────────
need_root() {
  if [[ $EUID -ne 0 ]]; then
    fail "This command must be run as root (sudo $0 $*)."
    exit 1
  fi
}

cmd_dirs() {
  need_root dirs

  echo "Creating directory tree..."

  # Static / binary tree
  for d in \
    "$SRV" \
    "$SRV/data/subjects" \
    "$SRV/educlaw-web/dist" \
    "$SRV/educlaw-plugin/hooks"; do
    mkdir -p "$d" && ok "$d"
  done

  # Runtime / data tree
  for d in "$VAR/sessions" "$VAR/students"; do
    mkdir -p "$d" && ok "$d"
  done

  # cgroups v2 subtree
  if [[ -d /sys/fs/cgroup ]]; then
    mkdir -p "$CGROUP_ROOT" && ok "$CGROUP_ROOT"
  else
    warn "/sys/fs/cgroup not found — skipping cgroups dir (is cgroups v2 mounted?)"
  fi

  # Hand ownership to the user that will run the server
  chown -R "$DEPLOY_USER:$DEPLOY_USER" "$SRV"
  chown -R "$DEPLOY_USER:$DEPLOY_USER" "$VAR"
  [[ -d "$CGROUP_ROOT" ]] && chown -R "$DEPLOY_USER:$DEPLOY_USER" "$CGROUP_ROOT"

  ok "All directories created and owned by $DEPLOY_USER"
}

cmd_env() {
  need_root env

  local src="$SCRIPT_DIR/.env"
  local dst="$SRV/.env"

  if [[ ! -f "$src" ]]; then
    fail "No .env found at $src"
    exit 1
  fi

  if [[ -f "$dst" ]]; then
    warn "$dst already exists — skipping (remove it manually to regenerate)."
    return
  fi

  cp "$src" "$dst"

  # Point CLAW_BIN at the production binary location
  sed -i "s|^CLAW_BIN=.*|CLAW_BIN=$SRV/claw|" "$dst"

  chown "$DEPLOY_USER:$DEPLOY_USER" "$dst"
  chmod 600 "$dst"

  ok "$dst written — edit it to set JWT_SECRET, ADMIN_SECRET, etc."
}

cmd_plugin() {
  need_root plugin

  local src="$SCRIPT_DIR/educlaw-plugin"
  local dst="$SRV/educlaw-plugin"

  if [[ ! -d "$src" ]]; then
    fail "Plugin source not found at $src"
    exit 1
  fi

  cp -r "$src/." "$dst/"
  chown -R "$DEPLOY_USER:$DEPLOY_USER" "$dst"
  ok "Plugin copied to $dst"
}

cmd_service() {
  need_root service

  cat > "$SERVICE_FILE" <<UNIT
[Unit]
Description=EduClaw Server
After=network.target

[Service]
Type=simple
User=$DEPLOY_USER
WorkingDirectory=$SRV
EnvironmentFile=$SRV/.env
ExecStart=$SRV/educlaw-server
Restart=on-failure
RestartSec=5
AmbientCapabilities=CAP_SYS_ADMIN
CapabilityBoundingSet=CAP_SYS_ADMIN

[Install]
WantedBy=multi-user.target
UNIT

  systemctl daemon-reload
  systemctl enable educlaw
  ok "systemd unit installed: $SERVICE_FILE"
  info "Start with:  sudo systemctl start educlaw"
  info "Watch logs:  sudo journalctl -u educlaw -f"
}

cmd_status() {
  echo ""
  echo "── Binaries ────────────────────────────────────────────"
  for f in \
    "$SRV/educlaw-server" \
    "$SRV/educlaw-mcp" \
    "$SRV/claw"; do
    if [[ -x "$f" ]]; then ok "$f"; else fail "$f  ← still needs to be copied"; fi
  done

  echo ""
  echo "── Directories ─────────────────────────────────────────"
  for d in \
    "$SRV/data/subjects" \
    "$SRV/educlaw-plugin/hooks" \
    "$SRV/educlaw-web/dist" \
    "$VAR/sessions" \
    "$VAR/students" \
    "$CGROUP_ROOT"; do
    if [[ -d "$d" ]]; then ok "$d"; else fail "$d"; fi
  done

  echo ""
  echo "── Config ──────────────────────────────────────────────"
  if [[ -f "$SRV/.env" ]]; then
    ok "$SRV/.env"
    # Show keys present without revealing values
    while IFS= read -r line; do
      [[ "$line" =~ ^#.*$ || -z "$line" ]] && continue
      key="${line%%=*}"
      info "  $key=…"
    done < "$SRV/.env"
  else
    fail "$SRV/.env  ← run: sudo $0 env"
  fi

  echo ""
  echo "── systemd ─────────────────────────────────────────────"
  if [[ -f "$SERVICE_FILE" ]]; then
    ok "$SERVICE_FILE"
    systemctl is-active educlaw &>/dev/null \
      && ok "  service is running" \
      || warn "  service is installed but not running"
  else
    warn "$SERVICE_FILE not installed  ← run: sudo $0 service"
  fi
  echo ""
}

cmd_all() {
  cmd_dirs
  cmd_env
  cmd_plugin
  cmd_service
  echo ""
  ok "Setup complete. Now copy the built files:"
  echo ""
  echo "  # Binaries"
  echo "  sudo cp $SCRIPT_DIR/educlaw-server/target/release/educlaw-server  $SRV/"
  echo "  sudo cp $SCRIPT_DIR/educlaw-mcp/target/release/educlaw-mcp        $SRV/"
  echo "  sudo cp $SCRIPT_DIR/claw                                           $SRV/"
  echo ""
  echo "  # Frontend (build it first: cd educlaw-web && bun run build)"
  echo "  cp -r $SCRIPT_DIR/educlaw-web/dist/. $SRV/educlaw-web/dist/"
  echo ""
  echo "  sudo systemctl start educlaw"
}

usage() {
  echo "Usage: sudo $0 <command>"
  echo ""
  echo "Commands:"
  echo "  dirs     Create all required directories"
  echo "  env      Write $SRV/.env (generates new JWT_SECRET + ADMIN_SECRET)"
  echo "  plugin   Copy plugin hooks to $SRV/educlaw-plugin/"
  echo "  service  Install systemd unit"
  echo "  status   Show installation status"
  echo "  all      Run dirs → env → plugin → service"
}

# ── Dispatch ──────────────────────────────────────────────────────────────
case "${1:-}" in
  dirs)    cmd_dirs    ;;
  env)     cmd_env     ;;
  plugin)  cmd_plugin  ;;
  service) cmd_service ;;
  status)  cmd_status  ;;
  all)     cmd_all     ;;
  *)       usage       ;;
esac
