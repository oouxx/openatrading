import { useState, useEffect, useRef, useCallback } from 'react'
import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import { apiChatStream, useDataStore, type IndexQuote } from '@/stores/data'

interface DisplayMessage {
  role: 'user' | 'ai'
  text: string
}

const quickPrompts = [
  { icon: '📊', label: '大盘研判', text: '今天A股大盘怎么看？上证和创业板哪个更强？' },
  { icon: '💰', label: '北向资金', text: '北向资金最近一周流向如何？哪些板块在流入？' },
  { icon: '🍶', label: '个股分析', text: '帮我分析一下贵州茅台当前的技术面和估值' },
  { icon: '🔄', label: '板块轮动', text: '近期哪些板块有轮动机会？' },
  { icon: '📅', label: '事件驱动', text: '重要会议前后A股一般怎么走？历史规律复盘' },
]

// ── Ticker ───────────────────────────────────────────

function TickerStrip() {
  const store = useDataStore()
  const [data, setData] = useState<IndexQuote[]>([])
  const [prices, setPrices] = useState<Record<string, { price: number; change: number }>>({})

  useEffect(() => { store.fetchIndexQuotes() }, [])
  useEffect(() => {
    const quotes = store.indexQuotes.data
    if (quotes.length && !data.length) {
      setData(quotes)
      const p: Record<string, { price: number; change: number }> = {}
      quotes.forEach((q) => {
        p[q.symbol] = { price: q.last_px, change: q.px_change_rate }
      })
      setPrices(p)
    }
  }, [store.indexQuotes.data])

  if (!data.length) return null

  const iconInfo: Record<string, { cls: string; label: string }> = {
    '000001.SS': { cls: 'sh', label: '沪' },
    '399001.SZ': { cls: 'sz', label: '深' },
    '000300.SS': { cls: 'hs300', label: '300' },
    '399006.SZ': { cls: 'cyb', label: '创' },
    '000688.SS': { cls: 'kc50', label: '科' },
  }

  const tickerData = data.map((d) => ({
    name: d.name.length > 4 ? d.name.replace('指数', '') : d.name,
    icon: iconInfo[d.symbol] || { cls: 'kc50', label: d.name[0] },
    price: prices[d.symbol]?.price ?? d.last_px,
    change: prices[d.symbol]?.change ?? d.px_change_rate,
    up: (prices[d.symbol]?.change ?? d.px_change_rate) >= 0,
  }))

  return (
    <div className="bg-bg-surface border-b border-hairline overflow-x-auto whitespace-nowrap" style={{ WebkitOverflowScrolling: 'touch' }}>
      <div className="inline-flex gap-0 px-4">
        {tickerData.map((m) => {
          const iconBg = {
            sh: 'bg-up',
            sz: 'bg-[oklch(45%_0.14_240)]',
            hs300: 'bg-ai',
            cyb: 'bg-down',
            kc50: 'bg-[oklch(42%_0.16_280)]',
          }
          return (
            <div
              key={m.name}
              className="inline-flex items-center gap-2.5 py-2.5 px-[18px] border-r border-hairline last:border-r-0 min-w-[178px] cursor-pointer hover:opacity-80 transition-opacity"
            >
              <span className={`w-8 h-8 rounded-lg flex items-center justify-center text-[10px] font-bold text-white flex-shrink-0 ${iconBg[m.icon.cls] || 'bg-ai'}`}>
                {m.icon.label}
              </span>
              <div className="leading-tight">
                <div className="font-semibold text-xs text-white tracking-wide">{m.name}</div>
                <div className="font-mono text-base font-semibold text-white">
                  {m.price.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}
                </div>
                <div className={`font-mono text-[11px] font-medium ${m.up ? 'text-up' : 'text-down'}`}>
                  {m.up ? '+' : ''}{m.change.toFixed(2)}%
                </div>
              </div>
            </div>
          )
        })}
      </div>
    </div>
  )
}

// ── Sidebar ───────────────────────────────────────────

