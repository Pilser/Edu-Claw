# EduClaw — Cohort + Subject Refactor Plan

## Status legend
- `[ ]` not started
- `[~]` in progress
- `[x]` done

> **Refactor complete.** All 15 steps implemented and both Rust crates (`educlaw-server`, `educlaw-mcp`) build release-clean. Frontend TypeScript reports no errors.

---

## Vocabulary

| Term | Meaning | Example values |
|---|---|---|
| **cohort** | The student's enrollment group. One cohort per student. | `primary-2b`, `grade-10a`, `cs-2024` |
| **subject** | A content area. Has materials on disk and assignments. | `mathematics`, `english`, `physics` |
| **assignment** | A task created by a teacher, linked to a subject. Agent reads it by ID. | `assignment-uuid` |

**Key rule:** A cohort is enrolled in multiple subjects. A student belongs to one cohort. When a session starts, the agent receives **only identity context (name, cohort_id)** — it discovers available subjects and assignments using its tools when the conversation calls for it. No pre-loading, no subject list in the prompt.

---

## Pre-refactor state (reference only — superseded)

### Database (`educlaw-server/src/db/schema.rs`)
- `students.class_ids` — JSON array `["id1","id2"]` (conflates cohort + subject)
- `sessions.course` — free text copied from create request
- `sessions.assignment` — free nullable text
- No `cohorts` table, no `subjects` table, no `assignments` table

### Rust models
- `db/students.rs` — `pub class_ids: String`
- `db/sessions.rs` — `pub course: String`, `pub assignment: Option<String>`
- `auth.rs` — `Claims.class_ids: Vec<String>`

### API routes
- `POST /sessions` body has `course: String`, `assignment: Option<String>`
- `GET /admin/students` returns `class_ids: Vec<String>`
- `POST /admin/students` accepts `class_ids: Option<Vec<String>>`
- `PUT /admin/students/:id` accepts `class_ids: Option<Vec<String>>`
- `POST /admin/classes` creates a directory (no DB row)
- `GET /admin/sessions` returns `course` + `assignment` fields
- `GET /files/class/{class_id}/tree` and `GET /files/class/{class_id}/read`

### MCP server (`educlaw-mcp/`)
- `config.rs` — `classes_root: String`, `EDUCLAW_CLASSES_ROOT` env var
- `config.rs` — `StudentInfo.class_ids: Vec<String>`
- `config.rs` — `assert_class_access(class_id)` enforces per-subject access
- `servers/class_files.rs` — tools: `list_class_files`, `read_class_file`, `search_class_content`, `index_class_file`; param name `class_id`
- `servers/student_context.rs` — description mentions "enrolled class IDs"

### Plugin (`educlaw-plugin/hooks/init.sh`)
- Reads `student.course`, `student.assignment`, `student.grade_level` from `EDUCLAW_STUDENT_JSON`
- System prompt says: *"AI tutor for {name} in the course "{course}""*

### Frontend (`educlaw-web/`)
- `api/types.ts` — `LoginResponse.class_ids: string[]`
- `api/client.ts` — `listClassFiles(classId, token)` → `/files/class/{classId}/tree`
- `api/client.ts` — `createSession(studentId, token, course)` sends `course` field
- `api/client.ts` — `adminCreateClass(token, classId, title)`
- `pages/AdminPage.tsx` — "Classes" tab with free-text class ID input; student create form has comma-separated `class_ids` text field
- `pages/SessionPage.tsx` — header badge shows `auth.class_ids[0]`; `FileTree` gets `classId={auth.class_ids[0]}`
- `components/FileTree.tsx` — prop `classId: string`; calls `api.listClassFiles(classId, token)`

### Env vars / setup
- `.env` — `EDUCLAW_CLASSES_ROOT=/srv/educlaw/data/classes`
- `educlaw-setup.sh` — creates `/srv/educlaw/data/classes` directory

---

## Target state

