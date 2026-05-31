import { useState, useRef, useCallback } from 'react'
import { motion } from 'framer-motion'
import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import { apiChatStream } from '@/stores/data'
import { useAiRoles } from '@/hooks/query/useAiRoles'

interface AiSidebarProps {
  overview?: unknown
}

interface DisplayMessage {
  role: 'user' | 'ai'
  text: string
}

const suggestedQuestions = [
  '今天市场情绪怎么样？',
  '当前龙头是谁？',
  '涨停板块有哪些？',
  '今天适合短线操作吗？',
]

export default function AiSidebar(_props: AiSidebarProps) {
  const [input, setInput] = useState('')
  const [messages, setMessages] = useState<DisplayMessage[]>([])
  const [streaming, setStreaming] = useState(false)
  const { data: aiRoles = [] } = useAiRoles()
  const [currentRole, setCurrentRole] = useState('zarili')
  const [showRoleDropdown, setShowRoleDropdown] = useState(false)
  const msgContainerRef = useRef<HTMLDivElement>(null)
  const inputRef = useRef<HTMLInputElement>(null)

  const currentRoleDisplay = aiRoles.find((r) => r.name === currentRole)?.display_name || currentRole

  const scrollDown = useCallback(() => {
    requestAnimationFrame(() => {
      if (msgContainerRef.current) {
        msgContainerRef.current.scrollTop = msgContainerRef.current.scrollHeight
      }
    })
  }, [])

  const appendToLastMessage = useCallback((text: string) => {
    setMessages((prev) => {
      const updated = [...prev]
      const last = updated[updated.length - 1]
      if (last && last.role === 'ai') {
        updated[updated.length - 1] = { ...last, text: last.text + text }
      }
      return updated
    })
    scrollDown()
  }, [scrollDown])

  const updateLastMessage = useCallback((text: string) => {
    setMessages((prev) => {
      const updated = [...prev]
      const last = updated[updated.length - 1]
      if (last && last.role === 'ai') {
        updated[updated.length - 1] = { ...last, text }
      }
      return updated
    })
  }, [])

  const send = useCallback(
    async (question?: string) => {
      const text = (question || input).trim()
      if (!text || streaming) return

      setInput('')
      const history = messages.map((m) => ({
        role: m.role === 'ai' ? 'assistant' : 'user',
        content: m.text,
      }))

      setMessages((prev) => [...prev, { role: 'user', text }, { role: 'ai', text: '' }])
      setStreaming(true)
      scrollDown()

      try {
        await apiChatStream(
          { message: text, role: currentRole, history },
          {
            onData: (data) => appendToLastMessage(data),
            onDone: () => setStreaming(false),
            onError: (err) => updateLastMessage(`[错误] ${err}`),
          },
        )
      } catch (e: unknown) {
        updateLastMessage(`[错误] ${(e as Error).message || '网络请求失败'}`)
      } finally {
        setStreaming(false)
      }
    },
    [input, streaming, messages, currentRole, scrollDown, appendToLastMessage, updateLastMessage],
  )

  const onKeydown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault()
        send()
      }
    },
    [send],
  )

  function selectRole(name: string) {
    setCurrentRole(name)
    setShowRoleDropdown(false)
    if (messages.length > 0) setMessages([])
  }

  return (
    <div className="h-full flex flex-col">
      <div ref={msgContainerRef} className="flex-1 overflow-y-auto p-4 space-y-3">
        {messages.length === 0 ? (
          <div className="text-center py-8">
            <p className="text-caption text-ink-muted mb-3">AI 交易副驾驶</p>
            <div className="flex justify-center mb-4">
              <div className="relative">
                <button
                  onClick={() => setShowRoleDropdown(!showRoleDropdown)}
                  className="flex items-center gap-2 px-3 py-1.5 rounded-card bg-bg-hover text-caption text-white hover:bg-bg-hover/80 transition-colors cursor-pointer"
                >
                  <span className="text-ai">{currentRoleDisplay}</span>
                  <svg xmlns="http://www.w3.org/2000/svg" className="w-3 h-3 text-ink-muted" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                    <path d="m6 9 6 6 6-6" />
                  </svg>
                </button>
                {showRoleDropdown && (
                  <div className="absolute top-full left-1/2 -translate-x-1/2 mt-1 bg-bg border border-hairline rounded-card shadow-lg z-10 min-w-[140px] py-1">
                    {aiRoles.map((r) => (
                      <button
                        key={r.name}
                        onClick={() => selectRole(r.name)}
                        className={`w-full text-left px-3 py-1.5 text-caption hover:bg-bg-hover transition-colors cursor-pointer ${
                          r.name === currentRole ? 'text-ai' : 'text-white'
                        }`}
                      >
                        {r.display_name}
                      </button>
                    ))}
                  </div>
                )}
              </div>
            </div>
            <p className="text-caption text-ink-muted text-sm">结合实时行情数据，回答短线市场问题</p>
            <div className="mt-4 space-y-2">
              {suggestedQuestions.map((q, qi) => (
                <motion.button
                  key={q}
                  onClick={() => send(q)}
                  className="w-full text-left px-3 py-2 rounded-card bg-bg-hover text-caption text-ink-muted hover:text-white transition-colors cursor-pointer"
                  initial={{ opacity: 0, x: -10 }}
                  animate={{ opacity: 1, x: 0 }}
                  transition={{ duration: 0.25, delay: qi * 0.06 }}
                  whileHover={{ x: 4 }}
                >
                  {q}
                </motion.button>
              ))}
            </div>
          </div>
        ) : (
          messages.map((msg, idx) => (
            <motion.div
              key={idx}
              className={`px-3 py-2 rounded-card text-caption ${
                msg.role === 'user' ? 'bg-ai/15 text-white ml-8' : 'bg-bg border border-hairline text-white/80 mr-4'
              }`}
              initial={{ opacity: 0, y: 8 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ duration: 0.2, delay: idx * 0.02 }}
            >
              {msg.role === 'ai' ? (
                <div className="ai-msg">
                  <ReactMarkdown remarkPlugins={[remarkGfm]}>{msg.text}</ReactMarkdown>
                </div>
              ) : (
                msg.text
              )}
            </motion.div>
          ))
        )}

        {streaming && (
          <div className="px-3 py-2 rounded-card text-caption bg-bg border border-hairline text-ai mr-4">
            <span className="animate-pulse">▌</span>
          </div>
        )}
      </div>

      <div className="flex gap-2 p-3 border-t border-hairline flex-shrink-0">
        <button
          onClick={() => setShowRoleDropdown(!showRoleDropdown)}
          className="px-2 py-2 bg-bg-hover text-ai rounded-button text-caption hover:bg-bg-hover/80 transition-colors cursor-pointer flex-shrink-0 text-xs font-mono"
          title={`当前角色: ${currentRoleDisplay}`}
        >
          {currentRoleDisplay}
        </button>
        {messages.length > 0 && (
          <button
            onClick={() => setMessages([])}
            className="px-2 py-2 bg-bg-hover text-ink-muted rounded-button text-caption hover:text-white transition-colors cursor-pointer flex-shrink-0"
            title="清空对话"
          >
            <svg xmlns="http://www.w3.org/2000/svg" className="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <path d="M3 6h18" />
              <path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6" />
              <path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2" />
            </svg>
          </button>
        )}
        <input
          ref={inputRef}
          value={input}
          onChange={(e) => setInput(e.target.value)}
          className="flex-1 bg-bg border border-hairline rounded-input px-3 py-2 text-caption text-white placeholder-ink-muted outline-none focus:border-ai/50 transition-colors font-mono"
          placeholder="问任何短线问题..."
          onKeyDown={onKeydown}
          disabled={streaming}
        />
        <button
          onClick={() => send()}
          disabled={streaming || !input.trim()}
          className="px-4 py-2 bg-ai/20 text-ai rounded-button text-caption font-mono hover:bg-ai/30 transition-colors cursor-pointer disabled:opacity-30 disabled:cursor-not-allowed"
        >
          发送
        </button>
      </div>
    </div>
  )
}