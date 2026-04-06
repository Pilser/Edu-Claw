import type { LoginResponse, Session, FileNode, AssignmentProgress, Cohort, Subject, Assignment } from './types'

const BASE = ''   // same origin

const json = (res: Response) => {
  if (!res.ok) return res.text().then(t => Promise.reject(new Error(t || `HTTP ${res.status}`)))
  return res.json()
}

export const api = {
  // ── Auth ────────────────────────────────────────────────────────────────

  async login(username: string, password: string): Promise<LoginResponse> {
    return fetch(`${BASE}/auth/login`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ username, password }),
    }).then(json)
  },

  /** Bootstrap token — uses ADMIN_SECRET, not username/password */
  async bootstrapToken(secret: string): Promise<{ token: string }> {
    return fetch(`${BASE}/auth/token`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ secret }),
    }).then(json)
  },

  // ── Sessions ─────────────────────────────────────────────────────────────

  async createSession(studentId: string, token: string): Promise<Session> {
    return fetch(`${BASE}/sessions`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${token}`,
      },
      body: JSON.stringify({ student_id: studentId }),
    }).then(json)
  },

  async endSession(sessionId: string, token: string): Promise<void> {
    await fetch(`${BASE}/sessions/${sessionId}`, {
      method: 'DELETE',
      headers: { Authorization: `Bearer ${token}` },
    })
  },

  // ── Files ────────────────────────────────────────────────────────────────

  async listSubjectFiles(subjectId: string, token: string): Promise<FileNode[]> {
    return fetch(`${BASE}/files/subject/${subjectId}/tree`, {
      headers: { Authorization: `Bearer ${token}` },
    }).then(json)
  },

  async listStudentFiles(sessionId: string, token: string): Promise<FileNode[]> {
    return fetch(`${BASE}/files/student/${sessionId}/tree`, {
      headers: { Authorization: `Bearer ${token}` },
    }).then(json)
  },

  async getProgress(sessionId: string, token: string): Promise<AssignmentProgress | null> {
    const res = await fetch(`${BASE}/sessions/${sessionId}/progress`, {
      headers: { Authorization: `Bearer ${token}` },
    })
    if (res.status === 404) return null
    return json(res)
  },

  // ── Admin — Students ─────────────────────────────────────────────────────

  async adminListStudents(token: string): Promise<AdminStudent[]> {
    return fetch(`${BASE}/admin/students`, {
      headers: { Authorization: `Bearer ${token}` },
    }).then(json)
  },

  async adminCreateStudent(token: string, body: CreateStudentBody): Promise<{ id: string }> {
    return fetch(`${BASE}/admin/students`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
      body: JSON.stringify(body),
    }).then(json)
  },

  async adminUpdateStudent(token: string, id: string, body: UpdateStudentBody): Promise<void> {
    await fetch(`${BASE}/admin/students/${id}`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
      body: JSON.stringify(body),
    }).then(json)
  },

  async adminDeleteStudent(token: string, id: string): Promise<void> {
    await fetch(`${BASE}/admin/students/${id}`, {
      method: 'DELETE',
      headers: { Authorization: `Bearer ${token}` },
    })
  },

  // ── Admin — Sessions ─────────────────────────────────────────────────────

  async adminListSessions(token: string): Promise<AdminSession[]> {
    return fetch(`${BASE}/admin/sessions`, {
      headers: { Authorization: `Bearer ${token}` },
    }).then(json)
  },

  async adminTerminateSession(token: string, sessionId: string): Promise<void> {
    await fetch(`${BASE}/admin/sessions/${sessionId}`, {
      method: 'DELETE',
      headers: { Authorization: `Bearer ${token}` },
    })
  },

  // ── Admin — Cohorts ──────────────────────────────────────────────────────

  async adminListCohorts(token: string): Promise<Cohort[]> {
    return fetch(`${BASE}/admin/cohorts`, {
      headers: { Authorization: `Bearer ${token}` },
    }).then(json)
  },

  async adminCreateCohort(token: string, slug: string, title: string): Promise<{ id: string }> {
    return fetch(`${BASE}/admin/cohorts`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
      body: JSON.stringify({ slug, title }),
    }).then(json)
  },

  async adminUpdateCohort(token: string, id: string, newSlug: string | undefined, title: string): Promise<{ id: string }> {
    return fetch(`${BASE}/admin/cohorts/${id}`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
      body: JSON.stringify({ new_slug: newSlug !== id ? newSlug : undefined, title }),
    }).then(json)
  },

  async adminDeleteCohort(token: string, id: string): Promise<void> {
    await fetch(`${BASE}/admin/cohorts/${id}`, {
      method: 'DELETE',
      headers: { Authorization: `Bearer ${token}` },
    })
  },

  // ── Admin — Subjects ─────────────────────────────────────────────────────

  async adminListSubjects(token: string, cohortId?: string): Promise<Subject[]> {
    const url = cohortId
      ? `${BASE}/admin/subjects?cohort_id=${encodeURIComponent(cohortId)}`
      : `${BASE}/admin/subjects`
    return fetch(url, {
      headers: { Authorization: `Bearer ${token}` },
    }).then(json)
  },

  async adminCreateSubject(token: string, cohortId: string, slug: string, title: string): Promise<{ id: string; slug: string }> {
    return fetch(`${BASE}/admin/subjects`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
      body: JSON.stringify({ cohort_id: cohortId, slug, title }),
    }).then(json)
  },

  async adminUpdateSubject(token: string, id: string, newSlug: string | undefined, title: string): Promise<{ id: string; slug: string }> {
    return fetch(`${BASE}/admin/subjects/${id}`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
      body: JSON.stringify({ new_slug: newSlug, title }),
    }).then(json)
  },

  async adminDeleteSubject(token: string, id: string): Promise<void> {
    await fetch(`${BASE}/admin/subjects/${id}`, {
      method: 'DELETE',
      headers: { Authorization: `Bearer ${token}` },
    })
  },

  // ── Admin — Assignments ──────────────────────────────────────────────────

  async adminListAssignments(token: string, subjectId?: string): Promise<Assignment[]> {
    const url = subjectId
      ? `${BASE}/admin/assignments?subject_id=${encodeURIComponent(subjectId)}`
      : `${BASE}/admin/assignments`
    return fetch(url, {
      headers: { Authorization: `Bearer ${token}` },
    }).then(json)
  },

  async adminCreateAssignment(token: string, body: CreateAssignmentBody): Promise<{ id: string }> {
    return fetch(`${BASE}/admin/assignments`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
      body: JSON.stringify(body),
    }).then(json)
  },
}

// ── Admin types (only used by AdminPage) ─────────────────────────────────

export interface AdminStudent {
  id: string
  username: string
  display_name: string
  role: string
  cohort_id: string
  last_seen: string | null
  has_active_session: boolean
}

export interface AdminSession {
  id: string
  student_id: string
  state: string
  idle_secs: number | null
  started_at: string
}

export interface CreateStudentBody {
  username: string
  password: string
  display_name: string
  role: 'student' | 'teacher' | 'admin'
  cohort_id: string
}

export interface UpdateStudentBody {
  display_name?: string
  cohort_id?: string
  new_password?: string
}

export interface CreateAssignmentBody {
  subject_id: string
  title: string
  description?: string
  due_date?: string
}

