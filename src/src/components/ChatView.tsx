import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import { Search, Send, Square, ArrowDown, Copy, Check } from "lucide-react"
import TextBlock from "@/components/chat/TextBlock"
import ThinkingBlock from "@/components/chat/ThinkingBlock"
import ToolCallBlock from "@/components/chat/ToolCallBlock"
import ToolResultBlock from "@/components/chat/ToolResultBlock"
import SystemNoticeBlock from "@/components/chat/SystemNoticeBlock"
import AgentStatusBar from "@/components/chat/AgentStatusBar"
import TurnDivider from "@/components/chat/TurnDivider"
import type { AgentPhase } from "@/components/chat/AgentStatusBar"
import type { HistoryMessage } from "@/agents/types"
import { SidebarTrigger } from "@/components/ui/sidebar"
import { Button } from "@/components/ui/button"
import { Textarea } from "@/components/ui/textarea"

import { fetchJson } from "@/stores/data"
import { fetchCommands, type ChatSession } from "@/hooks/useChatThreads"
import KlineDrawer from "@/components/KlineDrawer"
import { useStockTabStore } from "@/stores/stockTabStore"
import { useMultiAgentWebSocket } from "@/agents/useMultiAgentWebSocket"
import { TauriTransport, type AgentTransport } from "@/agents/transport"
import { applyEvent, historyToTurns } from "@/agents/reducer"
import type {
  TranscriptBlock,
  TranscriptTurn,
} from "@/agents/types"

interface SlashCommand {
  name: string
  description: string
  command_type: string
}

interface AiCopilotProps {
  activeThreadId: string | null
  onActiveThreadChange: (id: string | null) => void
  onReload?: () => void
  sessions?: ChatSession[]
}

const isTauri = "__TAURI_INTERNALS__" in window

// ── Empty-state starter prompts (clickable chips) ──────────────────────
const SUGGESTIONS = [
  "今天大盘怎么看？",
  "最近涨停板有哪些？",
  "分析一下贵州茅台",
  "最近资金面情况如何？",
  "板块轮动有什么方向？",
]


