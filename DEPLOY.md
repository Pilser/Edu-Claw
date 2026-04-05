# EduClaw — Production Deployment Guide

> This document covers everything needed to go from a compiled build to a
> running production server. Follow the steps in order.

---

## 1  Why the directory layout is split

EduClaw separates **binaries**, **data**, and **config** deliberately:

| Purpose | Location | Why |
|---------|----------|-----|
| Binaries (`educlaw-server`, `educlaw-mcp`) | `/srv/educlaw/` | Read-only; never change at runtime |
| SQLite database | `/var/lib/educlaw/` | Persistent data survives binary upgrades |
| Session overlays (student workspaces) | `/var/lib/educlaw/sessions/` | Written to constantly; keep on fast disk |
| Class materials (course content) | `/srv/educlaw/data/classes/` | Read-only content teachers upload |
| React frontend | `/srv/educlaw/educlaw-web/dist/` | Served as static files by the server |
| Plugin | `/srv/educlaw/educlaw-plugin/` | Injected into every student session |

> **Rule of thumb:** anything under `/srv/` is static (deployed once, rarely changed).
> Anything under `/var/lib/educlaw/` is dynamic (changes while the server runs).
> Keep them on different disks if possible — students writing to their workspaces
> should not compete with the database for I/O.

---

## 2  Prerequisites

Install these on the host before anything else:

```bash
# Ubuntu / Debian
sudo apt update
sudo apt install -y \
    fuse-overlayfs \       # OverlayFS without root
    uidmap \               # newuidmap / newgidmap for user namespaces
    python3 \              # required by the plugin init.sh hook
    sqlite3                # optional – useful for debugging the DB

# Verify user namespaces are enabled
sysctl kernel.unprivileged_userns_clone   # must be 1
# If it shows 0:
echo 'kernel.unprivileged_userns_clone=1' | sudo tee /etc/sysctl.d/99-educlaw.conf
sudo sysctl -p /etc/sysctl.d/99-educlaw.conf
```

You also need **`claw`** (the Claude Code CLI) installed and on PATH:

```bash
# Check it exists
which claw || echo "claw not found — install it first"
```

---

## 3  Create the directory tree

Run this once on the production machine (as root or with sudo):

```bash
# ── Static / binary tree ──────────────────────────────────────────────────
sudo mkdir -p /srv/educlaw/data/classes     # course materials go here
sudo mkdir -p /srv/educlaw/educlaw-web/dist # React build goes here
sudo mkdir -p /srv/educlaw/educlaw-plugin   # plugin hooks go here

# ── Runtime / data tree ───────────────────────────────────────────────────
sudo mkdir -p /var/lib/educlaw/sessions     # student session overlays
sudo mkdir -p /var/lib/educlaw/students     # student upper layers (persisted work)

# ── cgroups v2 subtree for EduClaw ───────────────────────────────────────
# The server writes here at runtime; must be writable by the server process.
sudo mkdir -p /sys/fs/cgroup/educlaw

# ── Ownership: replace 'youruser' with the user that runs educlaw-server ──
sudo chown -R youruser:youruser /srv/educlaw
sudo chown -R youruser:youruser /var/lib/educlaw
sudo chown -R youruser:youruser /sys/fs/cgroup/educlaw
```

---

## 4  Copy the binaries

The binaries are compiled in your dev workspace. Copy them to `/srv/educlaw/`:

```bash
# From your dev machine (adjust path if different)
WORKSPACE=/home/pilser/Edu-Claw

sudo cp $WORKSPACE/educlaw-server/target/release/educlaw-server /srv/educlaw/
sudo cp $WORKSPACE/educlaw-mcp/target/release/educlaw-mcp       /srv/educlaw/

# Make them executable (they already are, but be explicit)
sudo chmod +x /srv/educlaw/educlaw-server
sudo chmod +x /srv/educlaw/educlaw-mcp

# Put educlaw-mcp on PATH so the plugin init.sh can find it
sudo ln -sf /srv/educlaw/educlaw-mcp /usr/local/bin/educlaw-mcp
```

---

## 5  Copy the plugin and frontend

```bash
WORKSPACE=/home/pilser/Edu-Claw

# Plugin hooks (init.sh, pre_tool_use.sh, shutdown.sh)
sudo cp -r $WORKSPACE/educlaw-plugin/. /srv/educlaw/educlaw-plugin/

# React frontend — build it first if you haven't already
cd $WORKSPACE/educlaw-web
bun install && bun run build

sudo cp -r $WORKSPACE/educlaw-web/dist/. /srv/educlaw/educlaw-web/dist/
```

---

## 6  Create the `.env` file

The server reads its config from a `.env` file in its **working directory**
(wherever you run it from) or from exported environment variables.

Copy the template from the repo and edit it:

```bash
cp /home/pilser/Edu-Claw/educlaw-server/.env /srv/educlaw/.env
```

