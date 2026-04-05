import { useEffect, useRef } from 'react'
import { Terminal } from '@xterm/xterm'
import { FitAddon } from '@xterm/addon-fit'
import '@xterm/xterm/css/xterm.css'

interface Props {
  onPtyReady: (write: (data: Uint8Array) => void, resize: (cols: number, rows: number) => void) => void
  onInput: (data: string) => void
  onResize: (cols: number, rows: number) => void
}

export default function TutorTerminal({ onPtyReady, onInput, onResize }: Props) {
  const containerRef = useRef<HTMLDivElement>(null)
  const termRef      = useRef<Terminal | null>(null)
  const fitRef       = useRef<FitAddon | null>(null)

  useEffect(() => {
    if (!containerRef.current) return

    const term = new Terminal({
      cursorBlink: true,
      fontSize: 14,
      fontFamily: '"Cascadia Code", "Fira Code", "Consolas", monospace',
      theme: {
        background:  '#0f172a',   // slate-900
        foreground:  '#e2e8f0',   // slate-200
        cursor:      '#60a5fa',   // blue-400
        selectionBackground: '#334155',
        black:       '#1e293b',
        brightBlack: '#475569',
        red:         '#f87171',
        green:       '#4ade80',
        yellow:      '#fbbf24',
        blue:        '#60a5fa',
        magenta:     '#c084fc',
        cyan:        '#22d3ee',
        white:       '#e2e8f0',
        brightWhite: '#f8fafc',
      },
      scrollback: 5000,
      allowProposedApi: true,
    })

    const fit = new FitAddon()
    term.loadAddon(fit)
    term.open(containerRef.current)
    fit.fit()

    termRef.current = term
    fitRef.current  = fit

    // Expose write / initial-resize to parent
    onPtyReady(
      (data) => term.write(data),
      (cols, rows) => { void cols; void rows }, // initial noop — parent calls onResize
    )

    // Keystrokes → parent
    term.onData(onInput)

    // Fit → report resize to parent
    const observer = new ResizeObserver(() => {
      fit.fit()
      onResize(term.cols, term.rows)
    })
    observer.observe(containerRef.current)

    return () => {
      observer.disconnect()
      term.dispose()
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  return (
    <div
      ref={containerRef}
      className="xterm-container w-full h-full"
    />
  )
}
