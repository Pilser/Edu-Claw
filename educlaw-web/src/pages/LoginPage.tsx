import { useState, type FormEvent } from 'react'
import { api } from '../api/client'
import type { LoginResponse, Session } from '../api/types'

interface Props {
  onLogin: (auth: LoginResponse, session: Session, course: string) => void
}

export default function LoginPage({ onLogin }: Props) {
  const [loading, setLoading] = useState(false)
  const [error, setError]     = useState('')

  async function handleSubmit(e: FormEvent<HTMLFormElement>) {
    e.preventDefault()
    setError('')
    setLoading(true)

    const form       = e.currentTarget
    const username   = (form.elements.namedItem('username')   as HTMLInputElement).value.trim()
    const password   = (form.elements.namedItem('password')   as HTMLInputElement).value
    const course     = (form.elements.namedItem('course')     as HTMLInputElement).value.trim()
    const assignment = (form.elements.namedItem('assignment') as HTMLInputElement).value.trim() || undefined

    try {
      const auth = await api.login(username, password)
      const sess = await api.createSession(auth.student_id, auth.token, course, assignment)
      onLogin(auth, sess, course)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Login failed')
      setLoading(false)
    }
  }

  return (
    <div className="min-h-screen bg-gradient-to-br from-blue-50 to-indigo-100 flex items-center justify-center p-4">
      <div className="w-full max-w-md">

        {/* Logo / Brand */}
        <div className="text-center mb-8">
          <div className="inline-flex items-center justify-center w-16 h-16 rounded-2xl bg-blue-600 text-white text-3xl mb-4 shadow-lg">
            🎓
          </div>
          <h1 className="text-3xl font-bold text-slate-800">EduClaw</h1>
          <p className="text-slate-500 mt-1">Your AI-powered tutor</p>
        </div>

        {/* Card */}
        <div className="bg-white rounded-2xl shadow-xl p-8">
          <h2 className="text-xl font-semibold text-slate-700 mb-6">Sign in to start learning</h2>

          <form onSubmit={handleSubmit} className="space-y-4" autoComplete="off">
            <div>
              <label className="block text-sm font-medium text-slate-600 mb-1">Username</label>
              <input
                name="username"
                type="text"
                required
                placeholder="your-username"
                className="w-full px-4 py-2.5 rounded-lg border border-slate-300 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent text-slate-800 placeholder-slate-400"
              />
            </div>

            <div>
              <label className="block text-sm font-medium text-slate-600 mb-1">Password</label>
              <input
                name="password"
                type="password"
                required
                placeholder="••••••••"
                className="w-full px-4 py-2.5 rounded-lg border border-slate-300 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent text-slate-800 placeholder-slate-400"
              />
            </div>

            <div>
              <label className="block text-sm font-medium text-slate-600 mb-1">Course code</label>
              <input
                name="course"
                type="text"
                required
                placeholder="e.g. physics101"
                className="w-full px-4 py-2.5 rounded-lg border border-slate-300 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent text-slate-800 placeholder-slate-400"
              />
            </div>

            <div>
              <label className="block text-sm font-medium text-slate-600 mb-1">
                Assignment <span className="text-slate-400 font-normal">(optional)</span>
              </label>
              <input
                name="assignment"
                type="text"
                placeholder="e.g. hw1"
                className="w-full px-4 py-2.5 rounded-lg border border-slate-300 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent text-slate-800 placeholder-slate-400"
              />
            </div>

            {error && (
              <div className="flex items-start gap-2 bg-red-50 border border-red-200 text-red-700 rounded-lg px-4 py-3 text-sm">
                <span className="mt-0.5">⚠</span>
                <span>{error}</span>
              </div>
            )}

            <button
              type="submit"
              disabled={loading}
              className="w-full py-3 px-4 bg-blue-600 hover:bg-blue-700 disabled:bg-blue-400 text-white font-semibold rounded-lg transition-colors shadow-sm"
            >
              {loading ? (
                <span className="flex items-center justify-center gap-2">
                  <svg className="animate-spin h-4 w-4" viewBox="0 0 24 24" fill="none">
                    <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4"/>
                    <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v8H4z"/>
                  </svg>
                  Connecting…
                </span>
              ) : 'Start Session'}
            </button>
          </form>
        </div>

        <p className="text-center text-slate-400 text-xs mt-6">
          EduClaw · Student learning platform
        </p>
      </div>
    </div>
  )
}