### Database — new tables and column renames

```sql
-- 1. New: cohorts
CREATE TABLE IF NOT EXISTS cohorts (
    id          TEXT PRIMARY KEY,              -- slug: "primary-2b"
    title       TEXT NOT NULL,                 -- human: "Primary 2B"
    subject_ids TEXT NOT NULL DEFAULT '[]',    -- JSON array of subject IDs
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

-- 2. New: subjects
CREATE TABLE IF NOT EXISTS subjects (
    id          TEXT PRIMARY KEY,              -- slug: "mathematics"
    title       TEXT NOT NULL,                 -- human: "Mathematics"
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

-- 3. New: assignments
CREATE TABLE IF NOT EXISTS assignments (
    id          TEXT PRIMARY KEY,              -- UUID
    subject_id  TEXT NOT NULL REFERENCES subjects(id),
    title       TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    due_date    TEXT,                          -- ISO8601 or NULL
    created_by  TEXT NOT NULL,                 -- teacher/admin student_id
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

-- 4. students — clean schema, no class_ids
CREATE TABLE IF NOT EXISTS students (
    id             TEXT PRIMARY KEY,
    username       TEXT NOT NULL UNIQUE,
    password_hash  TEXT NOT NULL,
    display_name   TEXT NOT NULL,
    role           TEXT NOT NULL DEFAULT 'student',
    cohort_id      TEXT NOT NULL DEFAULT '',
    uid_slot       INTEGER,
    created_at     TEXT NOT NULL DEFAULT (datetime('now')),
    last_seen      TEXT
);

-- 5. sessions — clean schema, no course/assignment columns
CREATE TABLE IF NOT EXISTS sessions (
    id              TEXT PRIMARY KEY,
    student_id      TEXT NOT NULL REFERENCES students(id),
    focused_subject TEXT,
    state           TEXT NOT NULL DEFAULT 'active',
    namespace_pid   INTEGER,
    overlay_mount   TEXT NOT NULL,
    last_input_at   TEXT,
    started_at      TEXT NOT NULL DEFAULT (datetime('now')),
    ended_at        TEXT
);
```

**Migration:** Stop the server, delete `/var/lib/educlaw/educlaw.db`, restart — schema is recreated clean on first run. No old columns, no dead fields.

### JWT Claims

```rust
// auth.rs — new Claims
pub cohort_id: String,
pub subject_ids: Vec<String>,   // resolved at login from cohort row
// removed: class_ids
```

### Student JSON blob (injected into every session as EDUCLAW_STUDENT_JSON)

Carries identity and cohort only. Subject list is **not** included — the agent discovers subjects via tools.

```json
{
  "student_id":    "uuid",
  "username":      "alice",
  "display_name":  "Alice",
  "role":          "student",
  "cohort_id":     "primary-2b",
  "session_id":    "uuid"
}
```

**Why subject_ids are excluded from the blob:** The `subject_ids` live in the JWT (for API auth enforcement) and are resolved by MCP tools at runtime. Putting them in the system prompt or the student_json blob would front-load the agent with a list it should discover contextually — breaking the tool-first design.

### System prompt (plugin init.sh) — new template

**Design principle:** The system prompt does NOT list subject names. The agent is a CLI coding agent (claw) with built-in shell execution tools and MCP tools. It discovers what subjects exist and what materials are available at runtime by using those tools — the same way a developer would `ls` a directory. This keeps the system prompt lean and the agent behaviour grounded in what actually exists on disk.

```
You are EduClaw, a dedicated AI tutor embedded in a student's terminal.

Student:  {display_name}
Cohort:   {cohort_id}
Session:  {session_id}

Your role: guide, never solve. Ask Socratic questions.
Encourage the student to reason step by step.
Give hints and scaffolding rather than direct answers.
Celebrate progress. Be patient with mistakes.

You have access to:
- The student's private workspace at: {workspace}
- Subject materials — use list_subject_files or shell tools to explore /subjects/
- Assignments — use list_assignments or get_assignment MCP tools

When the student asks about a topic, use your tools to find relevant
materials in the subject folders before answering. Do not guess what
subjects exist — check with tools first.
```

