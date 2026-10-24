import {
  lazy,
  Suspense,
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react"
import { Search, Square, ArrowDown, ArrowUp, Paperclip, X } from "lucide-react"
import SystemNoticeBlock from "@/components/chat/SystemNoticeBlock"
import AgentStatusBar from "@/components/chat/AgentStatusBar"
import ActivityTimeline, { type ActivityItem } from "@/components/chat/ActivityTimeline"
import { summarizeTool } from "@/lib/tool-summary"
import type { AgentPhase } from "@/components/chat/AgentStatusBar"
import type { HistoryMessage } from "@/agents/types"
import { SidebarTrigger } from "@/components/ui/sidebar"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
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
import { fetchCommands } from "@/hooks/useChatThreads"
import { Skeleton } from "@/components/ui/skeleton"
import { useStockTabStore } from "@/stores/stockTabStore"
import { useMultiAgentWebSocket } from "@/agents/useMultiAgentWebSocket"
import { TauriTransport, type AgentTransport } from "@/agents/transport"
import { applyEvent, historyToTurns } from "@/agents/reducer"
import type { PromptImage, TranscriptBlock, TranscriptTurn } from "@/agents/types"

const TextBlock = lazy(() => import("@/components/chat/TextBlock"))
const KlineDrawer = lazy(() => import("@/components/KlineDrawer"))

interface SlashCommand {
  name: string
  description: string
  command_type: string
}

interface StockSearchHit {
  code: string
  name: string
}

interface StockSearchResponse {
  results?: StockSearchHit[]
}

interface AiCopilotProps {
  activeThreadId: string | null
  onActiveThreadChange: (id: string | null) => void
  onReload?: () => void
}

/** Read a File as a data URL (base64). Rejects on read error. */
function readFileAsDataURL(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => resolve(reader.result as string)
    reader.onerror = () => reject(reader.error)
    reader.readAsDataURL(file)
  })
}

import { isTauri } from "@/lib/env"

