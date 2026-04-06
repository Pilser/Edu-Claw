import { useState, type FormEvent } from 'react'
import { api } from '../api/client'
import type { LoginResponse } from '../api/types'

interface Props {
  onLogin:       (auth: LoginResponse) => void
  onStaffLogin:  (token: string, role: string, displayName: string) => void
  externalError?: string
}

export default function LoginPage({ onLogin, onStaffLogin, externalError }: Props) {
  const [loading, setLoading]   = useState(false)
  const [error, setError]       = useState('')
  const [staffOpen, setStaffOpen] = useState(false)
  const [staffMode, setStaffMode] = useState<'login' | 'bootstrap'>('login')

  // ── Student login ───────────────────────────────────────────────────────
  async function handleStudentSubmit(e: FormEvent<HTMLFormElement>) {
    e.preventDefault()
    setError('')
    setLoading(true)
    const form = e.currentTarget
    const username = (form.elements.namedItem('username') as HTMLInputElement).value.trim()
    const password = (form.elements.namedItem('password') as HTMLInputElement).value
    try {
      const auth = await api.login(username, password)
      if (auth.role !== 'student') {
        onStaffLogin(auth.token, auth.role, auth.display_name)
        return
      }
      onLogin(auth)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Login failed')
      setLoading(false)
    }
  }

  // ── Staff: username + password ──────────────────────────────────────────
  async function handleStaffLogin(e: FormEvent<HTMLFormElement>) {
    e.preventDefault()
    setError('')
    setLoading(true)
    const form = e.currentTarget
    const username = (form.elements.namedItem('s_username') as HTMLInputElement).value.trim()
    const password = (form.elements.namedItem('s_password') as HTMLInputElement).value
    try {
      const auth = await api.login(username, password)
      if (auth.role === 'student') {
        setError('This account does not have staff access.')
        setLoading(false)
        return
      }
      onStaffLogin(auth.token, auth.role, auth.display_name)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Login failed')
      setLoading(false)
    }
  }

  // ── Bootstrap: ADMIN_SECRET → token ────────────────────────────────────
  async function handleBootstrap(e: FormEvent<HTMLFormElement>) {
    e.preventDefault()
    setError('')
    setLoading(true)
    const form = e.currentTarget
    const secret = (form.elements.namedItem('secret') as HTMLInputElement).value
    try {
      const { token } = await api.bootstrapToken(secret)
      onStaffLogin(token, 'admin', 'Administrator')
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Invalid secret')
      setLoading(false)
    }
  }

  const inputCls = 'w-full px-4 py-2.5 rounded-lg border border-slate-300 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent text-slate-800 placeholder-slate-400'

  return (
    <div className="min-h-screen bg-gradient-to-br from-blue-50 to-indigo-100 flex items-center justify-center p-4">
      <div className="w-full max-w-md">

        {/* Brand */}
        <div className="text-center mb-8">
          <div className="inline-flex items-center justify-center w-16 h-16 rounded-2xl bg-blue-600 text-white text-3xl mb-4 shadow-lg">🎓</div>
          <h1 className="text-3xl font-bold text-slate-800">EduClaw</h1>
          <p className="text-slate-500 mt-1">Your AI-powered tutor</p>
        </div>

        {/* Student card */}
        <div className="bg-white rounded-2xl shadow-xl p-8">
          <h2 className="text-xl font-semibold text-slate-700 mb-6">Sign in</h2>

          <form onSubmit={handleStudentSubmit} className="space-y-4" autoComplete="off">
            <div>
              <label className="block text-sm font-medium text-slate-600 mb-1">Username</label>
              <input name="username" type="text" required placeholder="your-username" className={inputCls} />
            </div>
            <div>
              <label className="block text-sm font-medium text-slate-600 mb-1">Password</label>
              <input name="password" type="password" required placeholder="••••••••" className={inputCls} />
            </div>

            {(error || externalError) && !staffOpen && (
              <div className="flex items-start gap-2 bg-red-50 border border-red-200 text-red-700 rounded-lg px-4 py-3 text-sm">
                <span>⚠ {error || externalError}</span>
              </div>
            )}

            <button
              type="submit"
              disabled={loading}
              className="w-full py-3 px-4 bg-blue-600 hover:bg-blue-700 disabled:bg-blue-400 text-white font-semibold rounded-lg transition-colors shadow-sm"
            >
              {loading && !staffOpen ? (
                <span className="flex items-center justify-center gap-2">
                  <svg className="animate-spin h-4 w-4" viewBox="0 0 24 24" fill="none">
                    <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4"/>
                    <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v8H4z"/>
                  </svg>
                  Connecting…
                </span>
              ) : 'Start Learning'}
            </button>
          </form>
        </div>

        {/* Staff section — collapsed by default, subtle */}
        <div className="mt-6">
          <button
            onClick={() => { setStaffOpen(o => !o); setError('') }}
            className="w-full text-xs text-slate-400 hover:text-slate-500 transition-colors"
          >
            {staffOpen ? '▲ close' : 'Staff access'}
          </button>

          {staffOpen && (
            <div className="mt-3 bg-white rounded-xl shadow-md p-6 border border-slate-200">
              {/* Tab toggle */}
              <div className="flex gap-2 mb-4">
                <button
                  onClick={() => { setStaffMode('login'); setError('') }}
                  className={`flex-1 py-1.5 text-sm rounded-lg font-medium transition-colors ${staffMode === 'login' ? 'bg-slate-800 text-white' : 'bg-slate-100 text-slate-600 hover:bg-slate-200'}`}
                >
                  Sign in
                </button>
                <button
                  onClick={() => { setStaffMode('bootstrap'); setError('') }}
                  className={`flex-1 py-1.5 text-sm rounded-lg font-medium transition-colors ${staffMode === 'bootstrap' ? 'bg-slate-800 text-white' : 'bg-slate-100 text-slate-600 hover:bg-slate-200'}`}
                >
                  First-time setup
                </button>
              </div>

              {staffMode === 'login' ? (
                <form onSubmit={handleStaffLogin} className="space-y-3" autoComplete="off">
                  <input name="s_username" type="text" required placeholder="Username" className={inputCls} />
                  <input name="s_password" type="password" required placeholder="Password" className={inputCls} />
                  {error && staffOpen && staffMode === 'login' && (
                    <p className="text-red-600 text-sm">⚠ {error}</p>
                  )}
                  <button
                    type="submit"
                    disabled={loading}
                    className="w-full py-2 bg-slate-800 hover:bg-slate-700 disabled:bg-slate-400 text-white text-sm font-medium rounded-lg transition-colors"
                  >
                    {loading ? 'Signing in…' : 'Sign in as Staff'}
                  </button>
                </form>
              ) : (
                <form onSubmit={handleBootstrap} className="space-y-3">
                  <p className="text-xs text-slate-500">Enter the <code>ADMIN_SECRET</code> from your <code>.env</code> to access the admin panel.</p>
                  <input name="secret" type="password" required placeholder="Admin secret" className={inputCls} />
                  {error && staffOpen && staffMode === 'bootstrap' && (
                    <p className="text-red-600 text-sm">⚠ {error}</p>
                  )}
                  <button
                    type="submit"
                    disabled={loading}
                    className="w-full py-2 bg-slate-800 hover:bg-slate-700 disabled:bg-slate-400 text-white text-sm font-medium rounded-lg transition-colors"
                  >
                    {loading ? 'Verifying…' : 'Open Admin Panel'}
                  </button>
                </form>
              )}
            </div>
          )}
        </div>

        <p className="text-center text-slate-400 text-xs mt-6">EduClaw · Student learning platform</p>
      </div>
    </div>
  )
}