export default function AiCopilot({
  activeThreadId,
  onActiveThreadChange,
  onReload,
}: AiCopilotProps) {
  const [input, setInput] = useState("")
  const [streaming, setStreaming] = useState(false)
  const [commands, setCommands] = useState<SlashCommand[]>([])
  const [showSlash, setShowSlash] = useState(false)
  // Per-thread transcript turns — the single source of truth for rendering.
  const [threadTurns, setThreadTurns] = useState<Map<string, TranscriptTurn[]>>(new Map())
  const streamThreadRef = useRef<string | null>(null)
  const [streamThreadId, setStreamThreadId] = useState<string | null>(null)
  const transportRef = useRef<AgentTransport | null>(null)
  const activeStockTab = useStockTabStore((s) => s.activeStockTab)
  const stockTabs = useStockTabStore((s) => s.stockTabs)
  const openTab = useStockTabStore((s) => s.openTab)
  const closeTab = useStockTabStore((s) => s.closeTab)
  const setActiveStockTab = useStockTabStore((s) => s.setActiveStockTab)

  // Keep a ref mirror so async transport callbacks read the latest turns.
  const threadTurnsRef = useRef<Map<string, TranscriptTurn[]>>(threadTurns)
  useEffect(() => {
    threadTurnsRef.current = threadTurns
  }, [threadTurns])

  // Agent lifecycle phase — derived from the latest turn's status.
  const agentPhase = useMemo<AgentPhase>(() => {
    const turns = activeThreadId ? threadTurns.get(activeThreadId) : undefined
    if (!turns || turns.length === 0) return "idle"
    const lastTurn = turns[turns.length - 1]
    if (lastTurn.isError) return "error"
    if (lastTurn.status === "in_progress") {
      const hasRunningTool = lastTurn.blocks.some(
        (b) => b.type === "tool_call" && (b.status === "running" || b.status === "streaming")
      )
      return hasRunningTool ? "processing" : "starting"
    }
    return "settled"
  }, [threadTurns, activeThreadId])

  // Stock search
  const [searchQuery, setSearchQuery] = useState("")
  const [showDropdown, setShowDropdown] = useState(false)
  const [searchResults, setSearchResults] = useState<{ code: string; name: string }[]>([])
  const [searchLoading, setSearchLoading] = useState(false)
  const searchRef = useRef<HTMLInputElement>(null)
  const searchContainerRef = useRef<HTMLDivElement>(null)
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const msgContainerRef = useRef<HTMLDivElement>(null)
  const textareaRef = useRef<HTMLTextAreaElement | null>(null)

  const SCROLL_THRESHOLD = 60

  /** Scroll to bottom only if the user is already near the bottom (§6.2). */
  const scrollDown = useCallback(() => {
    requestAnimationFrame(() => {
      const el = msgContainerRef.current
      if (!el) return
      const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < SCROLL_THRESHOLD
      if (atBottom) {
        el.scrollTop = el.scrollHeight
      }
    })
  }, [])

  useEffect(() => {
    fetchCommands()
      .then(setCommands)
      .catch(() => {})
  }, [])

  // Tear down the live transport when the active thread changes or on unmount.
  useEffect(() => {
    return () => {
      transportRef.current?.dispose()
      transportRef.current = null
    }
  }, [])

  // ── WebSocket transport (Web) — multi-session ────────────────────────
  // Only open a live WebSocket for sessions that actually need one: the active
  // session and any session currently streaming in the background. Opening a
  // socket for every persisted session forced the backend to materialize every
  // session into the pool (LRU pollution) and fetched history for all of them
  // on connect. Idle sessions don't need a live socket — their history is
  // loaded on demand when the user switches to them.
  const allSessionIds = !isTauri
    ? [...new Set([
        ...(activeThreadId ? [activeThreadId] : []),
        ...(streamThreadId ? [streamThreadId] : []),
      ])].filter(Boolean)
    : []

  const wsAgent = useMultiAgentWebSocket(allSessionIds, {
    onTurnsUpdate: useCallback(
      (sessionId: string, turns: TranscriptTurn[]) => {
        if (isTauri) return
        threadTurnsRef.current.set(sessionId, turns)
        setThreadTurns(new Map(threadTurnsRef.current))
        if (sessionId === activeThreadId) {
          scrollDown()
        }
      },
      [activeThreadId, scrollDown],
    ),
    onTerminal: useCallback(
      (sessionId: string) => {
        if (isTauri) return
        setStreaming(false)
        if (sessionId === streamThreadRef.current) {
          streamThreadRef.current = null
          setStreamThreadId(null)
        }
        onReload?.()
      },
      [onReload],
    ),
  })

  // Load history messages when switching to a different session.
  // The WebSocket onopen only loads history once per connection, so
  // switching to an already-connected session needs an explicit fetch.
  useEffect(() => {
    if (!activeThreadId) return
    if (isTauri) {
      fetchJson<HistoryMessage[]>(`/api/ai/sessions/${activeThreadId}/messages`)
        .then((msgs) => {
          if (!msgs || msgs.length === 0) return
          const turns = historyToTurns(msgs)
          threadTurnsRef.current.set(activeThreadId, turns)
          setThreadTurns(new Map(threadTurnsRef.current))
        })
        .catch(() => {})
    } else {
      wsAgent.loadHistory(activeThreadId)
    }
  }, [activeThreadId, wsAgent.loadHistory])

  // ── Tauri transport ──────────────────────────────────────────────────
  const startTauriTransport = useCallback(
    async (threadId: string) => {
      if (!isTauri) return
      transportRef.current?.dispose()
      const transport = new TauriTransport(threadId)
      transportRef.current = transport
      transport.onEvent((event) => {
        const currentTurns = threadTurnsRef.current.get(threadId) ?? []
        const { turns: newTurns, terminal } = applyEvent(currentTurns, event)
        threadTurnsRef.current.set(threadId, newTurns)
        setThreadTurns(new Map(threadTurnsRef.current))
        scrollDown()
        if (terminal) {
          setStreaming(false)
          if (threadId === streamThreadRef.current) {
            streamThreadRef.current = null
            setStreamThreadId(null)
          }
          onReload?.()
        }
      })
      await transport.ready()
    },
    [scrollDown],
  )

  // ── Send ──────────────────────────────────────────────────────────────
  const send = useCallback(async (override?: string) => {
    const text = (override ?? input).trim()
    if (!text) return

    // If no active session, generate a client-side ID and use it directly.
    // The WebSocket endpoint will create the session if it doesn't exist.
    let sessionId = activeThreadId
    if (!sessionId) {
      sessionId = `sess_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`
      onActiveThreadChange(sessionId)
      // Yield to React so the re-render triggers useMultiAgentWebSocket
      // to create the WebSocket connection before we try to send.
      await new Promise((resolve) => setTimeout(resolve, 0))
    }

    setInput("")
    setShowSlash(false)
    setStreaming(true)
    streamThreadRef.current = sessionId
    setStreamThreadId(sessionId)

    if (isTauri) {
      await startTauriTransport(sessionId)
      transportRef.current?.sendPrompt(text)
    } else {
      // Use wsAgent directly with the resolved sessionId so we don't
      // depend on wsAgent.sendPrompt being updated after onActiveThreadChange.
      await wsAgent.sendPrompt(sessionId, text)
    }
  }, [input, activeThreadId, onActiveThreadChange, startTauriTransport, wsAgent.sendPrompt])

  // ── Permission response ──────────────────────────────────────────────
  const respondToPermission = useCallback(
    async (id: string, approved: boolean) => {
      if (isTauri) {
        await transportRef.current?.respondToPermission(id, approved)
      } else if (activeThreadId) {
        await wsAgent.respondToPermission(activeThreadId, id, approved)
      }
    },
    [wsAgent.respondToPermission, activeThreadId],
  )

  // ── Stop / interrupt the active stream ───────────────────────────────
  const stop = useCallback(async () => {
    if (!activeThreadId) return
    if (isTauri) {
      await transportRef.current?.interrupt()
    } else {
      await wsAgent.interrupt(activeThreadId)
    }
  }, [wsAgent.interrupt, activeThreadId])

  // ── Input handlers ────────────────────────────────────────────────────
  const onInputChange = useCallback(
    (e: React.ChangeEvent<HTMLTextAreaElement>) => {
      const value = e.target.value
      setInput(value)

      // Slash command detection
      if (value.startsWith("/") && !value.includes(" ")) {
        setShowSlash(true)
      } else {
        setShowSlash(false)
      }

      // Auto-resize
      const el = textareaRef.current
      if (el) {
        el.style.height = "auto"
        el.style.height = `${Math.min(el.scrollHeight, 200)}px`
      }
    },
    [],
  )

  const onInputKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
      if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault()
        send()
      }
    },
    [send],
  )

  // ── Stock search ──────────────────────────────────────────────────────
  const onSearchChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const q = e.target.value
      setSearchQuery(q)
      if (debounceRef.current) clearTimeout(debounceRef.current)
      if (q.length < 1) {
        setShowDropdown(false)
        setSearchResults([])
        return
      }
      debounceRef.current = setTimeout(async () => {
        setSearchLoading(true)
        try {
          const data = await fetchJson<any>(`/api/search-stocks?q=${encodeURIComponent(q)}`)
          setSearchResults(data?.results ?? data ?? [])
          setShowDropdown(true)
        } catch {
          setSearchResults([])
        } finally {
          setSearchLoading(false)
        }
      }, 200)
    },
    [],
  )

  const selectStock = useCallback(
    (code: string, name: string) => {
      openTab(code, name)
      setShowDropdown(false)
      setSearchQuery("")
    },
    [openTab],
  )

  // Close dropdown on outside click
  useEffect(() => {
    const handleClick = (e: MouseEvent) => {
      if (searchContainerRef.current && !searchContainerRef.current.contains(e.target as Node)) {
        setShowDropdown(false)
      }
    }
    document.addEventListener("mousedown", handleClick)
    return () => document.removeEventListener("mousedown", handleClick)
  }, [])

  // ── Derived state ─────────────────────────────────────────────────────
  const currentTurns = activeThreadId ? threadTurns.get(activeThreadId) ?? [] : []
  const isEmpty = currentTurns.length === 0 && !streaming
  // True only when the *active* thread is streaming — controls the Stop
  // button and disables Send for the active thread.
  const isStreamingActive = streaming && streamThreadId === activeThreadId
  // Show a "jump to latest" button when the user scrolls up during a stream.
  const [showJumpBottom, setShowJumpBottom] = useState(false)

  const onContainerScroll = useCallback((e: React.UIEvent<HTMLDivElement>) => {
    const el = e.currentTarget
    const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < SCROLL_THRESHOLD
    setShowJumpBottom(!atBottom && el.scrollHeight > el.clientHeight)
  }, [])

  // ── Render ────────────────────────────────────────────────────────────
  return (
    <div className="flex h-full flex-col">
      {/* ── Header ──────────────────────────────────────────────────── */}
      <div className="border-hairline flex shrink-0 items-center gap-2 border-b px-4 py-2">
        <SidebarTrigger />
        <div className="flex flex-1 items-center gap-2">
          <div ref={searchContainerRef} className="relative flex-1">
            <Search className="text-ink-muted absolute left-3 top-1/2 size-4 -translate-y-1/2" />
            <input
              ref={searchRef}
              className="bg-bg border-hairline focus:border-ai w-full rounded-lg border py-1.5 pl-9 pr-3 text-xs text-white outline-none transition-colors placeholder:text-gray-500"
              placeholder="搜索股票代码或名称..."
              value={searchQuery}
              onChange={onSearchChange}
            />
            {showDropdown && (
              <div className="bg-bg-surface border-hairline absolute left-0 right-0 top-full z-50 mt-1 max-h-48 overflow-y-auto rounded-lg border shadow-lg">
                {searchLoading ? (
                  <div className="text-ink-muted px-3 py-2 text-xs">搜索中...</div>
                ) : searchResults.length === 0 ? (
                  <div className="text-ink-muted px-3 py-2 text-xs">未找到结果</div>
                ) : (
                  searchResults.map((stock) => (
                    <button
                      key={stock.code}
                      className="hover:bg-bg-hover flex w-full items-center gap-2 px-3 py-1.5 text-left text-xs transition-colors"
                      onClick={() => selectStock(stock.code, stock.name)}
                    >
                      <span className="text-ink-muted font-mono">{stock.code}</span>
                      <span className="text-white">{stock.name}</span>
                    </button>
                  ))
                )}
              </div>
            )}
          </div>
          {/* Stock tabs */}
          {stockTabs.length > 0 && (
            <div className="flex items-center gap-1">
              {stockTabs.map((tab) => (
                <button
                  key={tab.id}
                  className={`rounded-md px-2 py-1 text-[11px] transition-colors ${
                    activeStockTab === tab.id
                      ? "bg-ai/15 text-ai"
                      : "text-ink-muted hover:bg-bg-hover"
                  }`}
                  onClick={() => setActiveStockTab(tab.id)}
                >
                  {tab.name}
                  <span
                    className="ml-1.5 text-[10px] opacity-50 hover:opacity-100"
                    onClick={(e) => {
                      e.stopPropagation()
                      closeTab(tab.id)
                    }}
                  >
                    ×
                  </span>
                </button>
              ))}
            </div>
          )}
        </div>
      </div>

      {/* ── Body ────────────────────────────────────────────────────── */}
      {activeStockTab ? (
        <KlineDrawer
          symbol={activeStockTab}
          name={stockTabs.find((t) => t.id === activeStockTab)?.name ?? activeStockTab}
          onClose={() => setActiveStockTab(null)}
        />
      ) : (
        <div className="relative flex min-h-0 flex-1 flex-col">
          <div
            ref={msgContainerRef}
            onScroll={onContainerScroll}
            className="flex flex-1 flex-col overflow-y-auto py-5"
          >
            {isEmpty ? (
              <div className="flex flex-1 items-center justify-center px-6">
                <div className="max-w-[520px] text-center">
                  <h2 className="mb-2 text-2xl font-medium text-white">AI 行情助手 · A 股专版</h2>
                  <p className="text-ink-muted mb-6 text-sm/relaxed">
                    覆盖上证、深证、北交所全市场数据。支持大盘研判、板块轮动、个股分析、资金面跟踪及策略问答。
                  </p>
                  <div className="flex flex-wrap justify-center gap-2">
                    {SUGGESTIONS.map((q) => (
                      <button
                        key={q}
                        onClick={() => send(q)}
                        className="border-hairline hover:border-ai/50 hover:text-ai text-ink-muted rounded-full border px-3 py-1.5 text-xs transition-colors"
                      >
                        {q}
                      </button>
                    ))}
                  </div>
                </div>
              </div>
            ) : (
              <div className="mx-auto flex w-full max-w-3xl flex-col gap-5 px-6">
                {/* ── Agent status bar ─────────────────────────────── */}
                <AgentStatusBar phase={agentPhase} />

                {currentTurns.map((turn, idx) => {
                  const isLastAi = turn.role === "assistant" && idx === currentTurns.length - 1
                  const isFirst = idx === 0

                  return (
                    <div key={turn.id}>
                      {/* ── Turn divider (between turns) ──────────── */}
                      {turn.role === "assistant" && !isFirst && (
                        <TurnDivider isError={turn.isError} />
                      )}

                      <div
                        className={`flex w-full gap-3 ${turn.role === "user" ? "flex-row-reverse" : ""}`}
                      >
                        <span
                          className={`flex size-8 shrink-0 items-center justify-center rounded-full text-[11px] font-semibold ${
                            turn.role === "user" ? "bg-ai text-white" : "bg-bg-hover text-white"
                          }`}
                        >
                          {turn.role === "user" ? "U" : "AI"}
                        </span>
                        <div
                          className={`min-w-0 flex-1 text-sm leading-relaxed ${
                            turn.role === "user" ? "flex items-center" : ""
                          }`}
                        >
                          {turn.role === "user" ? (
                            <p className="text-ink-muted">{turn.blocks.map((b) => ("content" in b ? b.content : "")).join("")}</p>
                          ) : (
                            <TurnRenderer
                              sessionId={activeThreadId}
                              turn={turn}
                              isStreaming={isLastAi && streaming}
                              respondToPermission={
                                isTauri
                                  ? (id: string, approved: boolean) => transportRef.current?.respondToPermission(id, approved)
                                  : (id: string, approved: boolean) => wsAgent.respondToPermission(activeThreadId ?? "", id, approved)
                              }
                            />
                          )}
                        </div>
                      </div>
                    </div>
                  )
                })}
              </div>
            )}
          </div>

          {/* ── Jump to latest ──────────────────────────────────────────── */}
          {showJumpBottom && (
            <button
              onClick={() => {
                const el = msgContainerRef.current
                if (el) el.scrollTop = el.scrollHeight
                setShowJumpBottom(false)
              }}
              className="bg-bg-surface border-hairline hover:border-ai/40 hover:text-ai absolute bottom-4 left-1/2 z-10 flex -translate-x-1/2 items-center gap-1 rounded-full border px-3 py-1.5 text-[11px] text-ink-muted shadow-lg transition-colors"
            >
              <ArrowDown className="size-3" />
              回到最新
            </button>
          )}

          {/* ── Input area ─────────────────────────────────────────────── */}
          <div className="border-hairline shrink-0 border-t px-6 py-3">
            <div className="mx-auto flex max-w-3xl items-end gap-2">
              <div className="relative flex-1">
                <Textarea
                  ref={textareaRef}
                  className="bg-bg border-hairline focus:border-ai min-h-[44px] w-full resize-none rounded-xl border px-4 py-3 pr-12 text-sm text-white outline-none transition-colors placeholder:text-gray-500"
                  placeholder="输入消息..."
                  rows={1}
                  value={input}
                  onChange={onInputChange}
                  onKeyDown={onInputKeyDown}
                />
                {showSlash && (
                  <div className="absolute bottom-full left-0 right-0 mb-1">
                    <SlashCommandMenu
                      commands={commands}
                      onSelect={(cmd) => {
                        setInput(`/${cmd.name} `)
                        setShowSlash(false)
                        textareaRef.current?.focus()
                      }}
                    />
                  </div>
                )}
                {isStreamingActive ? (
                  <Button
                    size="icon"
                    variant="destructive"
                    className="absolute bottom-1.5 right-1.5 size-8 rounded-lg"
                    onClick={stop}
                    aria-label="停止生成"
                  >
                    <Square className="size-4" fill="currentColor" />
                  </Button>
                ) : (
                  <Button
                    size="icon"
                    className="absolute bottom-1.5 right-1.5 size-8 rounded-lg"
                    onClick={() => send()}
                    disabled={!input.trim()}
                    aria-label="发送"
                  >
                    <Send className="size-4" />
                  </Button>
                )}
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}

// ── Slash command menu ─────────────────────────────────────────────────────

function SlashCommandMenu({
  commands,
  onSelect,
}: {
  commands: SlashCommand[]
  onSelect: (cmd: SlashCommand) => void
}) {
  if (commands.length === 0) return null
  return (
    <div className="bg-bg-surface border-hairline rounded-card max-h-48 overflow-y-auto border shadow-lg">
      {commands.map((cmd) => (
        <button
          key={cmd.name}
          className="hover:bg-bg-hover flex w-full items-center gap-2 px-3 py-2 text-left text-xs transition-colors"
          onClick={() => onSelect(cmd)}
        >
          <span className="font-mono text-ai">/{cmd.name}</span>
          <span className="text-ink-muted">{cmd.description}</span>
        </button>
      ))}
    </div>
  )
}

// ── Turn renderer ─────────────────────────────────────────────────────────

interface TurnRendererProps {
  sessionId: string | null
  turn: TranscriptTurn
  isStreaming: boolean
  respondToPermission: (id: string, approved: boolean) => Promise<void>
}

function TurnRenderer({ sessionId, turn, isStreaming, respondToPermission }: TurnRendererProps) {
  const segments = groupBlocks(turn.blocks)

  return (
    <>
      {segments.map((seg, i) => {
        if (seg.kind === "thinking") {
          return (
            <ThinkingBlock
              key={`t-${i}`}
              content={seg.block.content}
              isStreaming={isStreaming && seg.block.streaming}
            />
          )
        }
        if (seg.kind === "text") {
          return <TextSegment key={`t-${i}`} content={seg.block.content} isStreaming={isStreaming && seg.block.streaming} />
        }
        if (seg.kind === "tools") {
          return (
            <ToolCallBlock
              key={`t-${i}`}
              toolCalls={seg.blocks.map((tc) => ({
                id: tc.id,
                name: tc.name,
                args: tc.input,
                result: tc.result,
                partialResult: tc.partialResult,
                status: tc.status === "done" ? "success" : tc.status,
                output: tc.output,
              }))}
              isStreaming={isStreaming}
              sessionId={sessionId}
            />
          )
        }
        if (seg.kind === "permission") {
          return (
            <PermissionRequestCard
              key={`t-${i}`}
              block={seg.block}
              respondToPermission={respondToPermission}
            />
          )
        }
        if (seg.kind === "system") {
          return (
            <SystemNoticeBlock
              key={`t-${i}`}
              notices={seg.blocks.map((b) => ({
                id: b.id,
                kind: b.kind as "compaction" | "retry" | "turn" | "notice",
                detail: b.detail,
                isError: b.isError,
                status: b.status as "active" | "done" | "error" | undefined,
                attempt: b.attempt,
                maxAttempts: b.maxAttempts,
                delayMs: b.delayMs,
              }))}
            />
          )
        }
        // results
        return (
          <ToolResultBlock
            key={`t-${i}`}
            results={seg.blocks.map((b) => ({
              name: b.kind === "notice" ? "提示" : "工具结果",
              result: b.detail,
              isError: b.isError,
            }))}
          />
        )
      })}
    </>
  )
}

// ── Permission request card (§11.2) ──────────────────────────────────────

interface PermissionRequestCardProps {
  block: Extract<TranscriptBlock, { type: "permission_request" }>
  respondToPermission: (id: string, approved: boolean) => Promise<void>
}

function PermissionRequestCard({ block, respondToPermission }: PermissionRequestCardProps) {
  const [resolved, setResolved] = useState(block.resolved)

  const handleApprove = async () => {
    if (resolved) return
    await respondToPermission(block.id, true)
    setResolved(true)
  }

  const handleReject = async () => {
    if (resolved) return
    await respondToPermission(block.id, false)
    setResolved(true)
  }

  if (resolved) return null

  return (
    <div className="border-amber-500/30 bg-amber-500/5 my-2 overflow-hidden rounded-lg border">
      <div className="flex items-center gap-2 px-3 py-2">
        <span className="text-amber-500 text-[11px] font-medium">需要确认</span>
      </div>
      <div className="border-hairline border-t px-3 py-2">
        <div className="text-ink-muted font-mono text-[10px]">{block.toolName}</div>
        <pre className="text-ink-muted mt-1 overflow-x-auto font-mono text-[10px] whitespace-pre-wrap">
          {JSON.stringify(block.toolInput, null, 2)}
        </pre>
      </div>
      <div className="border-hairline flex items-center justify-end gap-2 border-t px-3 py-2">
        <button
          className="rounded-md bg-red-500/10 px-3 py-1 text-[11px] text-red-500 transition-colors hover:bg-red-500/20"
          onClick={handleReject}
        >
          拒绝
        </button>
        <button
          className="rounded-md bg-emerald-500/10 px-3 py-1 text-[11px] text-emerald-500 transition-colors hover:bg-emerald-500/20"
          onClick={handleApprove}
        >
          允许
        </button>
      </div>
    </div>
  )
}

// ── Text segment with hover copy (#3) ─────────────────────────────────────

interface TextSegmentProps {
  content: string
  isStreaming: boolean
}

function TextSegment({ content, isStreaming }: TextSegmentProps) {
  const [copied, setCopied] = useState(false)
  const onCopy = useCallback(() => {
    navigator.clipboard.writeText(content).then(() => {
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    })
  }, [content])
  return (
    <div className="group/seg relative">
      <TextBlock content={content} isStreaming={isStreaming} />
      {!isStreaming && content.trim() && (
        <button
          onClick={onCopy}
          className="text-ink-muted/60 hover:text-ink absolute -top-2 right-0 flex items-center gap-1 rounded px-1.5 py-0.5 text-[10px] opacity-0 transition-opacity group-hover/seg:opacity-100"
          aria-label="复制"
        >
          {copied ? <Check className="size-3 text-emerald-500" /> : <Copy className="size-3" />}
          {copied ? "已复制" : "复制"}
        </button>
      )}
    </div>
  )
}

// ── Block grouping helpers ─────────────────────────────────────────────────

type BlockGroup =
  | { kind: "thinking"; block: Extract<TranscriptBlock, { type: "thinking" }>; blocks: Extract<TranscriptBlock, { type: "thinking" }>[] }
  | { kind: "text"; block: Extract<TranscriptBlock, { type: "text" }>; blocks: Extract<TranscriptBlock, { type: "text" }>[] }
  | { kind: "tools"; block?: undefined; blocks: Extract<TranscriptBlock, { type: "tool_call" }>[] }
  | { kind: "results"; block?: undefined; blocks: Extract<TranscriptBlock, { type: "system_notice" }>[] }
  | { kind: "system"; block?: undefined; blocks: Extract<TranscriptBlock, { type: "system_notice" }>[] }
  | { kind: "permission"; block: Extract<TranscriptBlock, { type: "permission_request" }>; blocks: Extract<TranscriptBlock, { type: "permission_request" }>[] }

function groupBlocks(blocks: TranscriptBlock[]): BlockGroup[] {
  const groups: BlockGroup[] = []
  for (const block of blocks) {
    if (block.type === "thinking") {
      groups.push({ kind: "thinking", block, blocks: [block] })
    } else if (block.type === "text") {
      groups.push({ kind: "text", block, blocks: [block] })
    } else if (block.type === "tool_call") {
      const last = groups[groups.length - 1]
      if (last && last.kind === "tools") {
        last.blocks.push(block)
      } else {
        groups.push({ kind: "tools", blocks: [block] })
      }
    } else if (block.type === "system_notice" && (block.kind === "compaction" || block.kind === "retry")) {
      // Compaction and retry blocks get their own "system" group
      const last = groups[groups.length - 1]
      if (last && last.kind === "system") {
        last.blocks.push(block)
      } else {
        groups.push({ kind: "system", blocks: [block] })
      }
    } else if (block.type === "system_notice") {
      // Regular notices (turn, notice) go to results
      groups.push({ kind: "results", blocks: [block] })
    } else if (block.type === "permission_request") {
      groups.push({ kind: "permission", block, blocks: [block] })
    }
  }
  return groups
}
