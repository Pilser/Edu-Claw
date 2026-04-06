import { useState, useEffect, type FormEvent } from 'react'
import { api, type AdminStudent, type AdminSession, type CreateStudentBody, type CreateAssignmentBody } from '../api/client'
import type { Cohort, Subject, Assignment } from '../api/types'

interface Props {
  token: string
  role: string
  displayName: string
  onLogout: () => void
}

type Tab = 'students' | 'cohorts' | 'subjects' | 'assignments' | 'sessions'

export default function AdminPage({ token, role, displayName, onLogout }: Props) {
  const [tab, setTab] = useState<Tab>('students')

  const tabCls = (t: Tab) =>
    `px-4 py-2 text-sm font-medium rounded-lg transition-colors ${tab === t ? 'bg-slate-800 text-white' : 'text-slate-600 hover:bg-slate-100'}`

  return (
    <div className="min-h-screen bg-slate-50">
      {/* Header */}
      <header className="bg-white border-b border-slate-200 px-6 py-4 flex items-center justify-between">
        <div className="flex items-center gap-3">
          <span className="text-2xl">🎓</span>
          <div>
            <h1 className="font-bold text-slate-800">EduClaw Admin</h1>
            <p className="text-xs text-slate-500 capitalize">{role} · {displayName}</p>
          </div>
        </div>
        <button onClick={onLogout} className="text-sm text-slate-500 hover:text-slate-700 transition-colors">
          Sign out
        </button>
      </header>

      {/* Tab bar */}
      <div className="px-6 py-4 flex gap-2 border-b border-slate-200 bg-white flex-wrap">
        <button className={tabCls('students')}    onClick={() => setTab('students')}>Students</button>
        <button className={tabCls('cohorts')}     onClick={() => setTab('cohorts')}>Cohorts</button>
        <button className={tabCls('subjects')}    onClick={() => setTab('subjects')}>Subjects</button>
        <button className={tabCls('assignments')} onClick={() => setTab('assignments')}>Assignments</button>
        <button className={tabCls('sessions')}    onClick={() => setTab('sessions')}>Live Sessions</button>
      </div>

      <main className="px-6 py-6 max-w-5xl mx-auto">
        {tab === 'students'    && <StudentsTab    token={token} role={role} />}
        {tab === 'cohorts'     && <CohortsTab     token={token} />}
        {tab === 'subjects'    && <SubjectsTab    token={token} />}
        {tab === 'assignments' && <AssignmentsTab token={token} />}
        {tab === 'sessions'    && <SessionsTab    token={token} />}
      </main>
    </div>
  )
}

// ── Students tab ────────────────────────────────────────────────────────────

