import { useState } from 'react'
import LoginPage from './pages/LoginPage'
import SessionPage from './pages/SessionPage'
import AdminPage from './pages/AdminPage'
import { api } from './api/client'
import type { LoginResponse, Session } from './api/types'

type View =
  | { kind: 'login'; error?: string }
  | { kind: 'creating' }
  | { kind: 'session'; auth: LoginResponse; session: Session }
  | { kind: 'admin'; token: string; role: string; displayName: string }

export default function App() {
  const [view, setView] = useState<View>({ kind: 'login' })

  async function handleStudentLogin(auth: LoginResponse) {
    setView({ kind: 'creating' })
    try {
      const session = await api.createSession(auth.student_id, auth.token)
      setView({ kind: 'session', auth, session })
    } catch (err) {
      setView({ kind: 'login', error: String(err) })
    }
  }

  return (
    <>
      {view.kind === 'login' && (
        <LoginPage
          onLogin={handleStudentLogin}
          onStaffLogin={(token, role, displayName) => setView({ kind: 'admin', token, role, displayName })}
          externalError={view.error}
        />
      )}
      {view.kind === 'creating' && (
        <div className="flex h-screen items-center justify-center bg-slate-900 text-slate-300 text-sm">
          Starting session…
        </div>
      )}
      {view.kind === 'session' && (
        <SessionPage
          auth={view.auth}
          session={view.session}
          onEnd={() => setView({ kind: 'login' })}
        />
      )}
      {view.kind === 'admin' && (
        <AdminPage
          token={view.token}
          role={view.role}
          displayName={view.displayName}
          onLogout={() => setView({ kind: 'login' })}
        />
      )}
    </>
  )
}