**What the agent does NOT receive in the system prompt:**
- The list of subject IDs — it discovers these by calling `list_subject_files` or running `ls` in the namespace
- Assignment content — it fetches on demand via `get_assignment(id)` or `list_assignments()`
- Grade level or course label — the cohort ID is sufficient context

**How the agent finds subjects:**
1. Student says "help me with my maths homework"
2. Agent runs `list_subject_files` or shell `ls /subjects/` inside the namespace
3. Sees `mathematics/` folder exists → reads materials → tutors in context
4. Agent calls `list_assignments(subject_id="mathematics")` to check pending work
5. Everything is driven by the conversation, not pre-loaded into the prompt

---

## Step-by-step implementation checklist

**✅ All 15 steps complete** — both Rust crates build clean, frontend has no TypeScript errors.

### Step 1 — DB schema [x]
**File:** `educlaw-server/src/db/schema.rs`

- [x] Add `CREATE TABLE IF NOT EXISTS cohorts (...)` 
- [x] Add `CREATE TABLE IF NOT EXISTS subjects (...)`
- [x] Add `CREATE TABLE IF NOT EXISTS assignments (...)`
- [x] Rewrite `students` table DDL: remove `class_ids`, add `cohort_id TEXT NOT NULL DEFAULT ''`
- [x] Rewrite `sessions` table DDL: remove `course` and `assignment`, add `focused_subject TEXT`
- [x] Add index: `CREATE INDEX IF NOT EXISTS idx_assignments_subject ON assignments(subject_id)`
- [x] **Before deploying:** stop server, delete `/var/lib/educlaw/educlaw.db`, restart to recreate clean

---

### Step 2 — Rust DB models [x]
**Files to create:**
- `educlaw-server/src/db/cohorts.rs` — `Cohort` struct + `insert_cohort`, `get_cohort`, `list_cohorts`, `update_cohort_subjects`
- `educlaw-server/src/db/subjects.rs` — `Subject` struct + `insert_subject`, `get_subject`, `list_subjects`
- `educlaw-server/src/db/assignments.rs` — `Assignment` struct + `insert_assignment`, `get_assignment`, `list_assignments_for_subject`, `list_assignments_for_student` (via cohort→subjects join)

**Files to modify:**
- `educlaw-server/src/db/students.rs`
  - Replace `pub class_ids: String` with `pub cohort_id: String` — no dead field kept
  - Add `cohort_id` to `insert_student` query
  - Add `update_student_cohort(pool, id, cohort_id)` function
- `educlaw-server/src/db/sessions.rs`
  - Replace `pub course: String` and `pub assignment: Option<String>` with `pub focused_subject: Option<String>` only
  - Update `INSERT` to bind `focused_subject` — no `course` or `assignment` binds

**File to modify:**
- `educlaw-server/src/db/mod.rs` — add `pub mod cohorts`, `pub mod subjects`, `pub mod assignments`

---

### Step 3 — Auth / JWT [x]
**File:** `educlaw-server/src/auth.rs`

- [x] Add `cohort_id: String` to `Claims` struct
- [x] Add `subject_ids: Vec<String>` to `Claims` struct  
- [x] Remove `class_ids: Vec<String>` from `Claims` struct
- [x] Update `sign_student_token(student_id, role, cohort_id, subject_ids)` signature
- [x] Update `sign_session_token(student_id, role, cohort_id, subject_ids, session_id)` signature
- [x] Update `sign_admin_token` — set `cohort_id: ""`, `subject_ids: vec![]`

---

### Step 4 — Login route [x]
**File:** `educlaw-server/src/api/auth_routes.rs`

