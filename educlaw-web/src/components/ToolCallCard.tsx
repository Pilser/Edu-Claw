import { useState } from 'react'
import type { MessageSegment } from '../api/types'

type ToolSegment = Extract<MessageSegment, { kind: 'tool' }>

export default function ToolCallCard({ seg }: { seg: ToolSegment }) {
  const [open, setOpen] = useState(false)
  const isOk = seg.exitCode === 0
  const hasOutput = seg.output.trim().length > 0

  return (
    <div className="my-1.5 rounded-lg border border-slate-200 text-xs font-mono overflow-hidden not-prose">
      <button
        onClick={() => setOpen(o => !o)}
        className="w-full flex items-center gap-2 px-3 py-1.5 bg-slate-50 hover:bg-slate-100 transition-colors text-left"
      >
        <span className="text-slate-400 text-[10px] select-none">{open ? '▼' : '▶'}</span>
        <span className="text-violet-600 font-semibold shrink-0">{seg.tool}</span>
        <span className="text-slate-600 flex-1 truncate">{seg.command}</span>
        {!seg.done ? (
          <span className="text-slate-400 animate-pulse shrink-0">running…</span>
        ) : (
          <span className={`shrink-0 font-bold ${isOk ? 'text-emerald-500' : 'text-red-500'}`}>
            {isOk ? '✓' : '✗'}&nbsp;exit&nbsp;{seg.exitCode}
          </span>
        )}
      </button>

      {open && (
        <div className="bg-slate-900 text-slate-100 border-t border-slate-700">
          {/* Command */}
          <div className="px-3 py-2 border-b border-slate-800 text-slate-400">
            <span className="text-slate-500 select-none">$ </span>
            <span className="text-slate-200">{seg.command}</span>
          </div>
          {/* Output */}
          {hasOutput && (
            <pre className="px-3 py-2 text-[11px] leading-relaxed whitespace-pre-wrap overflow-x-auto max-h-64">
              {seg.output}
            </pre>
          )}
          {!seg.done && !hasOutput && (
            <div className="px-3 py-2 text-slate-500 italic">running…</div>
          )}
        </div>
      )}
    </div>
  )
}
