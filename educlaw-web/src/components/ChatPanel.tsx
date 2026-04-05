import { useEffect, useRef } from 'react'
import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter'
import { oneDark } from 'react-syntax-highlighter/dist/esm/styles/prism'
import type { ChatMessage } from '../api/types'

interface Props {
  messages: ChatMessage[]
  inputValue: string
  onInputChange: (v: string) => void
  onSend: (text: string) => void
  disabled: boolean
}

// A single message bubble.
function MessageBubble({ msg }: { msg: ChatMessage }) {
  const isUser = msg.role === 'user'

  return (
    <div className={`flex gap-3 ${isUser ? 'flex-row-reverse' : 'flex-row'} items-start`}>
      {/* Avatar */}
      <div className={`shrink-0 w-8 h-8 rounded-full flex items-center justify-center text-sm font-bold shadow-sm
        ${isUser ? 'bg-blue-500 text-white' : 'bg-gradient-to-br from-violet-500 to-indigo-600 text-white'}`}>
        {isUser ? '👤' : '🤖'}
      </div>

      {/* Bubble */}
      <div className={`max-w-[85%] rounded-2xl px-4 py-3 shadow-sm text-sm leading-relaxed
        ${isUser
          ? 'bg-blue-500 text-white rounded-tr-sm'
          : 'bg-white border border-slate-200 text-slate-800 rounded-tl-sm'
        }`}>
        {isUser ? (
          <p className="whitespace-pre-wrap">{msg.text}</p>
        ) : (
          <div className="prose prose-sm max-w-none prose-pre:p-0 prose-code:text-violet-700 prose-code:bg-violet-50 prose-code:px-1 prose-code:rounded">
            <ReactMarkdown
              remarkPlugins={[remarkGfm]}
              components={{
                code({ className, children, ...props }) {
                  const match = /language-(\w+)/.exec(className ?? '')
                  const isBlock = match !== null
                  if (isBlock) {
                    return (
                      <SyntaxHighlighter
                        style={oneDark}
                        language={match[1]}
                        PreTag="div"
                        customStyle={{ borderRadius: '0.5rem', fontSize: '0.78rem', margin: '0.5rem 0' }}
                      >
                        {String(children).replace(/\n$/, '')}
                      </SyntaxHighlighter>
                    )
                  }
                  return (
                    <code className={className} {...props}>
                      {children}
                    </code>
                  )
                },
              }}
            >
              {msg.text}
            </ReactMarkdown>
            {msg.pending && (
              <span className="inline-block w-1.5 h-4 bg-violet-400 animate-pulse ml-0.5 rounded-sm align-middle" />
            )}
          </div>
        )}
      </div>
    </div>
  )
}

export default function ChatPanel({ messages, inputValue, onInputChange, onSend, disabled }: Props) {
  const bottomRef = useRef<HTMLDivElement>(null)
  const textareaRef = useRef<HTMLTextAreaElement>(null)

  // Auto-scroll to bottom on new messages / streaming text
  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'smooth' })
  }, [messages])

  function handleKeyDown(e: React.KeyboardEvent<HTMLTextAreaElement>) {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault()
      doSend()
    }
  }

  function doSend() {
    const text = inputValue.trim()
    if (!text || disabled) return
    onSend(text)
    onInputChange('')
    setTimeout(() => textareaRef.current?.focus(), 0)
  }

  return (
    <div className="flex flex-col h-full">
      {/* Message list */}
      <div className="flex-1 overflow-y-auto px-4 py-4 space-y-4">
        {messages.length === 0 && (
          <div className="flex flex-col items-center justify-center h-full text-center py-16 text-slate-400 select-none">
            <div className="text-5xl mb-4">🤖</div>
            <p className="text-base font-medium text-slate-500">EduClaw AI Tutor</p>
            <p className="text-sm mt-1">Ask a question to get started.</p>
            <p className="text-xs mt-3 text-slate-300">Shift+Enter for new line · Enter to send</p>
          </div>
        )}
        {messages.map(msg => (
          <MessageBubble key={msg.id} msg={msg} />
        ))}
        <div ref={bottomRef} />
      </div>

      {/* Input bar */}
      <div className="shrink-0 border-t border-slate-200 bg-white px-4 py-3">
        <div className="flex gap-2 items-end">
          <textarea
            ref={textareaRef}
            className="flex-1 resize-none rounded-xl border border-slate-300 px-3.5 py-2.5 text-sm text-slate-800
              placeholder:text-slate-400 focus:outline-none focus:ring-2 focus:ring-violet-400 focus:border-transparent
              bg-slate-50 max-h-40 min-h-[2.75rem]"
            placeholder="Ask EduClaw anything… (Enter to send, Shift+Enter for new line)"
            rows={1}
            value={inputValue}
            onChange={e => {
              onInputChange(e.target.value)
              // Auto-grow
              e.target.style.height = 'auto'
              e.target.style.height = `${Math.min(e.target.scrollHeight, 160)}px`
            }}
            onKeyDown={handleKeyDown}
            disabled={disabled}
          />
          <button
            onClick={doSend}
            disabled={disabled || !inputValue.trim()}
            className="shrink-0 w-10 h-10 rounded-xl bg-violet-500 hover:bg-violet-600 disabled:bg-slate-200
              disabled:cursor-not-allowed text-white flex items-center justify-center transition-colors shadow-sm"
            title="Send message (Enter)"
          >
            {disabled
              ? <span className="w-4 h-4 border-2 border-white/40 border-t-white rounded-full animate-spin" />
              : <span className="text-base">↑</span>
            }
          </button>
        </div>
      </div>
    </div>
  )
}
