import { useEffect, useRef, useState, useCallback } from 'react'
import type { WsEnvelope, ShowImagePayload } from '../api/types'

export type WsStatus = 'connecting' | 'connected' | 'disconnected' | 'error'

interface Options {
  sessionId: string
  token: string
  onPtyData: (data: Uint8Array) => void
  onImage: (p: ShowImagePayload) => void
  onChatStart?: () => void
  onToolStart?: (tool: string, command: string) => void
  onToolEnd?: (exitCode: number, stdout: string, stderr: string) => void
  onAssistantText?: (text: string) => void
  onChatEnd?: () => void
}

export function useWebSocket({ sessionId, token, onPtyData, onImage, onChatStart, onToolStart, onToolEnd, onAssistantText, onChatEnd }: Options) {
  const [status, setStatus] = useState<WsStatus>('connecting')
  const wsRef = useRef<WebSocket | null>(null)
  const keepaliveRef = useRef<ReturnType<typeof setInterval> | null>(null)

  const send = useCallback((msg: object) => {
    wsRef.current?.readyState === WebSocket.OPEN &&
      wsRef.current.send(JSON.stringify(msg))
  }, [])

  const sendInput = useCallback((data: string) => {
    send({ type: 'input', data })
  }, [send])

  const sendResize = useCallback((cols: number, rows: number) => {
    send({ type: 'resize', cols, rows })
  }, [send])

  const sendCancel = useCallback(() => {
    send({ type: 'cancel' })
  }, [send])

  useEffect(() => {
    const proto = location.protocol === 'https:' ? 'wss:' : 'ws:'
    const url = `${proto}//${location.host}/sessions/${sessionId}/ws?token=${encodeURIComponent(token)}`

    const ws = new WebSocket(url)
    ws.binaryType = 'arraybuffer'
    wsRef.current = ws

    ws.onopen = () => {
      setStatus('connected')
      keepaliveRef.current = setInterval(() => {
        ws.readyState === WebSocket.OPEN && ws.send(JSON.stringify({ type: 'ping' }))
      }, 30_000)
    }

    ws.onmessage = (e) => {
      if (e.data instanceof ArrayBuffer) {
        // Check for OSC envelope sentinel  \x1b]99;{json}\x07
        const raw = new Uint8Array(e.data)
        const text = new TextDecoder().decode(raw)
        const oscMatch = text.match(/\x1b\]99;(.+?)\x07/)
        if (oscMatch) {
          const clean = text.replace(/\x1b\]99;.+?\x07/g, '')
          if (clean.length > 0) onPtyData(new TextEncoder().encode(clean))
          try {
            const envelope: WsEnvelope = JSON.parse(oscMatch[1])
            if (envelope.type === 'show_image') onImage(envelope.payload as ShowImagePayload)
          } catch { /* malformed envelope — ignore */ }
        } else {
          onPtyData(raw)
        }
      } else {
        // JSON control frame from server
        try {
          const frame = JSON.parse(e.data as string) as {
            type: string; text?: string; tool?: string; command?: string;
            exit_code?: number; payload?: unknown
          }
          switch (frame.type) {
            case 'show_image':
              onImage(frame.payload as ShowImagePayload)
              break
            case 'chat_start':
              onChatStart?.()
              break
            case 'tool_start':
              onToolStart?.(frame.tool ?? '', frame.command ?? '')
              break
            case 'tool_end': {
              const code = typeof frame.exit_code === 'number' ? frame.exit_code : 0
              const stdout = (frame as any).stdout ?? ''
              const stderr = (frame as any).stderr ?? ''
              onToolEnd?.(code, stdout, stderr)
              break
            }
            case 'assistant_text':
              if (frame.text) onAssistantText?.(frame.text)
              break
            case 'chat_end':
              onChatEnd?.()
              break
            default:
              break
          }
        } catch { /* ignore */ }
      }
    }

    ws.onclose = () => {
      setStatus('disconnected')
      if (keepaliveRef.current) clearInterval(keepaliveRef.current)
    }

    ws.onerror = () => setStatus('error')

    return () => {
      ws.close()
      if (keepaliveRef.current) clearInterval(keepaliveRef.current)
    }
  }, [sessionId, token, onPtyData, onImage, onChatStart, onToolStart, onToolEnd, onAssistantText, onChatEnd])

  return { status, sendInput, sendResize, sendCancel }
}