function StudentsTab({ token, role }: { token: string; role: string }) {
  const [students, setStudents] = useState<AdminStudent[]>([])
  const [cohorts,  setCohorts]  = useState<Cohort[]>([])
  const [loading, setLoading]   = useState(true)
  const [error, setError]       = useState('')
  const [creating, setCreating] = useState(false)

  async function load() {
    setLoading(true)
    try {
      const [s, c] = await Promise.all([
        api.adminListStudents(token),
        api.adminListCohorts(token),
      ])
      setStudents(s)
      setCohorts(c)
    } catch (e) { setError(e instanceof Error ? e.message : 'Failed to load') }
    finally { setLoading(false) }
  }

  useEffect(() => { load() }, [])

  async function handleCreate(e: FormEvent<HTMLFormElement>) {
    e.preventDefault()
    const form = e.currentTarget
    const get  = (name: string) => (form.elements.namedItem(name) as HTMLInputElement | HTMLSelectElement).value.trim()
    const body: CreateStudentBody = {
      username:     get('username'),
      password:     get('password'),
      display_name: get('display_name'),
      role:         get('role') as CreateStudentBody['role'],
      cohort_id:    get('cohort_id'),
    }
    try {
      await api.adminCreateStudent(token, body)
      form.reset()
      setCreating(false)
      load()
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Create failed')
    }
  }

  const inputCls = 'px-3 py-2 rounded-lg border border-slate-300 focus:outline-none focus:ring-2 focus:ring-blue-500 text-sm text-slate-800 placeholder-slate-400'

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h2 className="text-lg font-semibold text-slate-700">Students & Staff</h2>
        {role === 'admin' && (
          <button
            onClick={() => setCreating(c => !c)}
            className="px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white text-sm font-medium rounded-lg transition-colors"
          >
            {creating ? 'Cancel' : '+ New account'}
          </button>
        )}
      </div>

      {creating && (
        <form onSubmit={handleCreate} className="bg-white rounded-xl border border-slate-200 p-5 grid grid-cols-2 gap-3">
          <div className="col-span-2 font-medium text-sm text-slate-700 mb-1">Create account</div>
          <input name="username"     required placeholder="Username"     className={inputCls} />
          <input name="password"     required placeholder="Password" type="password" className={inputCls} />
          <input name="display_name" required placeholder="Display name"  className={inputCls} />
          <select name="role" className={inputCls}>
            <option value="student">Student</option>
            <option value="teacher">Teacher</option>
            <option value="admin">Admin</option>
          </select>
          <select name="cohort_id" required className={`${inputCls} col-span-2`}>
            <option value="">— Select cohort —</option>
            {cohorts.map(c => (
              <option key={c.id} value={c.id}>{c.title} ({c.id})</option>
            ))}
          </select>
          {error && <p className="col-span-2 text-red-600 text-sm">⚠ {error}</p>}
          <button type="submit" className="col-span-2 py-2 bg-slate-800 hover:bg-slate-700 text-white text-sm font-medium rounded-lg">
            Create
          </button>
        </form>
      )}

      {loading ? (
        <p className="text-slate-500 text-sm">Loading…</p>
      ) : (
        <div className="bg-white rounded-xl border border-slate-200 overflow-hidden">
          <table className="w-full text-sm">
            <thead className="bg-slate-50 text-slate-500 text-xs uppercase">
              <tr>
                {['Username', 'Name', 'Role', 'Cohort', 'Last seen', 'Status'].map(h => (
                  <th key={h} className="px-4 py-3 text-left font-medium">{h}</th>
                ))}
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-100">
              {students.map(s => (
                <tr key={s.id} className="hover:bg-slate-50">
                  <td className="px-4 py-3 font-mono text-slate-700">{s.username}</td>
                  <td className="px-4 py-3 text-slate-800">{s.display_name}</td>
                  <td className="px-4 py-3">
                    <span className={`px-2 py-0.5 rounded-full text-xs font-medium ${s.role === 'admin' ? 'bg-red-100 text-red-700' : s.role === 'teacher' ? 'bg-purple-100 text-purple-700' : 'bg-blue-100 text-blue-700'}`}>
                      {s.role}
                    </span>
                  </td>
                  <td className="px-4 py-3 text-slate-500 text-xs">{s.cohort_id || '—'}</td>
                  <td className="px-4 py-3 text-slate-400 text-xs">{s.last_seen ? new Date(s.last_seen).toLocaleString() : '—'}</td>
                  <td className="px-4 py-3">
                    {s.has_active_session
                      ? <span className="flex items-center gap-1 text-green-600 text-xs"><span className="w-2 h-2 bg-green-500 rounded-full inline-block" />Active</span>
                      : <span className="text-slate-400 text-xs">Offline</span>}
                  </td>
                </tr>
              ))}
              {students.length === 0 && (
                <tr><td colSpan={6} className="px-4 py-6 text-center text-slate-400 text-sm">No accounts yet — create the first one above.</td></tr>
              )}
            </tbody>
          </table>
        </div>
      )}
    </div>
  )
}

// ── Cohorts tab ──────────────────────────────────────────────────────────────

