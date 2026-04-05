import { useState } from 'react'
import LoginPage from './pages/LoginPage'
import SessionPage from './pages/SessionPage'
import type { LoginResponse, Session } from './api/types'

interface SessionState {
  auth: LoginResponse
  session: Session
  course: string
}

export default function App() {
  const [active, setActive] = useState<SessionState | null>(null)

  function handleLogin(auth: LoginResponse, session: Session, course: string) {
    setActive({ auth, session, course })
  }

  function handleEnd() {
    setActive(null)
  }

  if (!active) {
    return <LoginPage onLogin={handleLogin} />
  }

  return (
    <SessionPage
      auth={active.auth}
      session={active.session}
      course={active.course}
      onEnd={handleEnd}
    />
  )
}