function ContextSidebar() {
  const store = useDataStore()
  const overview = store.overview.data

  return (
    <aside className="w-[260px] min-w-[260px] bg-bg-surface border-l border-hairline flex flex-col overflow-y-auto hidden md:flex">
      <div className="px-4 py-4 border-b border-hairline">
        <h4 className="text-sm font-medium text-white mb-2.5">市场快照</h4>
        {overview ? (
          <div className="space-y-1">
            <SidebarRow label="涨停 / 跌停" value={`${overview.limit_up_count} / ${overview.limit_down_count}`} />
            <SidebarRow label="上涨 / 下跌" value={`${overview.rise_count} / ${overview.fall_count}`} />
            <SidebarRow label="炸板率" value={`${(overview.bomb_rate * 100).toFixed(1)}%`} />
            <SidebarRow label="市场温度" value={`${overview.market_temperature.toFixed(0)}°`} />
          </div>
        ) : (
          <div className="text-[11px] text-ink-muted">加载中...</div>
        )}
      </div>
      <div className="px-4 py-4 border-b border-hairline">
        <h4 className="text-sm font-medium text-white mb-2.5">风险 & 情绪</h4>
        {overview ? (
          <>
            <SidebarRow label="风险等级" value={overview.risk_level} valueClass={overview.risk_level === '高' ? 'text-up' : overview.risk_level === '中' ? 'text-ai' : 'text-down'} />
            <p className="text-[11px] text-ink-muted leading-relaxed mt-1.5 bg-ink-dim rounded-lg p-2">
              {overview.decision_summary || '暂无决策摘要'}
            </p>
          </>
        ) : (
          <div className="text-[11px] text-ink-muted">加载中...</div>
        )}
      </div>
      <div className="px-4 py-4 border-b border-hairline">
        <h4 className="text-sm font-medium text-white mb-2.5">活跃角色策略</h4>
        <p className="text-[11px] text-ink-muted leading-relaxed bg-ink-dim rounded-lg p-2">
          趋势跟踪（沪深300）运行中<br />
          均值回归策略 监控中<br />
          套利策略 v1 暂停
        </p>
      </div>
    </aside>
  )
}

function SidebarRow({ label, value, valueClass = 'text-white' }: { label: string; value: string; valueClass?: string }) {
  return (
    <div className="flex justify-between text-[11px] py-1 leading-relaxed">
      <span className="text-ink-muted">{label}</span>
      <span className={`font-mono text-[11px] ${valueClass}`}>{value}</span>
    </div>
  )
}

// ── Main Component ────────────────────────────────────

