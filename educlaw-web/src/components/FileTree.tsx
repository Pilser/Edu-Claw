import { useState, useEffect } from 'react'
import { api } from '../api/client'
import type { FileNode } from '../api/types'

interface Props {
  token: string
  sessionId: string
  classId: string
  onFileSelect: (path: string, name: string) => void
}

type Tab = 'class' | 'mine'

function FileRow({
  node,
  depth,
  onSelect,
}: {
  node: FileNode
  depth: number
  onSelect: (path: string, name: string) => void
}) {
  const [open, setOpen] = useState(depth === 0)
  const pad = depth * 12

  if (node.kind === 'dir') {
    return (
      <div>
        <button
          className="flex items-center gap-1 w-full px-2 py-1 text-sm text-slate-600 hover:bg-slate-100 rounded text-left"
          style={{ paddingLeft: pad + 8 }}
          onClick={() => setOpen(o => !o)}
        >
          <span className="text-xs opacity-50">{open ? '▾' : '▸'}</span>
          <span>📁</span>
          <span className="truncate">{node.name}</span>
        </button>
        {open && node.children?.map(child => (
          <FileRow key={child.path} node={child} depth={depth + 1} onSelect={onSelect} />
        ))}
      </div>
    )
  }

  const icon = node.name.endsWith('.pdf') ? '📄'
    : node.name.endsWith('.md') ? '📝'
    : node.name.match(/\.(png|jpg|jpeg|svg|gif)$/) ? '🖼'
    : node.name.match(/\.(py|rs|js|ts)$/) ? '💻'
    : '📃'

  return (
    <button
      className="flex items-center gap-1.5 w-full px-2 py-1 text-sm text-slate-700 hover:bg-blue-50 hover:text-blue-700 rounded text-left"
      style={{ paddingLeft: pad + 8 }}
      onClick={() => onSelect(node.path, node.name)}
    >
      <span>{icon}</span>
      <span className="truncate">{node.name}</span>
      {node.size !== undefined && (
        <span className="ml-auto text-xs text-slate-400 shrink-0">
          {node.size < 1024 ? `${node.size}B` : `${(node.size / 1024).toFixed(1)}K`}
        </span>
      )}
    </button>
  )
}

export default function FileTree({ token, sessionId, classId, onFileSelect }: Props) {
  const [tab, setTab]       = useState<Tab>('class')
  const [files, setFiles]   = useState<FileNode[]>([])
  const [loading, setLoading] = useState(false)
  const [error, setError]   = useState('')

  useEffect(() => {
    setError('')
    setLoading(true)
    const req = tab === 'class'
      ? api.listClassFiles(classId, token)
      : api.listStudentFiles(sessionId, token)

    req
      .then(setFiles)
      .catch(e => setError(e instanceof Error ? e.message : 'Failed to load files'))
      .finally(() => setLoading(false))
  }, [tab, token, classId, sessionId])

  return (
    <div className="flex flex-col h-full">
      {/* Tabs */}
      <div className="flex border-b border-slate-200 shrink-0">
        {(['class', 'mine'] as Tab[]).map(t => (
          <button
            key={t}
            onClick={() => setTab(t)}
            className={`flex-1 py-2 text-xs font-medium transition-colors ${
              tab === t
                ? 'text-blue-600 border-b-2 border-blue-600 -mb-px'
                : 'text-slate-500 hover:text-slate-700'
            }`}
          >
            {t === 'class' ? '📚 Class' : '🗂 My Files'}
          </button>
        ))}
      </div>

      {/* File list */}
      <div className="flex-1 overflow-y-auto py-1">
        {loading && (
          <p className="text-xs text-slate-400 text-center py-6">Loading…</p>
        )}
        {error && (
          <p className="text-xs text-red-500 px-3 py-2">{error}</p>
        )}
        {!loading && !error && files.length === 0 && (
          <p className="text-xs text-slate-400 text-center py-6">No files yet</p>
        )}
        {files.map(node => (
          <FileRow key={node.path} node={node} depth={0} onSelect={onFileSelect} />
        ))}
      </div>
    </div>
  )
}