function CohortsTab({ token }: { token: string }) {
  const [cohorts,    setCohorts]    = useState<Cohort[]>([])
  const [subjects,   setSubjects]   = useState<Subject[]>([])
  const [loading,    setLoading]    = useState(true)
  const [creating,   setCreating]   = useState(false)
  const [title,      setTitle]      = useState('')
  const [subjectIds, setSubjectIds] = useState('')
  const [error,      setError]      = useState('')

  async function load() {
    setLoading(true)
    try {
      const [c, s] = await Promise.all([
        api.adminListCohorts(token),
        api.adminListSubjects(token),
      ])
      setCohorts(c)
      setSubjects(s)
    } catch (e) { setError(e instanceof Error ? e.message : 'Failed to load') }
    finally { setLoading(false) }
  }

  useEffect(() => { load() }, [])

  async function handleCreate(e: FormEvent) {
    e.preventDefault()
    const ids = subjectIds.split(',').map(s => s.trim()).filter(Boolean)
    try {
      await api.adminCreateCohort(token, title.trim(), ids)
      setTitle('')
      setSubjectIds('')
      setCreating(false)
      load()
    } catch (e) { setError(e instanceof Error ? e.message : 'Create failed') }
  }

  const inputCls = 'px-3 py-2 rounded-lg border border-slate-300 focus:outline-none focus:ring-2 focus:ring-blue-500 text-sm text-slate-800 placeholder-slate-400'

  function subjectLabel(id: string) {
    return subjects.find(s => s.id === id)?.title ?? id
  }

  function parsedSubjectIds(raw: string): string[] {
    try { return JSON.parse(raw) } catch { return [] }
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h2 className="text-lg font-semibold text-slate-700">Cohorts</h2>
        <button
          onClick={() => setCreating(c => !c)}
          className="px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white text-sm font-medium rounded-lg transition-colors"
        >
          {creating ? 'Cancel' : '+ New cohort'}
        </button>
      </div>

      {creating && (
        <form onSubmit={handleCreate} className="bg-white rounded-xl border border-slate-200 p-5 space-y-3 max-w-md">
          <div className="font-medium text-sm text-slate-700">Create cohort</div>
          <div>
            <label className="block text-xs text-slate-500 mb-1">Title</label>
            <input value={title} onChange={e => setTitle(e.target.value)} required placeholder="e.g. Class 2025" className={`${inputCls} w-full`} />
          </div>
          <div>
            <label className="block text-xs text-slate-500 mb-1">Subject IDs (comma-separated, optional)</label>
            <input value={subjectIds} onChange={e => setSubjectIds(e.target.value)} placeholder="e.g. math, science" className={`${inputCls} w-full`} />
          </div>
          {error && <p className="text-red-600 text-sm">⚠ {error}</p>}
          <button type="submit" className="w-full py-2 bg-slate-800 hover:bg-slate-700 text-white text-sm font-medium rounded-lg">Create</button>
        </form>
      )}

      {loading ? (
        <p className="text-slate-500 text-sm">Loading…</p>
      ) : (
        <div className="bg-white rounded-xl border border-slate-200 overflow-hidden">
          <table className="w-full text-sm">
            <thead className="bg-slate-50 text-slate-500 text-xs uppercase">
              <tr>
                {['ID', 'Title', 'Subjects', 'Created'].map(h => (
                  <th key={h} className="px-4 py-3 text-left font-medium">{h}</th>
                ))}
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-100">
              {cohorts.map(c => (
                <tr key={c.id} className="hover:bg-slate-50">
                  <td className="px-4 py-3 font-mono text-xs text-slate-600">{c.id}</td>
                  <td className="px-4 py-3 text-slate-800">{c.title}</td>
                  <td className="px-4 py-3 text-slate-500 text-xs">
                    {parsedSubjectIds(c.subject_ids).map(id => (
                      <span key={id} className="inline-block bg-blue-50 text-blue-700 rounded px-1.5 py-0.5 mr-1 mb-1">{subjectLabel(id)}</span>
                    ))}
                    {parsedSubjectIds(c.subject_ids).length === 0 && '—'}
                  </td>
                  <td className="px-4 py-3 text-slate-400 text-xs">{new Date(c.created_at).toLocaleDateString()}</td>
                </tr>
              ))}
              {cohorts.length === 0 && (
                <tr><td colSpan={4} className="px-4 py-6 text-center text-slate-400 text-sm">No cohorts yet.</td></tr>
              )}
            </tbody>
          </table>
        </div>
      )}
    </div>
  )
}