- [x] After password check: fetch student's `cohort_id`
- [x] Query `cohorts` table to get `subject_ids` JSON → deserialize to `Vec<String>`
- [x] Call `sign_student_token(id, role, cohort_id, subject_ids)`
- [x] Login JSON response: replace `class_ids` with `cohort_id` + `subject_ids`

---

### Step 5 — Sessions API [x]
**File:** `educlaw-server/src/api/sessions.rs`

- [x] `CreateSessionRequest`: remove `course`, remove `assignment`
- [x] When building `student_json` blob: include `cohort_id` only — **do not include `subject_ids`**; the agent discovers subjects via tools
- [x] Fetch cohort at session creation to get `subject_ids` for the **JWT** (auth enforcement), not the student_json blob
- [x] `Session` DB insert: set `focused_subject: None`
- [x] `SessionView` struct: replace `course` + `assignment` with `focused_subject: Option<String>`
- [x] Add new endpoint `PUT /sessions/:id/focus` — student or agent signals which subject they are currently focusing on (updates `focused_subject` in DB and `student_json`)

---

### Step 6 — Admin API [x]
**File:** `educlaw-server/src/api/admin.rs`

**Students:**
- [x] `StudentView`: replace `class_ids: Vec<String>` with `cohort_id: String`
- [x] `CreateStudentBody`: replace `class_ids` with `cohort_id: Option<String>`
- [x] `UpdateStudentBody`: replace `class_ids` with `cohort_id: Option<String>`
- [x] `create_student` handler: write `cohort_id` to DB
- [x] `update_student` handler: call `update_student_cohort`

**Sessions:**
- [x] `SessionView`: replace `course` + `assignment` with `focused_subject: Option<String>`

**New Cohort endpoints:**
- [x] `GET  /admin/cohorts` — list all cohorts (id, title, subject_ids, student_count)
- [x] `POST /admin/cohorts` — create cohort (id slug, title, subject_ids array)
- [x] `PUT  /admin/cohorts/:id` — update title or subject_ids

**New Subject endpoints:**
- [x] `GET  /admin/subjects` — list all subjects (id, title, assignment_count)
- [x] `POST /admin/subjects` — create subject: insert DB row + `mkdir -p EDUCLAW_SUBJECTS_ROOT/{id}`
- [x] Rename `POST /admin/classes` → `POST /admin/subjects` (keep old route as alias returning 308 for compat)

**New Assignment endpoints:**
- [x] `GET  /admin/assignments?subject_id=` — list assignments for a subject
- [x] `POST /admin/assignments` — create assignment (subject_id, title, description, due_date)
- [x] `PUT  /admin/assignments/:id` — update assignment
- [x] `DELETE /admin/assignments/:id` — delete assignment
- [x] `GET  /assignments/:id` — **student-accessible** — returns assignment detail; JWT required; student must be enrolled in the assignment's subject via their cohort

---

### Step 7 — Files API [x]
**File:** `educlaw-server/src/api/files.rs`

- [x] Routes: `/files/class/{class_id}/...` → `/files/subject/{subject_id}/...`
- [x] Auth check: `claims.class_ids.contains(class_id)` → `claims.subject_ids.contains(subject_id)`
- [x] Root dir: `EDUCLAW_CLASSES_ROOT` → `EDUCLAW_SUBJECTS_ROOT`
- [x] Keep old `/files/class/...` routes as 308 redirects to new paths

---

### Step 8 — MCP config [x]
**File:** `educlaw-mcp/src/config.rs`

- [x] Rename field `classes_root` → `subjects_root`
- [x] Rename env var `EDUCLAW_CLASSES_ROOT` → `EDUCLAW_SUBJECTS_ROOT`
- [x] `StudentInfo` struct: remove `class_ids: Vec<String>`, add `cohort_id: String`
  - **Do not add `subject_ids`** to `StudentInfo` — the MCP server resolves allowed subjects from the session JWT, not from the student blob
