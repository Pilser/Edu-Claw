import type { LoginResponse, Session, FileNode, AssignmentProgress } from './types'

const json = (res: Response) => {
  if (!res.ok) return res.text().then(t => Promise.reject(new Error(t || `HTTP ${res.status}`)))
  return res.json()
}

export const api = {
  async login(username: string, password: string): Promise<LoginResponse> {
    return fetch('/auth/login', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ username, password }),
    }).then(json)
  },

  async createSession(
    studentId: string,
    token: string,
    course: string,
    assignment?: string,
  ): Promise<Session> {
    const body: Record<string, string> = { student_id: studentId, course }
    if (assignment) body.assignment = assignment
    return fetch('/sessions', {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${token}`,
      },
      body: JSON.stringify(body),
    }).then(json)
  },

  async endSession(sessionId: string, token: string): Promise<void> {
    await fetch(`/sessions/${sessionId}`, {
      method: 'DELETE',
      headers: { Authorization: `Bearer ${token}` },
    })
  },

  async listClassFiles(classId: string, token: string): Promise<FileNode[]> {
    return fetch(`/files/class/${classId}`, {
      headers: { Authorization: `Bearer ${token}` },
    }).then(json)
  },

  async listStudentFiles(sessionId: string, token: string): Promise<FileNode[]> {
    return fetch(`/files/student/${sessionId}`, {
      headers: { Authorization: `Bearer ${token}` },
    }).then(json)
  },

  async getFileContent(path: string, token: string): Promise<string> {
    return fetch(`/files/content?path=${encodeURIComponent(path)}`, {
      headers: { Authorization: `Bearer ${token}` },
    }).then(r => r.text())
  },

  async getProgress(
    sessionId: string,
    token: string,
  ): Promise<AssignmentProgress | null> {
    const res = await fetch(`/sessions/${sessionId}/progress`, {
      headers: { Authorization: `Bearer ${token}` },
    })
    if (res.status === 404) return null
    return json(res)
  },
}
