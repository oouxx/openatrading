import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react"
import { Search, Square, ArrowDown, Copy, Check } from "lucide-react"
import TextBlock from "@/components/chat/TextBlock"
import ThinkingBlock from "@/components/chat/ThinkingBlock"
import SystemNoticeBlock from "@/components/chat/SystemNoticeBlock"
import ToolCallList from "@/components/chat/ToolCallList"
import AgentStatusBar from "@/components/chat/AgentStatusBar"
import TurnDivider from "@/components/chat/TurnDivider"
import type { AgentPhase } from "@/components/chat/AgentStatusBar"
import type { HistoryMessage } from "@/agents/types"
import { SidebarTrigger } from "@/components/ui/sidebar"
import { Button } from "@/components/ui/button"
import { Textarea } from "@/components/ui/textarea"
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from "@/components/ui/command"

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

import { isTauri } from "@/lib/env"

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
  const slashMenuRef = useRef<HTMLDivElement | null>(null)

  const SCROLL_THRESHOLD = 60 // px — auto-scroll if user within this distance of bottom

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

  /** Unconditionally scroll to bottom — used after loading session history
   *  (switching sessions, page refresh) where content is replaced and the
   *  user's previous scroll position is no longer meaningful. */
  const scrollToBottomForced = useCallback(() => {
    requestAnimationFrame(() => {
      // Double rAF: React's async render may not have flushed the new turns
      // into the DOM by the first frame, so scroll after the next paint.
      requestAnimationFrame(() => {
        const el = msgContainerRef.current
        if (el) el.scrollTop = el.scrollHeight
      })
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
          scrollToBottomForced()
        })
        .catch(() => {})
    } else {
      wsAgent.loadHistory(activeThreadId).then(scrollToBottomForced)
    }
  }, [activeThreadId, wsAgent.loadHistory, scrollToBottomForced])

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
      try {
        await wsAgent.sendPrompt(sessionId, text)
      } catch (e) {
        // Connection couldn't be established — restore the input and stop
        // the streaming state so the user can retry instead of losing text.
        console.error("发送失败:", e)
        setInput(text)
        setStreaming(false)
        if (streamThreadRef.current === sessionId) {
          streamThreadRef.current = null
          setStreamThreadId(null)
        }
      }
    }
  }, [input, activeThreadId, onActiveThreadChange, startTauriTransport, wsAgent.sendPrompt])

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
    },
    [],
  )

  const onInputKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
      // Slash menu open: forward navigation keys to cmdk, Escape closes.
      if (showSlash && (e.key === "ArrowDown" || e.key === "ArrowUp" || e.key === "Enter")) {
        e.preventDefault()
        slashMenuRef.current?.dispatchEvent(
          new KeyboardEvent("keydown", { key: e.key, bubbles: true })
        )
        return
      }
      if (showSlash && e.key === "Escape") {
        setShowSlash(false)
        return
      }
      if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault()
        send()
      }
    },
    [send, showSlash],
  )

  // Pre-select the first command when the slash menu opens, so Enter works
  // immediately without an explicit ArrowDown first.
  useEffect(() => {
    if (showSlash) {
      slashMenuRef.current?.dispatchEvent(
        new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true })
      )
    }
  }, [showSlash])

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
  const isEmpty = currentTurns.length === 0 && !(streaming && streamThreadId === activeThreadId)
  // True only when the *active* thread is streaming — controls the Stop
  // button and disables Send for the active thread.
  const isStreamingActive = streaming && streamThreadId === activeThreadId
  // Show a "jump to latest" button when the user scrolls up during a stream.
  const [showJumpBottom, setShowJumpBottom] = useState(false)

  // Force-scroll to bottom when switching sessions. History loads
  // asynchronously, so keep scrolling until the new session's turns render.
  // useLayoutEffect runs synchronously after the DOM update, so scrollHeight
  // is already correct here — no rAF timing races.
  const lastSessionRef = useRef<string | null>(null)
  const pendingScrollRef = useRef(false)
  useLayoutEffect(() => {
    if (lastSessionRef.current !== activeThreadId) {
      lastSessionRef.current = activeThreadId
      pendingScrollRef.current = true
    }
    if (!pendingScrollRef.current) return
    const el = msgContainerRef.current
    if (el) el.scrollTop = el.scrollHeight
    if (currentTurns.length > 0) {
      pendingScrollRef.current = false
    }
  }, [activeThreadId, currentTurns])

  const onContainerScroll = useCallback((e: React.UIEvent<HTMLDivElement>) => {
    const el = e.currentTarget
    const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < SCROLL_THRESHOLD
    setShowJumpBottom(!atBottom && el.scrollHeight > el.clientHeight)
  }, [])

  // ── Input field (shared by the centered new-session hero and the docked
  //    bottom bar) so the textarea keeps one identity per layout branch.
  const renderInputField = () => (
    <div className="relative flex-1">
      <Textarea
        ref={textareaRef}
        className={`bg-surface border-hairline focus:border-brand/50 focus:ring-brand/20 min-h-[52px] max-h-[200px] w-full resize-none rounded-2xl border px-4 py-4 text-body text-primary shadow-card outline-none transition-all placeholder:text-tertiary focus:ring-3 ${
          isStreamingActive ? "pr-14" : "pr-4"
        }`}
        placeholder="有什么可以帮你？"
        rows={1}
        value={input}
        onChange={onInputChange}
        onKeyDown={onInputKeyDown}
      />
      {showSlash && (
        <div className="absolute bottom-full left-0 right-0 mb-1">
          <SlashCommandMenu
            commands={commands}
            filter={input.startsWith("/") ? input.slice(1) : ""}
            ref={slashMenuRef}
            onSelect={(cmd) => {
              setInput(`/${cmd.name} `)
              setShowSlash(false)
              textareaRef.current?.focus()
            }}
          />
        </div>
      )}
      {isStreamingActive && (
        <Button
          size="icon"
          variant="destructive"
          className="absolute bottom-2 right-2 size-9 rounded-lg"
          onClick={stop}
          aria-label="停止生成"
        >
          <Square className="size-4" fill="currentColor" />
        </Button>
      )}
    </div>
  )

  // ── Render ────────────────────────────────────────────────────────────
  return (
    <div className="flex h-full flex-col">
      {/* ── Header ──────────────────────────────────────────────────── */}
      <div className="border-hairline flex shrink-0 items-center gap-2 border-b px-4 py-2">
        <SidebarTrigger />
        <div className="flex flex-1 items-center gap-2">
          <div ref={searchContainerRef} className="relative flex-1">
            <Search className="text-secondary absolute left-3 top-1/2 size-4 -translate-y-1/2" />
            <input
              ref={searchRef}
              className="bg-canvas border-hairline focus:border-brand w-full rounded-lg border py-2 pl-9 pr-3 text-caption text-primary outline-none transition-colors placeholder:text-tertiary"
              placeholder="搜索股票代码或名称..."
              value={searchQuery}
              onChange={onSearchChange}
            />
            {showDropdown && (
              <div className="bg-surface border-hairline absolute left-0 right-0 top-full z-50 mt-1 max-h-48 overflow-y-auto rounded-lg border shadow-modal">
                {searchLoading ? (
                  <div className="text-secondary px-3 py-2 text-caption">搜索中...</div>
                ) : searchResults.length === 0 ? (
                  <div className="text-secondary px-3 py-2 text-caption">未找到结果</div>
                ) : (
                  searchResults.map((stock) => (
                    <button
                      key={stock.code}
                      className="hover:bg-hover flex w-full items-center gap-2 px-3 py-2 text-left text-caption transition-colors"
                      onClick={() => selectStock(stock.code, stock.name)}
                    >
                      <span className="text-secondary font-number">{stock.code}</span>
                      <span className="text-primary">{stock.name}</span>
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
                  className={`rounded-md px-2 py-1 text-caption transition-colors ${
                    activeStockTab === tab.id
                      ? "bg-brand/15 text-brand"
                      : "text-secondary hover:bg-hover"
                  }`}
                  onClick={() => setActiveStockTab(tab.id)}
                >
                  {tab.name}
                  <span
                    className="ml-1.5 text-caption opacity-50 hover:opacity-100"
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
      ) : isEmpty ? (
        // ── Centered new-session hero (Claude / ChatGPT style) ──────────
        <div className="flex min-h-0 flex-1 items-center justify-center px-6">
          <div className="readable-content">
            <h2 className="mb-6 text-center text-hero font-semibold text-primary">AI 行情助手</h2>
            {renderInputField()}
          </div>
        </div>
      ) : (
        <div className="relative flex min-h-0 flex-1 flex-col">
          <div
            ref={msgContainerRef}
            onScroll={onContainerScroll}
            className="flex flex-1 flex-col overflow-y-auto py-5"
          >
            <div className="readable-content flex flex-col gap-5 px-6 pb-40">
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
                        className={`flex size-8 shrink-0 items-center justify-center rounded-full text-caption font-semibold ${
                          turn.role === "user" ? "bg-brand text-primary" : "bg-hover text-primary"
                        }`}
                      >
                        {turn.role === "user" ? "U" : "AI"}
                      </span>
                      <div
                        className={`min-w-0 flex-1 text-body leading-relaxed ${
                          turn.role === "user" ? "flex items-center" : ""
                        }`}
                      >
                        {turn.role === "user" ? (
                          <p className="text-secondary">{turn.blocks.map((b) => ("content" in b ? b.content : "")).join("")}</p>
                        ) : (
                          <TurnRenderer
                            sessionId={activeThreadId}
                            turn={turn}
                            isStreaming={isLastAi && streaming && streamThreadId === activeThreadId}
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

              {/* ── Agent status bar (follows the latest content) ──── */}
              <AgentStatusBar phase={agentPhase} />
            </div>
          </div>

          {/* ── Jump to latest ──────────────────────────────────────────── */}
          {showJumpBottom && (
            <button
              onClick={() => {
                const el = msgContainerRef.current
                if (el) el.scrollTop = el.scrollHeight
                setShowJumpBottom(false)
              }}
              className="bg-surface border-hairline hover:border-brand/40 hover:text-brand absolute bottom-32 left-1/2 z-20 flex -translate-x-1/2 items-center gap-1 rounded-full border px-3 py-2 text-caption text-secondary shadow-modal transition-colors"
            >
              <ArrowDown className="size-3" />
              回到最新
            </button>
          )}

          {/* ── Floating input (Claude style) ─────────────────────────── */}
          <div className="pointer-events-none absolute inset-x-0 bottom-0 z-10 bg-linear-to-t from-canvas via-canvas/90 to-transparent px-6 pt-12 pb-4">
            <div className="pointer-events-auto readable-content flex items-end gap-2">
              {renderInputField()}
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
  filter,
  onSelect,
  ref,
}: {
  commands: SlashCommand[]
  filter: string
  onSelect: (cmd: SlashCommand) => void
  ref?: React.Ref<HTMLDivElement>
}) {
  const groups = useMemo(() => {
    const map = new Map<string, SlashCommand[]>()
    for (const cmd of commands) {
      const key = cmd.command_type || "Slash"
      const list = map.get(key) ?? []
      list.push(cmd)
      map.set(key, list)
    }
    return [...map.entries()]
  }, [commands])

  if (groups.length === 0) return null

  const groupLabel = (type: string) =>
    type === "Skill" ? "技能" : type === "Slash" ? "基础命令" : type

  return (
    <Command
      ref={ref}
      className="bg-surface border-hairline rounded-lg! max-h-48 overflow-hidden border p-0 shadow-modal"
    >
      {/* Hidden input drives cmdk's built-in filtering from the textarea value. */}
      <div className="hidden">
        <CommandInput value={filter} onValueChange={() => {}} />
      </div>
      <CommandList className="max-h-48 p-0">
        <CommandEmpty className="text-secondary py-3 text-caption">无匹配命令</CommandEmpty>
        {groups.map(([type, cmds]) => (
          <CommandGroup key={type} heading={groupLabel(type)} className="p-0">
            {cmds.map((cmd) => (
              <CommandItem
                key={cmd.name}
                value={cmd.name}
                onSelect={() => onSelect(cmd)}
                className="hover:bg-hover data-selected:bg-hover data-selected:text-primary flex w-full items-center gap-2 rounded-none px-3 py-2 text-left text-caption transition-colors [&>svg]:hidden"
              >
                <span className="font-number text-brand">/{cmd.name}</span>
                <span className="text-secondary">{cmd.description}</span>
              </CommandItem>
            ))}
          </CommandGroup>
        ))}
      </CommandList>
    </Command>
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
  // Tool calls render as compact inline rows (Claude web style). Consecutive
  // tool calls within a turn are grouped into one list so they stack tightly.
  const nodes: React.ReactNode[] = []
  let toolGroup: Extract<TranscriptBlock, { type: "tool_call" }>[] = []
  let idx = 0

  const flushTools = () => {
    if (toolGroup.length === 0) return
    nodes.push(
      <ToolCallList
        key={`tools-${idx++}`}
        toolCalls={toolGroup.map((tc) => ({
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
    toolGroup = []
  }

  for (const block of turn.blocks) {
    if (block.type === "tool_call") {
      toolGroup.push(block)
      continue
    }
    flushTools()

    if (block.type === "thinking") {
      nodes.push(
        <ThinkingBlock
          key={`t-${idx++}`}
          content={block.content}
          isStreaming={isStreaming && block.streaming}
        />
      )
    } else if (block.type === "text") {
      nodes.push(
        <TextSegment
          key={`t-${idx++}`}
          content={block.content}
          isStreaming={isStreaming && block.streaming}
        />
      )
    } else if (block.type === "system_notice" && block.kind !== "turn") {
      // Only non-turn system notices (compaction / retry / error) render in
      // the transcript. The "turn" kind is a concatenated tool-result preview
      // — redundant now that each tool row expands to show its own result.
      nodes.push(
        <SystemNoticeBlock
          key={`t-${idx++}`}
          notices={[
            {
              id: block.id,
              kind: block.kind,
              detail: block.detail,
              isError: block.isError,
              status: block.status,
              attempt: block.attempt,
              maxAttempts: block.maxAttempts,
              delayMs: block.delayMs,
            },
          ]}
        />
      )
    } else if (block.type === "permission_request") {
      nodes.push(
        <PermissionRequestCard
          key={`t-${idx++}`}
          block={block}
          respondToPermission={respondToPermission}
        />
      )
    }
  }
  flushTools()

  return <>{nodes}</>
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
          className="text-secondary/60 hover:text-primary absolute -top-2 right-0 flex items-center gap-1 rounded px-2 py-0.5 text-caption opacity-0 transition-opacity group-hover/seg:opacity-100"
          aria-label="复制"
        >
          {copied ? <Check className="size-3 text-success" /> : <Copy className="size-3" />}
          {copied ? "已复制" : "复制"}
        </button>
      )}
    </div>
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
    <div className="border-warning/30 bg-warning/10 my-2 overflow-hidden rounded-lg border">
      <div className="flex items-center gap-2 px-3 py-2">
        <span className="text-warning text-caption font-medium">需要确认</span>
      </div>
      <div className="border-hairline border-t px-3 py-2">
        <div className="text-secondary font-number text-caption">{block.toolName}</div>
        <pre className="text-secondary mt-1 overflow-x-auto font-number text-caption whitespace-pre-wrap">
          {JSON.stringify(block.toolInput, null, 2)}
        </pre>
      </div>
      <div className="border-hairline flex items-center justify-end gap-2 border-t px-3 py-2">
        <button
          className="rounded-md bg-danger/10 px-3 py-1 text-caption text-danger transition-colors hover:bg-danger/20"
          onClick={handleReject}
        >
          拒绝
        </button>
        <button
          className="rounded-md bg-success/10 px-3 py-1 text-caption text-success transition-colors hover:bg-success/20"
          onClick={handleApprove}
        >
          允许
        </button>
      </div>
    </div>
  )
}

// ── Text segment with hover copy (#3) ─────────────────────────────────────