export default function AiCopilot() {
  const store = useDataStore()
  const [input, setInput] = useState('')
  const [messages, setMessages] = useState<DisplayMessage[]>([])
  const [streaming, setStreaming] = useState(false)
  const msgContainerRef = useRef<HTMLDivElement>(null)
  const textareaRef = useRef<HTMLTextAreaElement>(null)

  useEffect(() => { store.fetchOverview(); store.fetchIndexQuotes() }, [])

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

  const autoResize = useCallback(() => {
    const el = textareaRef.current
    if (!el) return
    el.style.height = 'auto'
    el.style.height = Math.min(el.scrollHeight, 120) + 'px'
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
      // reset textarea height
      if (textareaRef.current) { textareaRef.current.style.height = 'auto' }
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

  const clearChat = useCallback(() => setMessages([]), [])

  const isEmpty = messages.length === 0

  return (
    <div className="min-h-screen bg-bg pt-11 flex flex-col" style={{ height: 'calc(100vh - 44px)' }}>
      {/* Ticker Strip */}
      <TickerStrip />

      {/* Main Layout */}
      <div className="flex flex-1 overflow-hidden">
        {/* Chat Panel */}
        <div className="flex-1 flex flex-col min-w-0 overflow-hidden">
          <div ref={msgContainerRef} className="flex-1 overflow-y-auto px-6 py-5 flex flex-col gap-5">
            {isEmpty ? (
              <div className="flex-1 flex items-center justify-center">
                <div className="text-center max-w-[500px]">
                  <h2 className="text-2xl font-medium text-white mb-2">AI 行情助手 · A 股专版</h2>
                  <p className="text-sm text-ink-muted leading-relaxed mb-6">
                    覆盖上证、深证、北交所全市场数据。支持大盘研判、板块轮动、个股分析、资金面跟踪及策略问答，输出结构化分析报告。
                  </p>
                  <div className="flex flex-wrap gap-2 justify-center">
                    {quickPrompts.map((q) => (
                      <button
                        key={q.label}
                        onClick={() => send(q.text)}
                        className="inline-flex items-center gap-1.5 px-4 py-2 rounded-[20px] border border-hairline bg-bg-surface text-xs text-white hover:border-ai hover:text-ai transition-colors cursor-pointer whitespace-nowrap"
                      >
                        {q.icon} {q.label}
                      </button>
                    ))}
                  </div>
                </div>
              </div>
            ) : (
              messages.map((msg, idx) => {
                const isLastAi = msg.role === 'ai' && idx === messages.length - 1
                return (
                  <div
                    key={idx}
                    className={`flex gap-3 max-w-[820px] ${msg.role === 'user' ? 'self-end flex-row-reverse' : ''}`}
                    style={{ animation: 'msgIn 0.25s ease-out' }}
                  >
                    <span className={`w-8 h-8 rounded-full flex items-center justify-center text-[11px] font-semibold flex-shrink-0 ${
                      msg.role === 'user' ? 'bg-ai text-white' : 'bg-bg-hover text-white'
                    }`}>
                      {msg.role === 'user' ? 'U' : 'AI'}
                    </span>
                    <div className={`px-4 py-3 rounded-xl text-sm leading-relaxed ${
                      msg.role === 'user'
                        ? 'rounded-tr-sm max-w-[70%] bg-ink-dim text-white'
                        : 'rounded-tl-sm bg-bg-surface border border-hairline text-white/80'
                    }`}>
                      {msg.role === 'ai' ? (
                        <div className="ai-msg">
                          {msg.text ? (
                            <ReactMarkdown remarkPlugins={[remarkGfm]}>{msg.text}</ReactMarkdown>
                          ) : (
                            <div className="flex gap-1 py-1">
                              <span className="w-1.5 h-1.5 rounded-full bg-ink-muted" style={{ animation: 'dotPulse 1.2s infinite ease-in-out' }} />
                              <span className="w-1.5 h-1.5 rounded-full bg-ink-muted" style={{ animation: 'dotPulse 1.2s 0.2s infinite ease-in-out' }} />
                              <span className="w-1.5 h-1.5 rounded-full bg-ink-muted" style={{ animation: 'dotPulse 1.2s 0.4s infinite ease-in-out' }} />
                            </div>
                          )}
                          {isLastAi && streaming && msg.text && (
                            <span className="inline-block w-2 h-4 bg-ai ml-0.5 rounded-sm align-text-bottom" style={{ animation: 'blink 0.7s steps(1) infinite' }} />
                          )}
                        </div>
                      ) : (
                        msg.text
                      )}
                    </div>
                  </div>
                )
              })
            )}
          </div>

          {/* Input */}
          <div className="px-6 py-3.5 pb-5 bg-bg-surface border-t border-hairline">
            <div className="flex gap-2.5 items-end max-w-[820px]">
              {!isEmpty && (
                <button
                  onClick={clearChat}
                  className="px-3 py-2 rounded-lg border border-hairline text-xs text-ink-muted hover:text-white transition-colors cursor-pointer flex-shrink-0"
                >
                  清空对话
                </button>
              )}
              <textarea
                ref={textareaRef}
                value={input}
                onChange={(e) => { setInput(e.target.value); autoResize() }}
                className="flex-1 px-3.5 py-2.5 rounded-xl border border-hairline text-sm leading-relaxed resize-none outline-none bg-bg text-white placeholder-ink-muted focus:border-ai transition-colors font-sans"
                style={{ minHeight: 44, maxHeight: 120 }}
                placeholder="输入问题，例如：上证 3300 能否站稳？哪些板块在支撑？"
                rows={1}
                onKeyDown={onKeydown}
                disabled={streaming}
              />
              <button
                onClick={() => send()}
                disabled={streaming || !input.trim()}
                className="w-11 h-11 rounded-xl border-none bg-ai text-white cursor-pointer flex items-center justify-center flex-shrink-0 hover:opacity-90 transition-opacity disabled:opacity-30 disabled:cursor-not-allowed"
                title="发送"
              >
                <svg xmlns="http://www.w3.org/2000/svg" className="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round"><line x1="12" y1="19" x2="12" y2="5"/><polyline points="5 12 12 5 19 12"/></svg>
              </button>
            </div>
            <p className="text-center text-[11px] text-ink-muted mt-1.5">
              Enter 发送 · Shift+Enter 换行
            </p>
          </div>
        </div>

        {/* Context Sidebar */}
        <ContextSidebar />
      </div>

      <style>{`
        @keyframes msgIn { from { opacity: 0; transform: translateY(8px); } to { opacity: 1; transform: translateY(0); } }
        @keyframes dotPulse { 0%, 80%, 100% { opacity: 0.2; transform: scale(0.8); } 40% { opacity: 1; transform: scale(1.1); } }
        @keyframes blink { 50% { opacity: 0; } }
        .ai-msg h1, .ai-msg h2, .ai-msg h3 { font-weight: 500; color: var(--color-fg); margin: 14px 0 8px; }
        .ai-msg h1 { font-size: 1.25rem; }
        .ai-msg h2 { font-size: 1.05rem; }
        .ai-msg h3 { font-size: 0.9rem; }
        .ai-msg p { margin: 8px 0; }
        .ai-msg ul, .ai-msg ol { margin: 8px 0; padding-left: 22px; }
        .ai-msg li { margin: 4px 0; }
        .ai-msg code { background: rgb(var(--color-fg-rgb) / 0.06); padding: 2px 6px; border-radius: 4px; font-size: 0.75rem; }
        .ai-msg strong { color: var(--color-fg); }
        .ai-msg blockquote { border-left: 3px solid var(--color-accent); padding-left: 14px; margin: 10px 0; font-style: italic; }
        .ai-msg table { width: 100%; border-collapse: collapse; margin: 12px 0; font-size: 0.78rem; }
        .ai-msg th { text-align: left; padding: 8px 12px; background: rgb(var(--color-fg-rgb) / 0.04); border-bottom: 1px solid var(--color-border); font-weight: 500; font-size: 0.72rem; text-transform: uppercase; letter-spacing: 0.03em; }
        .ai-msg td { padding: 8px 12px; border-bottom: 1px solid var(--color-border); font-family: 'JetBrains Mono', monospace; font-size: 0.76rem; }
      `}</style>
    </div>
  )
}