// ── Subjects tab ──────────────────────────────────────────────────────────────

function SubjectsTab({ token }: { token: string }) {
  const [subjects, setSubjects] = useState<Subject[]>([])
  const [loading,  setLoading]  = useState(true)
  const [creating, setCreating] = useState(false)
  const [title,    setTitle]    = useState('')
  const [error,    setError]    = useState('')

  async function load() {
    setLoading(true)
    try { setSubjects(await api.adminListSubjects(token)) }
    catch (e) { setError(e instanceof Error ? e.message : 'Failed to load') }
    finally { setLoading(false) }
  }

  useEffect(() => { load() }, [])

  async function handleCreate(e: FormEvent) {
    e.preventDefault()
    try {
      await api.adminCreateSubject(token, title.trim())
      setTitle('')
      setCreating(false)
      load()
    } catch (e) { setError(e instanceof Error ? e.message : 'Create failed') }
  }

  const inputCls = 'px-3 py-2 rounded-lg border border-slate-300 focus:outline-none focus:ring-2 focus:ring-blue-500 text-sm text-slate-800 placeholder-slate-400'

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h2 className="text-lg font-semibold text-slate-700">Subjects</h2>
        <button
          onClick={() => setCreating(c => !c)}
          className="px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white text-sm font-medium rounded-lg transition-colors"
        >
          {creating ? 'Cancel' : '+ New subject'}
        </button>
      </div>

      {creating && (
        <form onSubmit={handleCreate} className="bg-white rounded-xl border border-slate-200 p-5 space-y-3 max-w-md">
          <div className="font-medium text-sm text-slate-700">Create subject</div>
          <div>
            <label className="block text-xs text-slate-500 mb-1">Title</label>
            <input value={title} onChange={e => setTitle(e.target.value)} required placeholder="e.g. Introduction to Mathematics" className={`${inputCls} w-full`} />
          </div>
          {error && <p className="text-red-600 text-sm">⚠ {error}</p>}
          <button type="submit" className="w-full py-2 bg-slate-800 hover:bg-slate-700 text-white text-sm font-medium rounded-lg">Create</button>
        </form>
      )}

      {loading ? (
        <p className="text-slate-500 text-sm">Loading…</p>
      ) : (
        <div className="bg-white rounded-xl border border-slate-200 overflow-hidden">
          <table className="w-full text-sm">
            <thead className="bg-slate-50 text-slate-500 text-xs uppercase">
              <tr>
                {['ID', 'Title', 'Created'].map(h => (
                  <th key={h} className="px-4 py-3 text-left font-medium">{h}</th>
                ))}
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-100">
              {subjects.map(s => (
                <tr key={s.id} className="hover:bg-slate-50">
                  <td className="px-4 py-3 font-mono text-xs text-slate-600">{s.id}</td>
                  <td className="px-4 py-3 text-slate-800">{s.title}</td>
                  <td className="px-4 py-3 text-slate-400 text-xs">{new Date(s.created_at).toLocaleDateString()}</td>
                </tr>
              ))}
              {subjects.length === 0 && (
                <tr><td colSpan={3} className="px-4 py-6 text-center text-slate-400 text-sm">No subjects yet.</td></tr>
              )}
            </tbody>
          </table>
        </div>
      )}

      <div className="bg-slate-100 rounded-xl p-4 text-xs text-slate-600 max-w-md">
        <p className="font-medium mb-1">After creating a subject:</p>
        <p>Copy course materials (.md files + image folders) into:</p>
        <code className="block mt-1 bg-white rounded px-2 py-1 font-mono">/srv/educlaw/data/subjects/&lt;subject_id&gt;/</code>
      </div>
    </div>
  )
}

// ── Assignments tab ───────────────────────────────────────────────────────────

