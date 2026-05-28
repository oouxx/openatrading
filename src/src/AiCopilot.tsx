import { useState, useRef, useCallback } from 'react'
import { motion } from 'framer-motion'
import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import { apiChatStream } from '@/stores/data'

interface DisplayMessage {
  role: 'user' | 'ai'
  text: string
}

const suggestedQuestions = [
  '帮我分析一下今天的市场环境',
  '当前有哪些热门板块？',
  '短线操作有什么建议？',
  '帮我制定今天的交易计划',
]

export default function AiCopilot() {
  const [input, setInput] = useState('')
  const [messages, setMessages] = useState<DisplayMessage[]>([])
  const [streaming, setStreaming] = useState(false)
  const msgContainerRef = useRef<HTMLDivElement>(null)

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
          { message: text, role: 'zarili', history },
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
    [input, streaming, messages, scrollDown, appendToLastMessage, updateLastMessage],
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

  return (
    <div className="min-h-screen bg-bg pt-14">
      <div className="max-w-[800px] mx-auto px-6 py-4 h-[calc(100vh-56px)] flex flex-col">
        <h1 className="text-display-md text-white mb-4 flex-shrink-0">AI 副驾驶</h1>

        <div ref={msgContainerRef} className="flex-1 overflow-y-auto space-y-3 mb-4">
          {messages.length === 0 ? (
            <div className="text-center py-16">
              <p className="text-ink-muted text-caption mb-6">结合实时行情数据，AI 为你提供短线交易决策支持</p>
              <div className="grid grid-cols-2 gap-3 max-w-md mx-auto">
                {suggestedQuestions.map((q, qi) => (
                  <motion.button
                    key={q}
                    onClick={() => send(q)}
                    className="text-left px-4 py-3 rounded-card bg-bg-surface border border-hairline text-caption text-ink-muted hover:text-white hover:border-ai/30 transition-colors cursor-pointer"
                    initial={{ opacity: 0, y: 10 }}
                    animate={{ opacity: 1, y: 0 }}
                    transition={{ duration: 0.25, delay: qi * 0.08 }}
                    whileHover={{ y: -2 }}
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
                className={`px-4 py-3 rounded-card text-caption ${
                  msg.role === 'user'
                    ? 'bg-ai/15 text-white ml-12'
                    : 'bg-bg-surface border border-hairline text-white/80 mr-4'
                }`}
                initial={{ opacity: 0, y: 8 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ duration: 0.2 }}
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
            <div className="px-4 py-3 rounded-card text-caption bg-bg-surface border border-hairline text-ai mr-4">
              <span className="animate-pulse">▌</span>
            </div>
          )}
        </div>

        <div className="flex gap-3 flex-shrink-0">
          {messages.length > 0 && (
            <button
              onClick={() => setMessages([])}
              className="px-3 py-2 bg-bg-surface border border-hairline text-ink-muted rounded-button text-caption hover:text-white transition-colors cursor-pointer"
            >
              清空
            </button>
          )}
          <input
            value={input}
            onChange={(e) => setInput(e.target.value)}
            className="flex-1 bg-bg-surface border border-hairline rounded-input px-4 py-3 text-caption text-white placeholder-ink-muted outline-none focus:border-ai/50 transition-colors font-mono"
            placeholder="输入你的问题..."
            onKeyDown={onKeydown}
            disabled={streaming}
          />
          <button
            onClick={() => send()}
            disabled={streaming || !input.trim()}
            className="px-6 py-3 bg-ai/20 text-ai rounded-button text-caption font-mono hover:bg-ai/30 transition-colors cursor-pointer disabled:opacity-30 disabled:cursor-not-allowed"
          >
            发送
          </button>
        </div>
      </div>
    </div>
  )
}