- [x] Rename method `assert_class_access(class_id)` → `assert_subject_access(subject_id)`
  - This method now receives `subject_ids: &[String]` as a parameter (extracted from the JWT the MCP server receives), not from StudentInfo
  - Reason: StudentInfo is from `EDUCLAW_STUDENT_JSON` (identity blob); subject access is a JWT claim (auth concern)

**Two sources of truth in MCP:**
| Data | Source | Used for |
|---|---|---|
| `cohort_id`, `display_name` | `EDUCLAW_STUDENT_JSON` env var | Identity / system prompt |
| `subject_ids` | SessionJWT extracted from request header | Access control only |

---

### Step 9 — MCP servers [x]

**`educlaw-mcp/src/servers/class_files.rs` → rename to `subject_files.rs`:**
- [x] Rename file on disk
- [x] Rename all tool names: `list_class_files` → `list_subject_files`, `read_class_file` → `read_subject_file`, `search_class_content` → `search_subject_content`, `index_class_file` → `index_subject_file`
- [x] Rename param `class_id` → `subject_id` in all schemas and handler bodies
- [x] Call `assert_subject_access` instead of `assert_class_access`
- [x] Use `subjects_root` instead of `classes_root`

**New file `educlaw-mcp/src/servers/assignments.rs`:**
- [x] Tool: `get_assignment` — takes `assignment_id`, calls `GET /assignments/:id` on educlaw-server, returns JSON with title, description, due_date, subject_id
- [x] Tool: `list_assignments` — takes optional `subject_id`, fetches all assignments the student can see (i.e. subjects in their `subject_ids`)
- [x] Both tools use `EDUCLAW_SERVER_URL` + student JWT from config to call the server

**`educlaw-mcp/src/servers/student_context.rs`:**
- [x] Update tool description: "enrolled class IDs" → "cohort identity"
- [x] The JSON returned to the agent now includes `cohort_id` only (not `class_ids`, not `subject_ids`)
- [x] Reason: agent should use `list_subject_files` / `list_assignments` to discover content — not read a subject list from context and plan ahead of asking

**`educlaw-mcp/src/servers/mod.rs`:**
- [x] Add `pub mod subject_files` (replacing `pub mod class_files`)
- [x] Add `pub mod assignments`

**`educlaw-mcp/src/server.rs`:**
- [x] Replace all `class_files::` references with `subject_files::`
- [x] Add `assignments::` to `all_tools()` and `dispatch_tool()`
- [x] Rename `classes_root` → `subjects_root` in struct and `new()`
- [x] Dispatch arms: `"list_class_files" | "read_class_file" | ...` → subject/assignment equivalents

---

### Step 10 — Plugin hook [x]
**File:** `educlaw-plugin/hooks/init.sh`

- [x] Extract `display_name`, `cohort_id`, `session_id`, `workspace` from `EDUCLAW_STUDENT_JSON`
- [x] **Do NOT extract or embed `subject_ids`** — agent discovers subjects via tools
- [x] Build system prompt using new template (see Target State above) — lean, no subject list
- [x] Pass only `cohort_id` (not `subject_ids`, not `course`) to the MCP server env block
- [x] Remove reads of `student.course`, `student.assignment`, `student.grade_level` entirely

**Namespace filesystem layout the agent sees:**
```
/subjects/
  mathematics/    ← subject materials (read-only lower layer)
  english/
  physics/
{workspace}/     ← student's private writable upper layer
```
The agent can use shell tools (`ls /subjects/`) or MCP `list_subject_files(subject_id)` to discover what exists. Both paths work because claw has built-in shell execution.

---

### Step 11 — Frontend types [x]
**File:** `educlaw-web/src/api/types.ts`

- [x] `LoginResponse`: replace `class_ids: string[]` with `cohort_id: string` + `subject_ids: string[]`
- [x] Add new interfaces:
  ```typescript
  interface Cohort { id: string; title: string; subject_ids: string[] }
  interface Subject { id: string; title: string }
  interface Assignment {
    id: string; subject_id: string; title: string
    description: string; due_date: string | null; created_at: string
  }
  ```

