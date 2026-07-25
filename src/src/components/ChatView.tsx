import { useCallback, useEffect, useRef, useState, startTransition } from "react"
import type { ReactNode } from "react"
import { Search, Send, ToolCaseIcon } from "lucide-react"
import TextBlock from "@/components/chat/TextBlock"
import ThinkingBlock from "@/components/chat/ThinkingBlock"
import ToolCallBlock from "@/components/chat/ToolCallBlock"
import ToolResultBlock from "@/components/chat/ToolResultBlock"
import CompactionIndicator from "@/components/chat/CompactionIndicator"
import RetryIndicator from "@/components/chat/RetryIndicator"
import { SidebarTrigger } from "@/components/ui/sidebar"
import { Button } from "@/components/ui/button"
import { Textarea } from "@/components/ui/textarea"
import {
  Command,
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from "@/components/ui/command"
import { fetchJson } from "@/stores/data"
import { fetchCommands } from "@/hooks/useChatThreads"
import KlineDrawer from "@/components/KlineDrawer"
import ChromeTabs from "@/components/ChromeLikeTabs"
import { useStockTabStore } from "@/stores/stockTabStore"
import {
  createTransport,
  type AgentTransport,
} from "@/agents/transport"
import { applyEvent, historyToTurns } from "@/agents/reducer"
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

let turnSeq = 0
const newTurnId = () => `turn-${++turnSeq}`

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

  // Load persisted history when switching to an existing session (§7.3:
  // history is loaded via REST, the live transport only carries events after
  // connect).
  useEffect(() => {
    if (activeThreadId === null) return
    if (threadTurnsRef.current.has(activeThreadId)) {
      scrollDown()
      return
    }
    fetchJson<HistoryMessage[]>(`/api/ai/sessions/${activeThreadId}/messages`)
      .then((msgs) => {
        if (!msgs || msgs.length === 0) return
        const turns = historyToTurns(msgs)
        // If the session is currently running, add a placeholder in_progress
        // assistant turn so the UI shows "生成中..." until real events arrive (§7.4).
        const session = sessions?.find((s) => s.id === activeThreadId)
        if (session?.status === "running") {
          turns.push({
            id: newTurnId(),
            role: "assistant",
            status: "in_progress",
            blocks: [{
              type: "text",
              id: `blk-placeholder-${Date.now()}`,
              content: "",
              streaming: true,
            }],
          })
        }
        threadTurnsRef.current.set(activeThreadId, turns)
        setThreadTurns(new Map(threadTurnsRef.current))
        scrollDown()
      })
      .catch(() => {})
  }, [activeThreadId, scrollDown, sessions])

  // Stock search debounce
  useEffect(() => {
    if (debounceRef.current) clearTimeout(debounceRef.current)
    if (!searchQuery.trim()) {
      startTransition(() => setSearchResults([]))
      return
    }
    debounceRef.current = setTimeout(async () => {
      setSearchLoading(true)
      try {
        const data = await fetchJson<{ code: string; name: string }[]>(
          `/api/search-stocks?q=${encodeURIComponent(searchQuery.trim())}`
        )
        setSearchResults(data)
      } catch {
        setSearchResults([])
      } finally {
        setSearchLoading(false)
      }
    }, 200)
    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current)
    }
  }, [searchQuery])

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key === "p") {
        e.preventDefault()
        searchRef.current?.focus()
      }
    }
    window.addEventListener("keydown", handler)
    return () => window.removeEventListener("keydown", handler)
  }, [])

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (searchContainerRef.current && !searchContainerRef.current.contains(e.target as Node)) {
        setShowDropdown(false)
      }
    }
    document.addEventListener("mousedown", handler)
    return () => document.removeEventListener("mousedown", handler)
  }, [])

  const currentTurns = activeThreadId !== null ? (threadTurns.get(activeThreadId) ?? []) : []
  const isEmpty = currentTurns.length === 0
  const showStockTab = activeStockTab && stockTabs.some((t) => t.id === activeStockTab)

  let bodyContent: ReactNode | null = null
  if (showStockTab && activeStockTab) {
    const tab = stockTabs.find((t) => t.id === activeStockTab)
    bodyContent = (
      <div className="min-h-0 flex-1">
        {tab && (
          <div className="h-full overflow-y-auto">
            <KlineDrawer
              symbol={activeStockTab}
              name={tab.name}
              onClose={() => closeTab(activeStockTab)}
            />
          </div>
        )}
      </div>
    )
  }

  const send = useCallback(
    async (question?: string) => {
      const text = (question || input).trim()
      if (!text) return
      if (streaming && streamThreadRef.current === activeThreadId) return

      setShowSlash(false)
      setInput("")

      // For new threads, create a session first via the sessions API; the
      // session name is the user's first message (§12 title auto-generation).
      let sessionId = activeThreadId
      if (!sessionId) {
        try {
          const result = await fetchJson<{ id: string }>("/api/ai/sessions", {
            method: "POST",
            body: JSON.stringify({ name: text }),
          })
          sessionId = result.id
          onActiveThreadChange(sessionId)
        } catch (e) {
          const key = `_new_${Date.now()}`
          threadTurnsRef.current.set(key, [
            {
              id: newTurnId(),
              role: "assistant",
              status: "complete",
              isError: true,
              blocks: [
                {
                  type: "text",
                  id: `blk-err-${Date.now()}`,
                  content: `[错误] 创建会话失败: ${e}`,
                  streaming: false,
                },
              ],
            },
          ])
          setThreadTurns(new Map(threadTurnsRef.current))
          return
        }
      }

      const sid = sessionId
      // Append the user turn; the agent's first assistant turn is created by
      // the reducer when turn_start arrives.
      const userTurn: TranscriptTurn = {
        id: newTurnId(),
        role: "user",
        status: "complete",
        blocks: [{ type: "text", id: `blk-u-${Date.now()}`, content: text, streaming: false }],
      }
      const existing = threadTurnsRef.current.get(sid) ?? []
      threadTurnsRef.current.set(sid, [...existing, userTurn])
      setThreadTurns(new Map(threadTurnsRef.current))
      scrollDown()

      setStreaming(true)
      streamThreadRef.current = sid

      // Build the live transport for this session (§4). Dispose any previous
      // transport — only one live session is streamed at a time.
      transportRef.current?.dispose()
      const transport = createTransport(sid)
      transportRef.current = transport

      transport.onEvent((event) => {
        const prev = threadTurnsRef.current.get(sid) ?? []
        const { turns, terminal } = applyEvent(prev, event)
        threadTurnsRef.current.set(sid, turns)
        setThreadTurns(new Map(threadTurnsRef.current))
        if (terminal) {
          setStreaming(false)
          streamThreadRef.current = null
          transportRef.current?.dispose()
          transportRef.current = null
          onReload?.()
        }
      })

      try {
        await transport.ready()
        await transport.sendPrompt(text)
      } catch (e: unknown) {
        setStreaming(false)
        streamThreadRef.current = null
        transportRef.current?.dispose()
        transportRef.current = null
        const prev = threadTurnsRef.current.get(sid) ?? []
        threadTurnsRef.current.set(sid, [
          ...prev,
          {
            id: newTurnId(),
            role: "assistant",
            status: "complete",
            isError: true,
            blocks: [
              {
                type: "text",
                id: `blk-err-${Date.now()}`,
                content: `[错误] ${(e as Error).message || "网络请求失败"}`,
                streaming: false,
              },
            ],
          },
        ])
        setThreadTurns(new Map(threadTurnsRef.current))
      }
    },
    [activeThreadId, input, streaming, onActiveThreadChange, onReload, scrollDown]
  )

  const onInputKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
      if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault()
        send()
      }
    },
    [send]
  )

  const onInputChange = useCallback((e: React.ChangeEvent<HTMLTextAreaElement>) => {
    const val = e.target.value
    const prev = prevInputRef.current
    prevInputRef.current = val
    setInput(val)
    // Only trigger slash dialog when user actually types "/" (not when value already has "/" from command select)
    if (val.startsWith("/") && !prev.startsWith("/")) {
      setShowSlash(true)
    } else if (!val.startsWith("/")) {
      setShowSlash(false)
    }
  }, [])

  return (
    <div className="flex size-full flex-col">
      <div className="bg-bg-surface border-hairline flex shrink-0 items-center gap-3 border-b px-4 py-1.5">
        <SidebarTrigger className="shrink-0" />
        <div className="text-ink-muted flex shrink-0 items-center gap-1.5 text-xs whitespace-nowrap">
          <span className="font-medium text-white">OpenATrading</span>
        </div>
        <div className="flex flex-1 justify-center">
          <div className="relative w-full max-w-[200px]" ref={searchContainerRef}>
            <Search className="text-ink-muted pointer-events-none absolute top-1/2 left-2 size-3 -translate-y-1/2" />
            <input
              ref={searchRef}
              className="bg-bg border-hairline focus:border-ai placeholder:text-ink-muted h-7 w-full rounded-lg border pr-12 pl-7 text-xs text-white transition-colors outline-none"
              placeholder="搜索股票..."
              value={searchQuery}
              onChange={(e) => {
                setSearchQuery(e.target.value)
                setShowDropdown(true)
              }}
              onFocus={() => setShowDropdown(true)}
              onKeyDown={(e) => {
                if (e.key === "Escape") {
                  setShowDropdown(false)
                  searchRef.current?.blur()
                }
              }}
            />
            <span className="bg-bg rounded-badge text-ink-muted pointer-events-none absolute top-1/2 right-1.5 -translate-y-1/2 px-1 py-0.5 text-[9px] leading-none">
              ⌘P
            </span>
            {showDropdown && searchQuery.trim() && (
              <div className="bg-bg-surface border-hairline rounded-card absolute inset-x-0 top-full z-50 mt-0.5 max-h-72 overflow-y-auto border shadow-lg">
                {searchLoading && <div className="text-ink-muted px-3 py-2 text-xs">搜索中...</div>}
                {!searchLoading && searchResults.length === 0 && (
                  <div className="text-ink-muted px-3 py-2 text-xs">无匹配结果</div>
                )}
                {searchResults.map((s) => (
                  <button
                    key={s.code}
                    className="hover:bg-bg-hover flex w-full items-center gap-2 px-3 py-1.5 text-left text-xs transition-colors"
                    onClick={() => {
                      openTab(`${s.code}`, s.name)
                      setSearchQuery("")
                      setShowDropdown(false)
                      searchRef.current?.blur()
                    }}
                  >
                    <span className="font-medium text-white">{s.name}</span>
                    <span className="text-ink-muted font-mono">{s.code}</span>
                  </button>
                ))}
              </div>
            )}
          </div>
        </div>
        <div className="shrink-0">
          <button
            className="hover:bg-bg-hover rounded-button text-ink-muted flex size-6 items-center justify-center text-sm transition-colors hover:text-white"
            title="更多"
          >
            ⋯
          </button>
        </div>
      </div>

      <ChromeTabs
        tabs={stockTabs.map((t) => ({ id: t.id, label: t.name }))}
        activeTab={activeStockTab}
        onSelect={setActiveStockTab}
        onClose={closeTab}
      />

      {bodyContent ? (
        bodyContent
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
              currentTurns.map((turn, idx) => {
                const isLastAi = turn.role === "assistant" && idx === currentTurns.length - 1
                return (
                  <div
                    key={turn.id}
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
                        turn.role === "user" ? "flex items-center justify-end" : "text-white/90"
                      }`}
                    >
                      {turn.role === "assistant" ? (
                        <TurnView
                          turn={turn}
                          isStreaming={streaming && isLastAi}
                        />
                      ) : (
                        <div className="bg-ink-dim inline-block max-w-[80%] rounded-2xl rounded-tr-sm px-4 py-2.5 text-white">
                          {turn.blocks.map((b) =>
                            b.type === "text" ? b.content : null
                          )}
                        </div>
                      )}
                    </div>
                  </div>
                )
              })
            )}
          </div>

          {showSlash && (
            <CommandDialog
              open={showSlash}
              onOpenChange={(open) => {
                if (!open) {
                  setShowSlash(false)
                  requestAnimationFrame(() => textareaRef.current?.focus())
                }
              }}
            >
              <Command>
                <CommandInput placeholder="搜索命令..." />

                <CommandList>
                  <CommandEmpty>无匹配命令</CommandEmpty>
                  {(["tool", "skill", "slash"] as const).map((groupType) => {
                    const groupCmds = commands.filter((c) => c.command_type === groupType)
                    if (groupCmds.length === 0) return null
                    const headings: Record<string, string> = {
                      tool: "Tools",
                      skill: "Skills",
                      slash: "Slash Commands",
                    }
                    return (
                      <CommandGroup key={groupType} heading={headings[groupType]}>
                        {groupCmds.map((cmd) => (
                          <CommandItem
                            key={cmd.name}
                            onSelect={() => {
                              setInput(`/${cmd.name} `)
                              setShowSlash(false)
                              requestAnimationFrame(() => textareaRef.current?.focus())
                            }}
                          >
                            <ToolCaseIcon />
                            <span className="flex-1 truncate font-medium">{cmd.name}</span>
                            {cmd.description && (
                              <span className="text-ink-muted ml-auto max-w-[180px] truncate text-[10px]">
                                {cmd.description}
                              </span>
                            )}
                          </CommandItem>
                        ))}
                      </CommandGroup>
                    )
                  })}
                </CommandList>
              </Command>
            </CommandDialog>
          )}

          <div className="bg-bg-surface border-hairline shrink-0 border-t px-4 py-2">
            <div className="relative mx-auto max-w-[820px]">
              <Textarea
                ref={textareaRef}
                value={input}
                onChange={onInputChange}
                onKeyDown={onInputKeyDown}
                placeholder="输入问题，/ 触发命令..."
                disabled={streaming}
                className="max-h-[120px] min-h-[44px] resize-none pr-12 text-sm"
                rows={1}
              />
              <Button
                size="icon"
                className="absolute right-1.5 bottom-1.5 size-8"
                onClick={() => send()}
                disabled={streaming || !input.trim()}
              >
                <Send className="size-4" />
              </Button>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}

// ── Turn rendering (docs/agentTransport.md §6) ────────────────────────────
//
// Blocks are rendered in arrival order. Consecutive tool_call blocks are
// grouped into a single ToolCallBlock (matches the previous visual layout).
// compaction/retry system_notice blocks feed the indicator components;
// turn/notice notices render as result cards.

interface TurnViewProps {
  turn: TranscriptTurn
  isStreaming: boolean
  sessionId?: string
  transport?: AgentTransport | null
}

function TurnView({ turn, isStreaming, sessionId, transport }: TurnViewProps) {
  // Derive compaction / retry indicator state from the turn's system_notice
  // blocks (the reducer records lifecycle there).
  const compaction = lastNotice(turn, "compaction")
  const retry = lastNotice(turn, "retry")

  const compactionStatus =
    compaction?.status === "active"
      ? "compacting"
      : (compaction?.status ?? "idle")
  const retryStatus =
    retry?.status === "active"
      ? "retrying"
      : (retry?.status ?? "idle")

  // Group consecutive tool_call blocks for ToolCallBlock rendering.
  const segments: Array<
    | { kind: "thinking"; block: Extract<TranscriptBlock, { type: "thinking" }> }
    | { kind: "text"; block: Extract<TranscriptBlock, { type: "text" }> }
    | { kind: "tools"; blocks: Array<Extract<TranscriptBlock, { type: "tool_call" }>> }
    | { kind: "results"; blocks: Array<Extract<TranscriptBlock, { type: "system_notice" }>> }
    | { kind: "permission"; block: Extract<TranscriptBlock, { type: "permission_request" }> }
  > = []
  for (const block of turn.blocks) {
    const last = segments[segments.length - 1]
    if (block.type === "tool_call") {
      if (last?.kind === "tools") last.blocks.push(block)
      else segments.push({ kind: "tools", blocks: [block] })
    } else if (
      block.type === "system_notice" &&
      (block.kind === "turn" || block.kind === "notice")
    ) {
      if (last?.kind === "results") last.blocks.push(block)
      else segments.push({ kind: "results", blocks: [block] })
    } else if (block.type === "thinking") {
      segments.push({ kind: "thinking", block })
    } else if (block.type === "text") {
      segments.push({ kind: "text", block })
    } else if (block.type === "permission_request") {
      segments.push({ kind: "permission", block })
    }
    // compaction/retry system_notices are rendered via indicators below.
  }

  return (
    <>
      {turn.isError && (
        <div className="border-red-500/30 bg-red-500/5 mb-2 flex items-center gap-2 rounded-lg border px-3 py-2">
          <div className="flex size-5 items-center justify-center rounded-full bg-red-500/20">
            <span className="text-[10px] font-bold text-red-500">!</span>
          </div>
          <span className="text-ink-muted text-[11px]">此消息因错误已自动重试</span>
        </div>
      )}
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
              transport={transport}
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
      <CompactionIndicator
        status={compactionStatus}
        reason={compaction?.detail ?? ""}
        error={compaction?.isError ? compaction.detail : null}
      />
      <RetryIndicator
        status={retryStatus}
        attempt={retry?.attempt}
        maxAttempts={retry?.maxAttempts}
        delayMs={retry?.delayMs}
        error={retry?.isError ? retry.detail : null}
      />
    </>
  )
}

// ── Permission request card (§11.2) ──────────────────────────────────────

interface PermissionRequestCardProps {
  block: Extract<TranscriptBlock, { type: "permission_request" }>
  transport?: AgentTransport | null
}

function PermissionRequestCard({ block, transport }: PermissionRequestCardProps) {
  const [resolved, setResolved] = useState(block.resolved)

  const handleApprove = async () => {
    if (!transport || resolved) return
    await transport.respondToPermission(block.id, true)
    setResolved(true)
  }

  const handleReject = async () => {
    if (!transport || resolved) return
    await transport.respondToPermission(block.id, false)
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

function lastNotice(
  turn: TranscriptTurn,
  kind: "compaction" | "retry",
): Extract<TranscriptBlock, { type: "system_notice" }> | undefined {
  for (let i = turn.blocks.length - 1; i >= 0; i--) {
    const b = turn.blocks[i]
    if (b.type === "system_notice" && b.kind === kind) return b
  }
  return undefined
}
