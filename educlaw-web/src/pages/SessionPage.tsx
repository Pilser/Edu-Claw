import { useState, useCallback, useRef } from 'react'
import TutorTerminal from '../components/TutorTerminal'
import FileTree from '../components/FileTree'
import AssignmentPanel from '../components/AssignmentPanel'
import { useWebSocket, type WsStatus } from '../hooks/useWebSocket'
import { api } from '../api/client'
import type { LoginResponse, Session, ShowImagePayload } from '../api/types'

interface Props {
  auth: LoginResponse
  session: Session
  course: string
  onEnd: () => void
}

function StatusDot({ status }: { status: WsStatus }) {
  const colors: Record<WsStatus, string> = {
    connecting:   'bg-yellow-400',
    connected:    'bg-emerald-400',
    disconnected: 'bg-red-400',
    error:        'bg-red-500',
  }
  const labels: Record<WsStatus, string> = {
    connecting:   'Connecting',
    connected:    'Connected',
    disconnected: 'Disconnected',
    error:        'Connection error',
  }
  return (
    <span className="flex items-center gap-1.5 text-xs text-slate-500">
      <span className={`w-2 h-2 rounded-full ${colors[status]} ${status === 'connecting' ? 'animate-pulse' : ''}`} />
      {labels[status]}
    </span>
  )
}

export default function SessionPage({ auth, session, course, onEnd }: Props) {
  const [sidebarOpen, setSidebarOpen]       = useState(true)
  const [rightPanelOpen, setRightPanelOpen] = useState(true)
  const [lightboxImage, setLightboxImage]   = useState<ShowImagePayload | null>(null)

  // Terminal write function set once xterm mounts
  const ptyWriteRef  = useRef<((d: Uint8Array) => void) | null>(null)

  const handlePtyReady = useCallback(
    (write: (d: Uint8Array) => void, _resize: (cols: number, rows: number) => void) => {
      ptyWriteRef.current = write
    },
    [],
  )

  const handlePtyData = useCallback((data: Uint8Array) => {
    ptyWriteRef.current?.(data)
  }, [])

  const handleImage = useCallback((p: ShowImagePayload) => {
    setLightboxImage(p)
  }, [])

  const { status, sendInput, sendResize } = useWebSocket({
    sessionId: session.session_id,
    token:     session.token,
    onPtyData: handlePtyData,
    onImage:   handleImage,
  })

  async function handleEndSession() {
    if (!confirm('End this session? Your progress is saved.')) return
    await api.endSession(session.session_id, session.token).catch(() => {})
    onEnd()
  }

  return (
    <div className="flex flex-col h-screen bg-slate-100">
      {/* ── Header ──────────────────────────────────────────────────────── */}
      <header className="shrink-0 h-14 bg-white border-b border-slate-200 flex items-center px-4 gap-4 shadow-sm">
        {/* Logo */}
        <div className="flex items-center gap-2 mr-2">
          <span className="text-xl">🎓</span>
          <span className="font-bold text-slate-800 text-sm">EduClaw</span>
        </div>

        {/* Course badge */}
        <span className="px-2.5 py-0.5 rounded-full bg-blue-100 text-blue-700 text-xs font-medium">
          {course}
        </span>

        {/* Status */}
        <StatusDot status={status} />

        <div className="flex-1" />

        {/* Student name */}
        <span className="text-sm text-slate-600 hidden sm:block">
          👤 {auth.display_name}
        </span>

        {/* Sidebar toggle */}
        <button
          onClick={() => setSidebarOpen(o => !o)}
          title="Toggle file browser"
          className="p-2 rounded-lg hover:bg-slate-100 text-slate-500 hover:text-slate-700"
        >
          📁
        </button>

        {/* Progress toggle */}
        <button
          onClick={() => setRightPanelOpen(o => !o)}
          title="Toggle progress"
          className="p-2 rounded-lg hover:bg-slate-100 text-slate-500 hover:text-slate-700"
        >
          📊
        </button>

        {/* End session */}
        <button
          onClick={handleEndSession}
          className="px-3 py-1.5 rounded-lg bg-red-50 hover:bg-red-100 text-red-600 text-xs font-medium transition-colors"
        >
          End session
        </button>
      </header>

      {/* ── Body ────────────────────────────────────────────────────────── */}
      <div className="flex flex-1 overflow-hidden">

        {/* Left sidebar — file browser */}
        {sidebarOpen && (
          <aside className="w-60 shrink-0 bg-white border-r border-slate-200 flex flex-col overflow-hidden">
            <div className="px-3 pt-3 pb-1.5 text-xs font-semibold text-slate-500 uppercase tracking-wider">
              Files
            </div>
            <div className="flex-1 overflow-hidden">
              <FileTree
                token={session.token}
                sessionId={session.session_id}
                classId={course}
                onFileSelect={(_path, name) => {
                  // Phase 4 — file viewer; for now just log
                  console.info('file selected:', name)
                }}
              />
            </div>
          </aside>
        )}

        {/* Centre — AI Tutor terminal */}
        <main className="flex-1 flex flex-col overflow-hidden p-3 gap-2">
          {/* Section label */}
          <div className="shrink-0 flex items-center gap-2">
            <span className="text-sm font-semibold text-slate-700">🤖 AI Tutor</span>
            <span className="text-xs text-slate-400">Powered by Claude</span>
          </div>

          {/* Terminal card */}
          <div className="flex-1 rounded-xl overflow-hidden shadow-inner border border-slate-800 bg-slate-900">
            <TutorTerminal
              onPtyReady={handlePtyReady}
              onInput={sendInput}
              onResize={sendResize}
            />
          </div>
        </main>

        {/* Right panel — assignment progress */}
        {rightPanelOpen && (
          <aside className="w-64 shrink-0 bg-white border-l border-slate-200 flex flex-col overflow-hidden">
            <div className="px-3 pt-3 pb-1 text-xs font-semibold text-slate-500 uppercase tracking-wider shrink-0">
              Assignment Progress
            </div>
            <div className="flex-1 overflow-y-auto">
              <AssignmentPanel
                sessionId={session.session_id}
                token={session.token}
              />
            </div>
          </aside>
        )}
      </div>

      {/* ── Image lightbox (Phase 4.5) ──────────────────────────────────── */}
      {lightboxImage && (
        <div
          className="fixed inset-0 bg-black/70 flex items-center justify-center z-50 p-4"
          onClick={() => setLightboxImage(null)}
        >
          <div
            className="bg-white rounded-2xl shadow-2xl max-w-3xl max-h-[85vh] overflow-hidden"
            onClick={e => e.stopPropagation()}
          >
            <div className="flex items-center justify-between px-4 py-3 border-b border-slate-200">
              <span className="text-sm font-medium text-slate-700 truncate">
                {lightboxImage.path.split('/').pop()}
              </span>
              <button
                onClick={() => setLightboxImage(null)}
                className="text-slate-400 hover:text-slate-600 text-lg leading-none"
              >
                ✕
              </button>
            </div>
            <div className="overflow-auto p-4">
              <img
                src={lightboxImage.data_url}
                alt="Figure"
                className="max-w-full h-auto rounded"
              />
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