function AssignmentsTab({ token }: { token: string }) {
  const [assignments, setAssignments] = useState<Assignment[]>([])
  const [subjects,    setSubjects]    = useState<Subject[]>([])
  const [filterSubj,  setFilterSubj]  = useState('')
  const [loading,     setLoading]     = useState(true)
  const [creating,    setCreating]    = useState(false)
  const [form, setForm] = useState<CreateAssignmentBody>({ subject_id: '', title: '', description: '', due_date: '' })
  const [error, setError] = useState('')

  async function load(subjectId?: string) {
    setLoading(true)
    try {
      const [a, s] = await Promise.all([
        api.adminListAssignments(token, subjectId),
        api.adminListSubjects(token),
      ])
      setAssignments(a)
      setSubjects(s)
    } catch (e) { setError(e instanceof Error ? e.message : 'Failed to load') }
    finally { setLoading(false) }
  }

  useEffect(() => { load() }, [])

  function handleFilterChange(subjectId: string) {
    setFilterSubj(subjectId)
    load(subjectId || undefined)
  }

  async function handleCreate(e: FormEvent) {
    e.preventDefault()
    try {
      await api.adminCreateAssignment(token, {
        subject_id:  form.subject_id,
        title:       form.title,
        description: form.description || undefined,
        due_date:    form.due_date    || undefined,
      })
      setForm({ subject_id: '', title: '', description: '', due_date: '' })
      setCreating(false)
      load(filterSubj || undefined)
    } catch (e) { setError(e instanceof Error ? e.message : 'Create failed') }
  }

  const inputCls = 'px-3 py-2 rounded-lg border border-slate-300 focus:outline-none focus:ring-2 focus:ring-blue-500 text-sm text-slate-800 placeholder-slate-400'

  function subjectTitle(id: string) {
    return subjects.find(s => s.id === id)?.title ?? id
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h2 className="text-lg font-semibold text-slate-700">Assignments</h2>
        <button
          onClick={() => setCreating(c => !c)}
          className="px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white text-sm font-medium rounded-lg transition-colors"
        >
          {creating ? 'Cancel' : '+ New assignment'}
        </button>
      </div>

      <div className="flex items-center gap-3">
        <label className="text-xs text-slate-500">Filter by subject:</label>
        <select
          value={filterSubj}
          onChange={e => handleFilterChange(e.target.value)}
          className={`${inputCls} min-w-[180px]`}
        >
          <option value="">All subjects</option>
          {subjects.map(s => (
            <option key={s.id} value={s.id}>{s.title}</option>
          ))}
        </select>
      </div>

      {creating && (
        <form onSubmit={handleCreate} className="bg-white rounded-xl border border-slate-200 p-5 space-y-3 max-w-lg">
          <div className="font-medium text-sm text-slate-700">Create assignment</div>
          <div>
            <label className="block text-xs text-slate-500 mb-1">Subject</label>
            <select required value={form.subject_id} onChange={e => setForm(f => ({ ...f, subject_id: e.target.value }))} className={`${inputCls} w-full`}>
              <option value="">— Select subject —</option>
              {subjects.map(s => (
                <option key={s.id} value={s.id}>{s.title} ({s.id})</option>
              ))}
            </select>
          </div>
          <div>
            <label className="block text-xs text-slate-500 mb-1">Title</label>
            <input required value={form.title} onChange={e => setForm(f => ({ ...f, title: e.target.value }))} placeholder="e.g. Chapter 3 Quiz" className={`${inputCls} w-full`} />
          </div>
          <div>
            <label className="block text-xs text-slate-500 mb-1">Description (optional)</label>
            <textarea rows={3} value={form.description} onChange={e => setForm(f => ({ ...f, description: e.target.value }))} placeholder="Instructions for the student…" className={`${inputCls} w-full resize-none`} />
          </div>
          <div>
            <label className="block text-xs text-slate-500 mb-1">Due date (optional)</label>
            <input type="date" value={form.due_date} onChange={e => setForm(f => ({ ...f, due_date: e.target.value }))} className={`${inputCls} w-full`} />
          </div>
          {error && <p className="text-red-600 text-sm">⚠ {error}</p>}
          <button type="submit" className="w-full py-2 bg-slate-800 hover:bg-slate-700 text-white text-sm font-medium rounded-lg">Create assignment</button>
        </form>
      )}

      {loading ? (
        <p className="text-slate-500 text-sm">Loading…</p>
      ) : (
        <div className="bg-white rounded-xl border border-slate-200 overflow-hidden">
          <table className="w-full text-sm">
            <thead className="bg-slate-50 text-slate-500 text-xs uppercase">
              <tr>
                {['Title', 'Subject', 'Due date', 'Created'].map(h => (
                  <th key={h} className="px-4 py-3 text-left font-medium">{h}</th>
                ))}
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-100">
              {assignments.map(a => (
                <tr key={a.id} className="hover:bg-slate-50">
                  <td className="px-4 py-3 text-slate-800 font-medium">{a.title}</td>
                  <td className="px-4 py-3 text-slate-500 text-xs">{subjectTitle(a.subject_id)}</td>
                  <td className="px-4 py-3 text-slate-400 text-xs">{a.due_date ? new Date(a.due_date).toLocaleDateString() : '—'}</td>
                  <td className="px-4 py-3 text-slate-400 text-xs">{new Date(a.created_at).toLocaleDateString()}</td>
                </tr>
              ))}
              {assignments.length === 0 && (
                <tr><td colSpan={4} className="px-4 py-6 text-center text-slate-400 text-sm">No assignments yet.</td></tr>
              )}
            </tbody>
          </table>
        </div>
      )}
    </div>
  )
}