export default function AiCopilot({
  activeThreadId,
  onActiveThreadChange,
  onReload,
}: AiCopilotProps) {
  const [input, setInput] = useState("")
  const [attachments, setAttachments] = useState<PromptImage[]>([])
  const fileInputRef = useRef<HTMLInputElement | null>(null)
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
  const searchSeqRef = useRef(0)
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
      .catch((error) => console.warn("[chat] 加载 slash commands 失败", error))
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
    ? [
        ...new Set([
          ...(activeThreadId ? [activeThreadId] : []),
          ...(streamThreadId ? [streamThreadId] : []),
        ]),
      ].filter(Boolean)
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
      [activeThreadId, scrollDown]
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
      [onReload]
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
        .catch((error) => console.warn("[chat] 加载历史消息失败", error))
    } else {
      wsAgent.loadHistory(activeThreadId).then(scrollToBottomForced)
    }
  }, [activeThreadId, wsAgent, scrollToBottomForced])

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
    [scrollDown, onReload]
  )

  // ── Send ──────────────────────────────────────────────────────────────
  /** Optimistically append the user's own turn (text + image thumbnails) so
   *  sending has immediate visual feedback before the assistant turn streams.
   *  The turn is replaced by persisted history once the agent settles. */
  const appendLocalUserTurn = useCallback(
    (sessionId: string, text: string, images: PromptImage[]) => {
      const current = threadTurnsRef.current
      const turns = current.get(sessionId) ?? []
      const now = Date.now()
      const blocks: TranscriptBlock[] = [
        ...images.map((img, i) => ({
          type: "image" as const,
          id: `${sessionId}-img-${now}-${i}`,
          data: img.data,
          mimeType: img.mimeType,
        })),
        ...(text
          ? [
              {
                type: "text" as const,
                id: `${sessionId}-user-${now}`,
                content: text,
                streaming: false,
              },
            ]
          : []),
      ]
      const next = [
        ...turns,
        {
          id: `${sessionId}-turn-${now}`,
          role: "user" as const,
          status: "complete" as const,
          blocks,
        },
      ]
      const updated = new Map(current)
      updated.set(sessionId, next)
      threadTurnsRef.current = updated
      setThreadTurns(updated)
      scrollDown()
    },
    [scrollDown]
  )

  const send = useCallback(
    async (override?: string) => {
      const text = (override ?? input).trim()
      const images = attachments
      if (!text && images.length === 0) return

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

      // Optimistically render the user's own message (text + image thumbnails)
      // so there's immediate feedback while the turn streams. History reload
      // replaces this turn once the agent settles.
      if (sessionId) {
        appendLocalUserTurn(sessionId, text, images)
      }

      setInput("")
      setAttachments([])
      setShowSlash(false)
      setStreaming(true)
      streamThreadRef.current = sessionId
      setStreamThreadId(sessionId)

      if (isTauri) {
        await startTauriTransport(sessionId)
        transportRef.current?.sendPrompt(text, images)
      } else {
        // Use wsAgent directly with the resolved sessionId so we don't
        // depend on wsAgent.sendPrompt being updated after onActiveThreadChange.
        try {
          await wsAgent.sendPrompt(sessionId, text, images)
        } catch (e) {
          // Connection couldn't be established — restore the input and stop
          // the streaming state so the user can retry instead of losing text.
          console.error("发送失败:", e)
          setInput(text)
          setAttachments(images)
          setStreaming(false)
          if (streamThreadRef.current === sessionId) {
            streamThreadRef.current = null
            setStreamThreadId(null)
          }
        }
      }
    },
    [
      input,
      attachments,
      activeThreadId,
      onActiveThreadChange,
      startTauriTransport,
      wsAgent,
      appendLocalUserTurn,
    ]
  )

  // ── Attachment handlers (multimodal) ─────────────────────────────────
  const handleAttachFiles = useCallback(async (e: React.ChangeEvent<HTMLInputElement>) => {
    const files = Array.from(e.target.files ?? [])
    e.target.value = ""
    if (files.length === 0) return
    const imgs: PromptImage[] = []
    for (const file of files) {
      if (!file.type.startsWith("image/")) continue
      try {
        const dataUrl = await readFileAsDataURL(file)
        const idx = dataUrl.indexOf(",")
        imgs.push({
          data: idx >= 0 ? dataUrl.slice(idx + 1) : dataUrl,
          mimeType: file.type || "image/png",
        })
      } catch {
        // Skip unreadable files.
      }
    }
    if (imgs.length > 0) setAttachments((prev) => [...prev, ...imgs])
  }, [])

  const removeAttachment = useCallback((index: number) => {
    setAttachments((prev) => prev.filter((_, i) => i !== index))
  }, [])

  // ── Stop / interrupt the active stream ───────────────────────────────
  const stop = useCallback(async () => {
    if (!activeThreadId) return
    if (isTauri) {
      await transportRef.current?.interrupt()
    } else {
      await wsAgent.interrupt(activeThreadId)
    }
  }, [wsAgent, activeThreadId])

  // ── Input handlers ────────────────────────────────────────────────────
  const onInputChange = useCallback((e: React.ChangeEvent<HTMLTextAreaElement>) => {
    const value = e.target.value
    setInput(value)

    // Slash command detection
    if (value.startsWith("/") && !value.includes(" ")) {
      setShowSlash(true)
    } else {
      setShowSlash(false)
    }
  }, [])

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
    [send, showSlash]
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
  const onSearchChange = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    const q = e.target.value
    setSearchQuery(q)
    const seq = ++searchSeqRef.current
    if (debounceRef.current) clearTimeout(debounceRef.current)
    if (q.length < 1) {
      setShowDropdown(false)
      setSearchResults([])
      setSearchLoading(false)
      return
    }
    debounceRef.current = setTimeout(async () => {
      if (seq !== searchSeqRef.current) return
      setSearchLoading(true)
      try {
        const data = await fetchJson<StockSearchResponse | StockSearchHit[]>(
          `/api/search-stocks?q=${encodeURIComponent(q)}`
        )
        if (seq !== searchSeqRef.current) return
        setSearchResults(Array.isArray(data) ? data : (data?.results ?? []))
        setShowDropdown(true)
      } catch (error) {
        if (seq !== searchSeqRef.current) return
        console.warn("[stock-search] 搜索失败", error)
        setSearchResults([])
      } finally {
        if (seq === searchSeqRef.current) setSearchLoading(false)
      }
    }, 200)
  }, [])

  useEffect(
    () => () => {
      if (debounceRef.current) clearTimeout(debounceRef.current)
      // Invalidate any in-flight search so it cannot update an unmounted view.
      searchSeqRef.current++
    },
    []
  )

  const selectStock = useCallback(
    (code: string, name: string) => {
      openTab(code, name)
      setShowDropdown(false)
      setSearchQuery("")
    },
    [openTab]
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
  const currentTurns = useMemo(
    () => (activeThreadId ? (threadTurns.get(activeThreadId) ?? []) : []),
    [activeThreadId, threadTurns]
  )
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
  //    Claude.ai 样式：大圆角输入框 + 输入内容时右下角圆形发送按钮。
  const renderInputField = () => (
    <div className="relative flex-1">
      {/* Attached image previews (multimodal) */}
      {attachments.length > 0 && (
        <div className="mb-2 flex flex-wrap gap-2">
          {attachments.map((img, i) => (
            <div key={i} className="relative">
              <img
                src={`data:${img.mimeType};base64,${img.data}`}
                alt={`附件 ${i + 1}`}
                className="border-hairline h-16 w-16 rounded-lg border object-cover shadow-sm"
              />
              <Button
                type="button"
                variant="ghost"
                size="icon-xs"
                onClick={() => removeAttachment(i)}
                className="bg-scrim-strong text-brand-foreground hover:bg-scrim absolute -top-1.5 -right-1.5 size-5 rounded-full"
                aria-label="移除附件"
              >
                <X />
              </Button>
            </div>
          ))}
        </div>
      )}
      <Textarea
        ref={textareaRef}
        className={`bg-surface focus:border-brand/60 focus:ring-brand/20 border-hairline text-body text-primary placeholder:text-tertiary shadow-card max-h-[200px] min-h-[56px] w-full resize-none rounded-lg border py-4 pr-16 pl-16 transition-colors outline-none focus:ring-3`}
        placeholder="有什么可以帮你？"
        rows={1}
        value={input}
        onChange={onInputChange}
        onKeyDown={onInputKeyDown}
      />
      {showSlash && (
        <div className="absolute right-0 bottom-full left-0 mb-1">
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
      {isStreamingActive ? (
        <Button
          size="icon"
          variant="destructive"
          className="absolute right-2 bottom-2 size-9 rounded-full"
          onClick={stop}
          aria-label="停止生成"
        >
          <Square className="size-4" fill="currentColor" />
        </Button>
      ) : (
        (input.trim() || attachments.length > 0) && (
          <Button
            size="icon-lg"
            onClick={() => send()}
            className="shadow-card absolute right-2 bottom-2 rounded-full"
            aria-label="发送"
          >
            <ArrowUp />
          </Button>
        )
      )}
      {/* Image attach (multimodal) — hidden input opened by the paperclip button. */}
      <input
        ref={fileInputRef}
        type="file"
        accept="image/*"
        multiple
        className="hidden"
        onChange={handleAttachFiles}
      />
      <Button
        type="button"
        variant="ghost"
        size="icon-lg"
        onClick={() => fileInputRef.current?.click()}
        className="text-secondary hover:text-brand hover:bg-hover absolute bottom-2 left-2 rounded-full"
        aria-label="添加图片"
        disabled={isStreamingActive}
      >
        <Paperclip className="size-4.5" />
      </Button>
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
            <Search className="text-secondary absolute top-1/2 left-3 size-4 -translate-y-1/2" />
            <Input
              ref={searchRef}
              className="bg-canvas border-hairline focus-visible:border-brand text-caption h-9 pl-9"
              placeholder="搜索股票代码或名称..."
              value={searchQuery}
              onChange={onSearchChange}
            />
            {showDropdown && (
              <div className="absolute top-full right-0 left-0 z-50 mt-1">
                <Command className="bg-surface border-hairline shadow-modal rounded-lg border">
                  <CommandList className="max-h-48">
                    {searchLoading ? (
                      <div className="text-secondary text-caption px-3 py-2">搜索中...</div>
                    ) : searchResults.length === 0 ? (
                      <CommandEmpty className="text-secondary text-caption py-3 text-center">
                        未找到结果
                      </CommandEmpty>
                    ) : (
                      <CommandGroup>
                        {searchResults.map((stock) => (
                          <CommandItem
                            key={stock.code}
                            value={`${stock.code} ${stock.name}`}
                            onSelect={() => selectStock(stock.code, stock.name)}
                            className="text-caption"
                          >
                            <span className="text-secondary font-number">{stock.code}</span>
                            <span className="text-primary">{stock.name}</span>
                          </CommandItem>
                        ))}
                      </CommandGroup>
                    )}
                  </CommandList>
                </Command>
              </div>
            )}
          </div>
          {/* Stock tabs */}
          {stockTabs.length > 0 && (
            <div className="flex items-center gap-1">
              {stockTabs.map((tab) => (
                <div
                  key={tab.id}
                  className={`flex items-center rounded-md ${
                    activeStockTab === tab.id ? "bg-brand/15" : ""
                  }`}
                >
                  <Button
                    type="button"
                    variant="ghost"
                    size="xs"
                    className={`text-caption rounded-r-none ${
                      activeStockTab === tab.id
                        ? "text-brand hover:bg-brand/15 hover:text-brand"
                        : "text-secondary hover:bg-hover"
                    }`}
                    onClick={() => setActiveStockTab(tab.id)}
                  >
                    {tab.name}
                  </Button>
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon-xs"
                    aria-label={`关闭 ${tab.name}`}
                    className="text-secondary hover:bg-hover rounded-l-none opacity-60 hover:opacity-100"
                    onClick={() => closeTab(tab.id)}
                  >
                    ×
                  </Button>
                </div>
              ))}
            </div>
          )}
        </div>
      </div>

      {/* ── Body ────────────────────────────────────────────────────── */}
      {activeStockTab ? (
        <Suspense
          fallback={
            <div className="p-4">
              <Skeleton className="h-24 w-full" />
            </div>
          }
        >
          <KlineDrawer
            symbol={activeStockTab}
            name={stockTabs.find((t) => t.id === activeStockTab)?.name ?? activeStockTab}
            onClose={() => setActiveStockTab(null)}
          />
        </Suspense>
      ) : isEmpty ? (
        // ── Centered new-session hero (Claude.ai 样式) ──────────
        <div className="flex min-h-0 flex-1 items-center justify-center px-6">
          <div className="w-full max-w-[48rem]">
            <h2 className="text-hero text-primary mb-6 text-center font-semibold">AI 行情助手</h2>
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
            {/* Claude.ai 消息列宽：固定 48rem 居中（不随宽屏放大） */}
            <div className="mx-auto flex w-full max-w-[48rem] flex-col gap-6 px-6 pb-40">
              {currentTurns.map((turn, idx) => {
                const isLastAi = turn.role === "assistant" && idx === currentTurns.length - 1

                return (
                  <div key={turn.id}>
                    {turn.role === "user" ? (
                      // Claude 样式：用户消息右侧气泡（无头像）
                      <div className="flex w-full justify-end">
                        <div className="bg-surface text-primary shadow-card max-w-[85%] rounded-lg px-4 py-2.5">
                          {turn.blocks.some((b) => b.type === "image") && (
                            <div className="mb-2 flex flex-wrap gap-2">
                              {turn.blocks.map((b) =>
                                b.type === "image" ? (
                                  <img
                                    key={b.id}
                                    src={`data:${b.mimeType};base64,${b.data}`}
                                    alt="图片"
                                    className="border-hairline h-24 w-24 rounded-lg border object-cover"
                                  />
                                ) : null
                              )}
                            </div>
                          )}
                          <p className="text-body leading-[1.7] whitespace-pre-wrap">
                            {turn.blocks.map((b) => ("content" in b ? b.content : "")).join("")}
                          </p>
                        </div>
                      </div>
                    ) : (
                      // 助手消息直排、无头像；角色靠左右版式区分，不再逐条打「AI 助手」标签
                      <div className="flex w-full flex-col">
                        <div className="text-body w-full min-w-0 leading-[1.7]">
                          <TurnRenderer
                            sessionId={activeThreadId}
                            turn={turn}
                            isStreaming={isLastAi && streaming && streamThreadId === activeThreadId}
                            respondToPermission={
                              isTauri
                                ? (id: string, approved: boolean) =>
                                    transportRef.current?.respondToPermission(id, approved)
                                : (id: string, approved: boolean) =>
                                    wsAgent.respondToPermission(activeThreadId ?? "", id, approved)
                            }
                          />
                        </div>
                      </div>
                    )}
                  </div>
                )
              })}

              {/* ── Agent status bar (follows the latest content) ──── */}
              <AgentStatusBar phase={agentPhase} />
            </div>
          </div>

          {/* ── Jump to latest ──────────────────────────────────────────── */}
          {showJumpBottom && (
            <Button
              variant="outline"
              size="sm"
              onClick={() => {
                const el = msgContainerRef.current
                if (el) el.scrollTop = el.scrollHeight
                setShowJumpBottom(false)
              }}
              className="bg-surface border-hairline shadow-modal hover:border-brand/40 hover:text-brand text-secondary absolute bottom-32 left-1/2 z-20 -translate-x-1/2 rounded-full"
            >
              <ArrowDown className="size-3" />
              回到最新
            </Button>
          )}

          {/* ── Floating input (Claude style) ─────────────────────────── */}
          <div className="from-canvas via-canvas/90 pointer-events-none absolute inset-x-0 bottom-0 z-10 bg-linear-to-t to-transparent px-6 pt-12 pb-4">
            <div className="pointer-events-auto mx-auto flex w-full max-w-[48rem] items-end gap-2">
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

  // Backend serializes CommandType as snake_case (`slash`/`skill`/`extension`);
  // compare case-insensitively so group headings always match.
  const groupLabel = (type: string) => {
    const t = type.toLowerCase()
    if (t === "skill") return "技能"
    if (t === "slash") return "基础命令"
    if (t === "extension") return "扩展命令"
    return type
  }

  return (
    <Command
      ref={ref}
      className="bg-popover text-popover-foreground border-hairline shadow-modal max-h-48 overflow-hidden rounded-lg! border p-0"
    >
      {/* Hidden input drives cmdk's built-in filtering from the textarea value. */}
      <div className="hidden">
        <CommandInput value={filter} onValueChange={() => {}} />
      </div>
      <CommandList className="max-h-48 p-0">
        <CommandEmpty className="text-secondary text-caption py-3">无匹配命令</CommandEmpty>
        {groups.map(([type, cmds]) => (
          <CommandGroup key={type} heading={groupLabel(type)} className="p-0">
            {cmds.map((cmd) => (
              <CommandItem
                key={cmd.name}
                value={cmd.name}
                onSelect={() => onSelect(cmd)}
                className="hover:bg-hover data-selected:bg-hover data-selected:text-primary text-caption flex w-full items-center gap-2 rounded-none px-3 py-2 text-left transition-colors [&>svg]:hidden"
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
  // Claude.ai 活动时间线：thinking 与 tool call 按发生顺序串在一条垂直
  // 时间线上；文字/系统通知/权限卡不在时间线内（flush 后独立渲染）。
  const nodes: React.ReactNode[] = []
  let activity: ActivityItem[] = []
  let idx = 0

  const flushActivity = () => {
    if (activity.length === 0) return
    nodes.push(
      <ActivityTimeline
        key={`activity-${idx++}`}
        items={activity}
        isStreaming={isStreaming}
        sessionId={sessionId}
      />
    )
    activity = []
  }

  for (const block of turn.blocks) {
    if (block.type === "thinking") {
      activity.push({
        type: "thinking",
        id: block.id,
        content: block.content,
        streaming: isStreaming && block.streaming,
      })
      continue
    }
    if (block.type === "tool_call") {
      activity.push({
        type: "tool",
        id: block.id,
        toolCall: {
          id: block.id,
          name: block.name,
          args: block.input,
          result: block.result,
          partialResult: block.partialResult,
          status: block.status === "done" ? "success" : block.status,
          output: block.output,
        },
      })
      continue
    }
    flushActivity()

    if (block.type === "text") {
      nodes.push(
        <Suspense key={`t-${idx++}`} fallback={<Skeleton className="my-2 h-4 w-2/3" />}>
          <TextBlock content={block.content} isStreaming={isStreaming && block.streaming} />
        </Suspense>
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
  flushActivity()

  return <>{nodes}</>
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
        <span className="text-warning text-caption font-medium">
          允许执行「{summarizeTool(block.toolName, JSON.stringify(block.toolInput))}」？
        </span>
      </div>
      <div className="border-hairline border-t px-3 py-2">
        <pre className="text-secondary font-number text-caption mt-1 overflow-x-auto whitespace-pre-wrap">
          {JSON.stringify(block.toolInput, null, 2)}
        </pre>
      </div>
      <div className="border-hairline flex items-center justify-end gap-2 border-t px-3 py-2">
        <Button variant="destructive" size="sm" onClick={handleReject}>
          拒绝
        </Button>
        <Button
          variant="ghost"
          size="sm"
          className="bg-success/10 text-success hover:bg-success/20 hover:text-success"
          onClick={handleApprove}
        >
          允许
        </Button>
      </div>
    </div>
  )
}

// ── Text segment with hover copy (#3) ─────────────────────────────────────