---

### Step 12 — Frontend API client [x]
**File:** `educlaw-web/src/api/client.ts`

- [x] `createSession(studentId, token)` — remove `course` param; body has no `course`
- [x] `listClassFiles(classId, token)` → `listSubjectFiles(subjectId, token)` — calls `/files/subject/{subjectId}/tree`
- [x] `adminCreateClass(...)` → split:
  - `adminCreateSubject(token, subjectId, title)`
  - `adminCreateCohort(token, cohortId, title, subjectIds)`
- [x] Add `adminListCohorts(token): Promise<Cohort[]>` → `GET /admin/cohorts`
- [x] Add `adminListSubjects(token): Promise<Subject[]>` → `GET /admin/subjects`
- [x] Add `adminUpdateCohort(token, id, subjectIds)` → `PUT /admin/cohorts/:id`
- [x] Add `adminListAssignments(token, subjectId): Promise<Assignment[]>`
- [x] Add `adminCreateAssignment(token, body): Promise<{ id: string }>`
- [x] Add `adminUpdateAssignment(token, id, body)`
- [x] Add `adminDeleteAssignment(token, id)`
- [x] Add `getAssignment(token, assignmentId): Promise<Assignment>` — student-accessible

---

### Step 13 — AdminPage [x]
**File:** `educlaw-web/src/pages/AdminPage.tsx`

**Tabs:** Students | Cohorts | Subjects | Assignments | Live Sessions

**Students tab:**
- [x] Create form: replace comma-separated `class_ids` text input with `cohort_id` dropdown (loaded from `GET /admin/cohorts`)
- [x] Student table: replace `class_ids` column with `cohort_id`

**Cohorts tab (new):**
- [x] Create form: slug ID + human title + multi-select checkboxes of available subjects
- [x] Table: list cohorts, show subject_ids badges, student count

**Subjects tab (renamed from Classes):**
- [x] Create form: slug ID + human title (creates dir + DB row)
- [x] Table: list subjects with assignment count

**Assignments tab (new):**
- [x] Subject selector dropdown (to filter)
- [x] Assignment table: title, description preview, due date, actions (edit/delete)
- [x] Create form: subject dropdown, title, description (textarea), due date (date input)
- [x] Edit inline or in modal

**Live Sessions tab:**
- [x] `course` column → `focused_subject` (show `—` if null)

---

### Step 14 — SessionPage + FileTree [x]

**`educlaw-web/src/pages/SessionPage.tsx`:**
- [x] Header badge: `auth.class_ids[0] ?? 'open'` → `auth.cohort_id || auth.subject_ids[0] || 'open'`
- [x] `FileTree` prop: `classId={auth.class_ids[0] ?? ''}` → `subjectId={auth.subject_ids[0] ?? ''}`

**`educlaw-web/src/components/FileTree.tsx`:**
- [x] Prop: `classId: string` → `subjectId: string`
- [x] Tab label `'class'` → `'subject'`
- [x] API call: `api.listClassFiles(classId, token)` → `api.listSubjectFiles(subjectId, token)`

---

### Step 15 — Env vars + setup script [x]

**`.env` (educlaw-server + educlaw-mcp):**
- [x] `EDUCLAW_CLASSES_ROOT=/srv/educlaw/data/classes` → `EDUCLAW_SUBJECTS_ROOT=/srv/educlaw/data/subjects`

**`educlaw-setup.sh`:**
- [x] `cmd_dirs`: replace `/srv/educlaw/data/classes` with `/srv/educlaw/data/subjects`
- [x] Comment block at top: update path references

---

## Assignment flow (how teacher → agent works)

