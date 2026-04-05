import { useState, useEffect } from 'react'
import { api } from '../api/client'
import type { AssignmentProgress } from '../api/types'

interface Props {
  sessionId: string
  token: string
}

function ProgressBar({ value, max }: { value: number; max: number }) {
  const pct = max === 0 ? 0 : Math.round((value / max) * 100)
  return (
    <div className="w-full bg-slate-200 rounded-full h-2">
      <div
        className="bg-blue-500 h-2 rounded-full transition-all"
        style={{ width: `${pct}%` }}
      />
    </div>
  )
}

export default function AssignmentPanel({ sessionId, token }: Props) {
  const [progress, setProgress] = useState<AssignmentProgress | null>(null)
  const [loading, setLoading]   = useState(true)

  useEffect(() => {
    api.getProgress(sessionId, token)
      .then(setProgress)
      .catch(() => setProgress(null))
      .finally(() => setLoading(false))

    // Refresh every 30s so scores update as the AI records answers
    const tid = setInterval(() => {
      api.getProgress(sessionId, token).then(setProgress).catch(() => {})
    }, 30_000)
    return () => clearInterval(tid)
  }, [sessionId, token])

  if (loading) return (
    <div className="p-4 text-xs text-slate-400 text-center">Loading progress…</div>
  )

  if (!progress) return (
    <div className="p-4 text-xs text-slate-400 text-center">No active assignment</div>
  )

  const pct = progress.max_score === 0
    ? 0
    : Math.round((progress.total_score / progress.max_score) * 100)

  return (
    <div className="p-4 flex flex-col gap-4">
      {/* Header */}
      <div>
        <h3 className="font-semibold text-slate-800 text-sm truncate">{progress.title}</h3>
        <p className="text-xs text-slate-500 mt-0.5">
          {progress.submitted
            ? '✅ Submitted'
            : `${progress.questions.filter(q => q.answered).length} / ${progress.questions.length} questions answered`}
        </p>
      </div>

      {/* Score bar */}
      <div>
        <div className="flex justify-between text-xs text-slate-600 mb-1">
          <span>Score</span>
          <span className="font-medium">{progress.total_score} / {progress.max_score} pts ({pct}%)</span>
        </div>
        <ProgressBar value={progress.total_score} max={progress.max_score} />
      </div>

      {/* Per-question breakdown */}
      <div className="space-y-2">
        {progress.questions.map(q => (
          <div key={q.question_id} className="flex items-start gap-2">
            <span className="mt-0.5 text-sm shrink-0">
              {!q.answered ? '○' : q.correct ? '✅' : '❌'}
            </span>
            <div className="min-w-0">
              <p className="text-xs text-slate-700 truncate">{q.prompt}</p>
              <p className="text-xs text-slate-400">
                {q.answered
                  ? `${q.score}/${q.max_points} pts · ${q.attempts} attempt${q.attempts !== 1 ? 's' : ''}`
                  : 'Not answered'}
              </p>
            </div>
          </div>
        ))}
      </div>
    </div>
  )
}
