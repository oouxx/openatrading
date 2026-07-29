import { useCallback, useEffect, useRef, useState, startTransition } from "react"
import type { ReactNode } from "react"
import { Search, Send } from "lucide-react"
import TextBlock from "@/components/chat/TextBlock"
import ThinkingBlock from "@/components/chat/ThinkingBlock"
import ToolCallBlock from "@/components/chat/ToolCallBlock"
import ToolResultBlock from "@/components/chat/ToolResultBlock"
import SystemNoticeBlock from "@/components/chat/SystemNoticeBlock"
import AgentStatusBar from "@/components/chat/AgentStatusBar"
import TurnDivider from "@/components/chat/TurnDivider"
import type { AgentPhase } from "@/components/chat/AgentStatusBar"
import { SidebarTrigger } from "@/components/ui/sidebar"
import { Button } from "@/components/ui/button"
import { Textarea } from "@/components/ui/textarea"

import { fetchJson } from "@/stores/data"
import { fetchCommands, type ChatSession } from "@/hooks/useChatThreads"
import KlineDrawer from "@/components/KlineDrawer"
import ChromeTabs from "@/components/ChromeLikeTabs"
import { useStockTabStore } from "@/stores/stockTabStore"
import { useAgentWebSocket } from "@/agents/useAgentWebSocket"
import { TauriTransport, type AgentTransport } from "@/agents/transport"
import { applyEvent, historyToTurns, newTurnId } from "@/agents/reducer"
import type {
  HistoryMessage,
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

export default function AiCopilot({
  activeThreadId,
  onActiveThreadChange,
  onReload,
  sessions,
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
  // Ref to always have the latest sendPrompt from useAgentWebSocket hook
  const sendPromptRef = useRef<((text: string) => Promise<void>) | null>(null)
  const activeStockTab = useStockTabStore((s) => s.activeStockTab)
  const stockTabs = useStockTabStore((s) => s.stockTabs)
  const openTab = useStockTabStore((s) => s.openTab)
  const closeTab = useStockTabStore((s) => s.closeTab)
  const setActiveStockTab = useStockTabStore((s) => s.setActiveStockTab)

  // Agent lifecycle phase — derived from the latest turn's status.
  const [agentPhase, setAgentPhase] = useState<AgentPhase>("idle")

  // Keep a ref mirror so async transport callbacks read the latest turns.
  const threadTurnsRef = useRef<Map<string, TranscriptTurn[]>>(threadTurns)
  useEffect(() => {
    threadTurnsRef.current = threadTurns
  }, [threadTurns])

  // Derive agent phase from turns
  useEffect(() => {
    const turns = activeThreadId ? threadTurns.get(activeThreadId) : undefined
    if (!turns || turns.length === 0) {
      setAgentPhase("idle")
      return
    }
    const lastTurn = turns[turns.length - 1]
    if (lastTurn.isError) {
      setAgentPhase("error")
    } else if (lastTurn.status === "in_progress") {
      // Check if any tool is running or streaming
      const hasRunningTool = lastTurn.blocks.some(
        (b) => b.type === "tool_call" && (b.status === "running" || b.status === "streaming")
      )
      if (hasRunningTool) {
        setAgentPhase("processing")
      } else {
        setAgentPhase("starting")
      }
    } else {
      setAgentPhase("settled")
    }
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
  const prevInputRef = useRef("")

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

  // ── WebSocket transport (Web) via native WebSocket hook ──────────────
  const wsAgent = useAgentWebSocket(!isTauri ? activeThreadId : null, {
    onTurnsUpdate: useCallback(
      (turns: TranscriptTurn[]) => {
        if (isTauri || !activeThreadId) return
        threadTurnsRef.current.set(activeThreadId, turns)
        setThreadTurns(new Map(threadTurnsRef.current))
        scrollDown()
      },
      [activeThreadId, scrollDown],
    ),
    onTerminal: useCallback(() => {
      if (isTauri) return
      setStreaming(false)
      streamThreadRef.current = null
      setStreamThreadId(null)
      onReload?.()
    }, [onReload]),
  })

  // Keep a ref to the latest sendPrompt so the running send() closure
  // always uses the current hook instance (fixes race when creating a new session).
  useEffect(() => {
    sendPromptRef.current = wsAgent.sendPrompt
  }, [wsAgent.sendPrompt])

  // ── Tauri transport ──────────────────────────────────────────────────
  const startTauriTransport = useCallback(
    async (threadId: string) => {
      if (!isTauri) return
      transportRef.current?.dispose()
      const transport = new TauriTransport(threadId)
      transportRef.current = transport
      transport.onTurnsUpdate = (turns: TranscriptTurn[]) => {
        threadTurnsRef.current.set(threadId, turns)
        setThreadTurns(new Map(threadTurnsRef.current))
        scrollDown()
      }
      transport.onTerminal = () => {
        setStreaming(false)
        streamThreadRef.current = null
        setStreamThreadId(null)
        onReload?.()
      }
      await transport.start()
    },
    [scrollDown, onReload],
  )

  // ── Send ──────────────────────────────────────────────────────────────
  const send = useCallback(async () => {
    const text = input.trim()
    if (!text || !activeThreadId) return

    setInput("")
    setShowSlash(false)
    setStreaming(true)
    setAgentPhase("starting")
    streamThreadRef.current = activeThreadId
    setStreamThreadId(activeThreadId)

    if (isTauri) {
      await startTauriTransport(activeThreadId)
      transportRef.current?.sendPrompt(text)
    } else {
      await sendPromptRef.current?.(text)
    }
  }, [input, activeThreadId, startTauriTransport])

  // ── Permission response ──────────────────────────────────────────────
  const respondToPermission = useCallback(
    async (id: string, approved: boolean) => {
      if (isTauri) {
        await transportRef.current?.respondToPermission(id, approved)
      } else {
        await wsAgent.respondToPermission(id, approved)
      }
    },
    [wsAgent.respondToPermission],
  )

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
        el.style.height = `${Math.min(el.scrollHeight, 120)}px`
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
          const data = await fetchJson<any>(`/api/stocks/search?q=${encodeURIComponent(q)}`)
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
                  key={tab.code}
                  className={`rounded-md px-2 py-1 text-[11px] transition-colors ${
                    activeStockTab === tab.code
                      ? "bg-ai/15 text-ai"
                      : "text-ink-muted hover:bg-bg-hover"
                  }`}
                  onClick={() => setActiveStockTab(tab.code)}
                >
                  {tab.name}
                  <span
                    className="ml-1.5 text-[10px] opacity-50 hover:opacity-100"
                    onClick={(e) => {
                      e.stopPropagation()
                      closeTab(tab.code)
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
        <KlineDrawer />
      ) : (
        <div className="flex min-h-0 flex-1 flex-col">
          <div
            ref={msgContainerRef}
            className="flex flex-1 flex-col gap-5 overflow-y-auto px-6 py-5"
          >
            {isEmpty ? (
              <div className="flex flex-1 items-center justify-center">
                <div className="max-w-[500px] text-center">
                  <h2 className="mb-2 text-2xl font-medium text-white">AI 行情助手 · A 股专版</h2>
                  <p className="text-ink-muted mb-6 text-sm/relaxed">
                    覆盖上证、深证、北交所全市场数据。支持大盘研判、板块轮动、个股分析、资金面跟踪及策略问答。
                  </p>
                </div>
              </div>
            ) : (
              <>
                {/* ── Agent status bar ─────────────────────────────── */}
                <AgentStatusBar phase={agentPhase} />

                {currentTurns.map((turn, idx) => {
                  const isLastAi = turn.role === "assistant" && idx === currentTurns.length - 1
                  const isFirst = idx === 0

                  return (
                    <div key={turn.id}>
                      {/* ── Turn divider (between turns) ──────────── */}
                      {turn.role === "assistant" && !isFirst && (
                        <TurnDivider
                          label={turn.status === "complete" ? "Turn 结束" : undefined}
                          isError={turn.isError}
                        />
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
                                  : wsAgent.respondToPermission
                              }
                            />
                          )}
                        </div>
                      </div>
                    </div>
                  )
                })}
              </>
            )}
          </div>

          {/* ── Input area ─────────────────────────────────────────────── */}
          <div className="border-hairline shrink-0 border-t px-4 py-3">
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
                <Button
                  size="icon"
                  className="absolute bottom-1.5 right-1.5 size-8 rounded-lg"
                  onClick={() => send()}
                  disabled={!input.trim() || (streaming && streamThreadId === activeThreadId)}
                >
                  <Send className="size-4" />
                </Button>
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
          return (
            <TextBlock
              key={`t-${i}`}
              content={seg.block.content}
              isStreaming={isStreaming && seg.block.streaming}
            />
          )
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