// ── Sessions tab ──────────────────────────────────────────────────────────────

function SessionsTab({ token }: { token: string }) {
  const [sessions, setSessions] = useState<AdminSession[]>([])
  const [loading, setLoading]   = useState(true)

  async function load() {
    setLoading(true)
    try { setSessions(await api.adminListSessions(token)) }
    finally { setLoading(false) }
  }

  useEffect(() => { load() }, [])

  async function terminate(id: string) {
    if (!confirm('Terminate this session?')) return
    await api.adminTerminateSession(token, id)
    load()
  }

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h2 className="text-lg font-semibold text-slate-700">Live Sessions</h2>
        <button onClick={load} className="text-sm text-slate-500 hover:text-slate-700">↻ Refresh</button>
      </div>

      {loading ? (
        <p className="text-slate-500 text-sm">Loading…</p>
      ) : (
        <div className="bg-white rounded-xl border border-slate-200 overflow-hidden">
          <table className="w-full text-sm">
            <thead className="bg-slate-50 text-slate-500 text-xs uppercase">
              <tr>
                {['Student ID', 'State', 'Idle', 'Started', ''].map(h => (
                  <th key={h} className="px-4 py-3 text-left font-medium">{h}</th>
                ))}
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-100">
              {sessions.map(s => (
                <tr key={s.id} className="hover:bg-slate-50">
                  <td className="px-4 py-3 font-mono text-xs text-slate-600">{s.student_id.slice(0, 8)}…</td>
                  <td className="px-4 py-3 text-slate-800">{s.state}</td>
                  <td className="px-4 py-3 text-slate-500">
                    {s.idle_secs != null ? `${Math.floor(s.idle_secs / 60)}m ${s.idle_secs % 60}s` : '—'}
                  </td>
                  <td className="px-4 py-3 text-slate-400 text-xs">{new Date(s.started_at).toLocaleString()}</td>
                  <td className="px-4 py-3">
                    <button onClick={() => terminate(s.id)} className="text-xs text-red-500 hover:text-red-700">
                      Terminate
                    </button>
                  </td>
                </tr>
              ))}
              {sessions.length === 0 && (
                <tr><td colSpan={5} className="px-4 py-6 text-center text-slate-400 text-sm">No active sessions.</td></tr>
              )}
            </tbody>
          </table>
        </div>
      )}
    </div>
  )
}