Then open `/srv/educlaw/.env` and at minimum set:

| Variable | What to change |
|----------|---------------|
| `JWT_SECRET` | Replace with a new random value: `openssl rand -hex 32` |
| `ADMIN_SECRET` | Replace with a strong random password |
| `XAI_API_KEY` | Your xAI key is already there; update if rotated |
| `CLAW_BIN` | Path to your `claw` binary, e.g. `/usr/local/bin/claw` |

All other paths match the directory tree created in step 3 — no changes needed
unless you chose different locations.

---

## 7  Create the first admin account

The database is created automatically on first run. After starting the server
for the first time, create an admin account:

```bash
# Get an admin token using the ADMIN_SECRET from your .env
ADMIN_TOKEN=$(curl -sX POST http://localhost:8080/auth/token \
  -H 'Content-Type: application/json' \
  -d '{"secret":"your-admin-secret"}' | jq -r .token)

# Create your admin user
curl -sX POST http://localhost:8080/admin/students \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{
    "username": "admin",
    "password": "your-strong-password",
    "display_name": "Administrator",
    "role": "admin",
    "class_ids": []
  }'
```

---

## 8  Run the server

```bash
cd /srv/educlaw
./educlaw-server
```

The server starts on `0.0.0.0:8080` by default.
On first boot you will see:

```
INFO educlaw_server: database ready at sqlite:///var/lib/educlaw/educlaw.db
INFO educlaw_server: educlaw-server listening on 0.0.0.0:8080
```

To run it in the background as a systemd service, see **Section 10** below.

---

## 9  Add a class and enroll students

```bash
# Create a class directory (admin API)
curl -sX POST http://localhost:8080/admin/classes \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"class_id":"math101","title":"Introduction to Mathematics"}'

# This creates: /srv/educlaw/data/classes/math101/

# Upload course materials — just copy prepared .md + figure folders there:
cp -r ~/my-course-materials/. /srv/educlaw/data/classes/math101/

# Create a student account enrolled in the class
curl -sX POST http://localhost:8080/admin/students \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{
    "username": "alice",
    "password": "welcome123",
    "display_name": "Alice",
    "role": "student",
    "class_ids": ["math101"]
  }'
```

---

## 10  systemd service (optional but recommended)

Create `/etc/systemd/system/educlaw.service`:

```ini
[Unit]
Description=EduClaw Server
After=network.target

[Service]
Type=simple
User=youruser
WorkingDirectory=/srv/educlaw
EnvironmentFile=/srv/educlaw/.env
ExecStart=/srv/educlaw/educlaw-server
Restart=on-failure
RestartSec=5

# Give it access to user namespaces and cgroups
AmbientCapabilities=CAP_SYS_ADMIN
CapabilityBoundingSet=CAP_SYS_ADMIN

[Install]
WantedBy=multi-user.target
```

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now educlaw
sudo journalctl -u educlaw -f   # watch logs
```

---

## 11  Upgrading

When you compile new binaries:

```bash
sudo systemctl stop educlaw

sudo cp educlaw-server/target/release/educlaw-server /srv/educlaw/
sudo cp educlaw-mcp/target/release/educlaw-mcp       /srv/educlaw/
sudo ln -sf /srv/educlaw/educlaw-mcp /usr/local/bin/educlaw-mcp

sudo systemctl start educlaw
```

The database and all student workspace data in `/var/lib/educlaw/` are untouched.

---

## 12  Quick sanity-check after startup

```bash
# Health endpoint
curl http://localhost:8080/health
# → {"status":"ok"}

# Login check
curl -sX POST http://localhost:8080/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"username":"alice","password":"welcome123"}' | jq .
```

---

## 13  Full path reference

```
/srv/educlaw/
├── educlaw-server          ← main binary (HTTP + PTY + WebSocket)
├── educlaw-mcp             ← MCP binary (launched as child by claw)
├── .env                    ← configuration (JWT_SECRET, API keys, paths)
├── educlaw-plugin/         ← injected into every student session
│   └── hooks/
│       ├── init.sh         ← writes settings.json + registers MCP
│       ├── pre_tool_use.sh
│       └── shutdown.sh
├── educlaw-web/
│   └── dist/               ← built React app (served as static files)
└── data/
    └── classes/
        └── math101/        ← course materials (.md files + figure folders)
            ├── class.json
            ├── lecture_01.md
            └── lecture_01/
                └── fig1.png

/var/lib/educlaw/
├── educlaw.db              ← SQLite: students, sessions, audit log
├── educlaw-search.db       ← LanceDB vector search index
├── sessions/               ← per-session OverlayFS layers (cleaned on end)
│   └── {session_id}/
│       ├── lower/
│       ├── upper/
│       ├── work/
│       └── merged/
└── students/               ← persisted student work (survives session end)
    └── {student_id}/
        ├── upper/          ← student's saved files
        └── work/
```