```
Teacher in AdminPage
  → Assignments tab
  → Select subject "mathematics"
  → Create assignment: title="Chapter 3 Exercise", description="Solve problems 1-5 from chapter 3", due_date="2026-04-20"
  → POST /admin/assignments → stored in DB with UUID

Agent in student session (has student's subject_ids in context)
  → at any time calls MCP tool: list_assignments (subject_id="mathematics")
  → server returns all assignments for that subject that the student's cohort can access
  → agent sees: [{ id: "uuid", title: "Chapter 3 Exercise", description: "...", due_date: "2026-04-20" }]
  → agent can call: get_assignment(assignment_id="uuid") for full detail
  → agent proactively tells student about pending assignments or answers questions about them
```

**Student visibility rule:** A student can see assignments for subjects that are in their cohort's `subject_ids`. No explicit per-student assignment table needed — cohort enrollment implies assignment access.

**Agent discovery flow (tool-first, not prompt-first):**
```
1. Session starts — agent receives only: display_name, cohort_id, workspace path, session_id
2. Student: "what should I be working on?"
3. Agent calls MCP: list_assignments() [no subject_id filter — returns all for cohort]
4. Agent finds: mathematics assignment due 2026-04-20
5. Agent: "You have a maths exercise due next week — want to start with that?"
6. Student: "yes, I'm stuck on quadratics"
7. Agent calls MCP: list_subject_files(subject_id="mathematics")
8. Agent reads chapter3.md, explains with context from actual course material
9. Student: "show me the exercise"
10. Agent calls MCP: get_assignment(assignment_id="uuid") → reads description
11. Agent guides student through the problems step by step
```

The agent never needs a pre-loaded subject list — it uses tools exactly as a developer would use `ls` and `cat`.

---

## Files touched summary

| File | Change type |
|---|---|
| `educlaw-server/src/db/schema.rs` | Add 3 tables + 2 ALTER columns |
| `educlaw-server/src/db/students.rs` | Add cohort_id field |
| `educlaw-server/src/db/sessions.rs` | Add focused_subject field |
| `educlaw-server/src/db/cohorts.rs` | **NEW** |
| `educlaw-server/src/db/subjects.rs` | **NEW** |
| `educlaw-server/src/db/assignments.rs` | **NEW** |
| `educlaw-server/src/db/mod.rs` | Add 3 new modules |
| `educlaw-server/src/auth.rs` | Replace class_ids with cohort_id + subject_ids |
| `educlaw-server/src/api/auth_routes.rs` | Login response update |
| `educlaw-server/src/api/sessions.rs` | CreateSessionRequest + student_json |
| `educlaw-server/src/api/admin.rs` | Student CRUD + new Cohort/Subject/Assignment endpoints |
| `educlaw-server/src/api/files.rs` | Rename class→subject in routes |
| `educlaw-server/src/api/mod.rs` | Wire new routes |
| `educlaw-mcp/src/config.rs` | Rename classes_root, update StudentInfo |
| `educlaw-mcp/src/servers/class_files.rs` → `subject_files.rs` | Rename file + all internals |
| `educlaw-mcp/src/servers/assignments.rs` | **NEW** |
| `educlaw-mcp/src/servers/student_context.rs` | Description update |
| `educlaw-mcp/src/servers/mod.rs` | Add subject_files + assignments |
| `educlaw-mcp/src/server.rs` | Dispatch + struct rename |
| `educlaw-plugin/hooks/init.sh` | New system prompt template |
| `educlaw-web/src/api/types.ts` | LoginResponse + new interfaces |
| `educlaw-web/src/api/client.ts` | Rename + new admin methods |
| `educlaw-web/src/pages/AdminPage.tsx` | New tabs + dropdowns |
| `educlaw-web/src/pages/SessionPage.tsx` | Badge + FileTree prop |
| `educlaw-web/src/components/FileTree.tsx` | classId → subjectId |
| `educlaw-server/.env` | EDUCLAW_SUBJECTS_ROOT |
| `educlaw-setup.sh` | Path rename |
