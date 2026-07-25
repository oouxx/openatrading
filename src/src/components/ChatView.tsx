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
import { agentStream, fetchJson } from "@/stores/data"
import { fetchCommands } from "@/hooks/useChatThreads"
import KlineDrawer from "@/components/KlineDrawer"
import ChromeTabs from "@/components/ChromeLikeTabs"
import { useStockTabStore } from "@/stores/stockTabStore"

interface DisplayMessage {
  id: string
  role: "user" | "assistant"
  content: string
  reasoning?: string
  toolCalls?: {
    name: string
    id: string
    args?: string
    result?: string
    partialResult?: string
    status?: "streaming" | "running" | "success" | "error"
  }[]
  toolResults?: {
    name: string
    result: string
    isError?: boolean
  }[]
  isError?: boolean
}

interface SlashCommand {
  name: string
  description: string
  command_type: string
}

interface AiCopilotProps {
  activeThreadId: string | null
  onActiveThreadChange: (id: string | null) => void
  onReload?: () => void
}

let msgSeq = 0
const newMsgId = () => `msg-${++msgSeq}`

export default function AiCopilot({
  activeThreadId,
  onActiveThreadChange,
  onReload,
}: AiCopilotProps) {
  const [input, setInput] = useState("")
  const [streaming, setStreaming] = useState(false)
  const [commands, setCommands] = useState<SlashCommand[]>([])
  const [showSlash, setShowSlash] = useState(false)
  const [threadMessages, setThreadMessages] = useState<Map<string, DisplayMessage[]>>(new Map())
  const [compactionStatus, setCompactionStatus] = useState<"idle" | "compacting" | "done" | "error">("idle")
  const [compactionReason, setCompactionReason] = useState<string>("")
  const [compactionError, setCompactionError] = useState<string | null>(null)
  const [retryStatus, setRetryStatus] = useState<"idle" | "will-retry" | "retrying" | "done" | "error">("idle")
  const [retryAttempt, setRetryAttempt] = useState<number>(0)
  const [retryMaxAttempts, setRetryMaxAttempts] = useState<number>(0)
  const [retryDelayMs, setRetryDelayMs] = useState<number>(0)
  const [retryError, setRetryError] = useState<string | null>(null)
  const streamThreadRef = useRef<string | null>(null)
  const activeStockTab = useStockTabStore((s) => s.activeStockTab)
  const stockTabs = useStockTabStore((s) => s.stockTabs)
  const openTab = useStockTabStore((s) => s.openTab)
  const closeTab = useStockTabStore((s) => s.closeTab)
  const setActiveStockTab = useStockTabStore((s) => s.setActiveStockTab)

  // Per-thread message store — persists across thread switches
  const threadMessagesRef = useRef<Map<string, DisplayMessage[]>>(threadMessages)

  // Keep ref in sync with state (avoids accessing ref during render)
  useEffect(() => {
    threadMessagesRef.current = threadMessages
  }, [threadMessages])

  // Stock search
  const [searchQuery, setSearchQuery] = useState("")
  const [showDropdown, setShowDropdown] = useState(false)
  const [searchResults, setSearchResults] = useState<{ code: string; name: string }[]>([])
  const [searchLoading, setSearchLoading] = useState(false)
  const searchRef = useRef<HTMLInputElement>(null)
  const searchContainerRef = useRef<HTMLDivElement>(null)
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const msgContainerRef = useRef<HTMLDivElement>(null)
  const textareaRef = useRef<HTMLTextAreaElement>(null)
  const prevInputRef = useRef("")

  const scrollDown = useCallback(() => {
    requestAnimationFrame(() => {
      if (msgContainerRef.current) {
        msgContainerRef.current.scrollTop = msgContainerRef.current.scrollHeight
      }
    })
  }, [])

  useEffect(() => {
    fetchCommands()
      .then(setCommands)
      .catch(() => {})
  }, [])

  // Messages from previous turns are stored in pi-coding-agent's JSONL session file.
  // Load messages from session JSONL when switching to an existing session
  useEffect(() => {
    if (activeThreadId === null) return
    if (threadMessagesRef.current.has(activeThreadId)) {
      scrollDown()
      return
    }
    fetchJson<{ role: string; content: string }[]>(`/api/ai/sessions/${activeThreadId}/messages`)
      .then((msgs) => {
        if (!msgs || msgs.length === 0) return
        threadMessagesRef.current.set(
          activeThreadId,
          msgs.map((m) => ({
            id: newMsgId(),
            role: m.role === "user" ? "user" : "assistant",
            content: m.content,
          }))
        )
        setThreadMessages(new Map(threadMessagesRef.current))
        scrollDown()
      })
      .catch(() => {})
  }, [activeThreadId, scrollDown])

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

  const currentMessages = activeThreadId !== null ? (threadMessages.get(activeThreadId) ?? []) : []
  const isEmpty = currentMessages.length === 0
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

      // For new threads, create a session first via the sessions API
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
          setStreaming(false)
          const key = `_new_${Date.now()}`
          const msgs = threadMessagesRef.current.get(key) ?? []
          msgs.push({ id: newMsgId(), role: "assistant", content: `[错误] 创建会话失败: ${e}` })
          threadMessagesRef.current.set(key, msgs)
          setThreadMessages(new Map(threadMessagesRef.current))
          return
        }
      }

      const userMsg: DisplayMessage = { id: newMsgId(), role: "user", content: text }
      const aiMsg: DisplayMessage = { id: newMsgId(), role: "assistant", content: "" }

      const existing = threadMessagesRef.current.get(sessionId) ?? []
      threadMessagesRef.current.set(sessionId, [...existing, userMsg, aiMsg])
      setThreadMessages(new Map(threadMessagesRef.current))
      scrollDown()

      setStreaming(true)
      streamThreadRef.current = sessionId

      try {
        await agentStream(
          {
            message: text,
            session_id: sessionId,
          },
          {
            onText: (data) => {
              const msgs = threadMessagesRef.current.get(sessionId) ?? []
              const last = msgs[msgs.length - 1]
              if (last && last.role === "assistant") {
                msgs[msgs.length - 1] = { ...last, content: last.content + data }
                threadMessagesRef.current.set(sessionId, msgs)
                setThreadMessages(new Map(threadMessagesRef.current))
                if (activeThreadId === sessionId) scrollDown()
              }
            },
            onReasoning: (data) => {
              const msgs = threadMessagesRef.current.get(sessionId) ?? []
              const last = msgs[msgs.length - 1]
              if (last && last.role === "assistant") {
                msgs[msgs.length - 1] = { ...last, reasoning: (last.reasoning ?? "") + data }
                threadMessagesRef.current.set(sessionId, msgs)
                setThreadMessages(new Map(threadMessagesRef.current))
              }
            },
            onToolCall: (tool) => {
              const msgs = threadMessagesRef.current.get(sessionId) ?? []
              const last = msgs[msgs.length - 1]
              if (last && last.role === "assistant") {
                const calls = last.toolCalls ?? []
                msgs[msgs.length - 1] = {
                  ...last,
                  toolCalls: [
                    ...calls,
                    { name: tool.name, id: tool.id, args: tool.args, status: "streaming" },
                  ],
                }
                threadMessagesRef.current.set(sessionId, msgs)
                setThreadMessages(new Map(threadMessagesRef.current))
              }
            },
            onToolDelta: (delta) => {
              const msgs = threadMessagesRef.current.get(sessionId) ?? []
              const last = msgs[msgs.length - 1]
              if (last && last.role === "assistant") {
                const calls = last.toolCalls ?? []
                const idx = calls.findIndex((tc) => tc.id === delta.id)
                if (idx !== -1) {
                  calls[idx] = {
                    ...calls[idx],
                    args: (calls[idx].args ?? "") + delta.args,
                    status: "streaming",
                  }
                  msgs[msgs.length - 1] = { ...last, toolCalls: [...calls] }
                  threadMessagesRef.current.set(sessionId, msgs)
                  setThreadMessages(new Map(threadMessagesRef.current))
                }
              }
            },
            onToolResult: (result) => {
              const msgs = threadMessagesRef.current.get(sessionId) ?? []
              const last = msgs[msgs.length - 1]
              if (last && last.role === "assistant") {
                const calls = last.toolCalls ?? []
                const idx = calls.findIndex((tc) => tc.id === result.id)
                if (idx !== -1) {
                  calls[idx] = { ...calls[idx], result: result.result, status: "success" }
                  msgs[msgs.length - 1] = { ...last, toolCalls: [...calls] }
                  threadMessagesRef.current.set(sessionId, msgs)
                  setThreadMessages(new Map(threadMessagesRef.current))
                }
              }
            },
            onToolExecUpdate: (update) => {
              const msgs = threadMessagesRef.current.get(sessionId) ?? []
              const last = msgs[msgs.length - 1]
              if (last && last.role === "assistant") {
                const calls = last.toolCalls ?? []
                const idx = calls.findIndex((tc) => tc.id === update.id)
                if (idx !== -1) {
                  calls[idx] = {
                    ...calls[idx],
                    partialResult: update.partial_result,
                    status: "running",
                  }
                  msgs[msgs.length - 1] = { ...last, toolCalls: [...calls] }
                  threadMessagesRef.current.set(sessionId, msgs)
                  setThreadMessages(new Map(threadMessagesRef.current))
                }
              }
            },
            onTurnStart: () => {
              // Always start a new assistant message for the next turn
              const msgs = threadMessagesRef.current.get(sessionId) ?? []
              const newMsg: DisplayMessage = { id: newMsgId(), role: "assistant", content: "" }
              msgs.push(newMsg)
              threadMessagesRef.current.set(sessionId, msgs)
              setThreadMessages(new Map(threadMessagesRef.current))
              scrollDown()
            },
            onTurnEnd: (data) => {
              // Store tool results as structured data on the current message
              const msgs = threadMessagesRef.current.get(sessionId) ?? []
              const last = msgs[msgs.length - 1]
              if (last && last.role === "assistant" && data.tool_results.length > 0) {
                const results = data.tool_results.map((r) => ({
                  name: "工具结果",
                  result: r,
                  isError: false,
                }))
                msgs[msgs.length - 1] = {
                  ...last,
                  toolResults: [...(last.toolResults ?? []), ...results],
                }
                threadMessagesRef.current.set(sessionId, msgs)
                setThreadMessages(new Map(threadMessagesRef.current))
                scrollDown()
              }
            },
            onRetry: () => {
              // Finalize current message as error and prepare for retry
              const msgs = threadMessagesRef.current.get(sessionId) ?? []
              const last = msgs[msgs.length - 1]
              if (last && last.role === "assistant") {
                msgs[msgs.length - 1] = { ...last, isError: true }
                threadMessagesRef.current.set(sessionId, msgs)
                setThreadMessages(new Map(threadMessagesRef.current))
              }
              setRetryStatus("will-retry")
            },
            onCompactionStart: (reason) => {
              setCompactionStatus("compacting")
              setCompactionReason(reason)
              setCompactionError(null)
            },
            onCompactionEnd: (data) => {
              setCompactionStatus(data.error ? "error" : "done")
              setCompactionError(data.error)
              if (data.willRetry) {
                setRetryStatus("will-retry")
              }
            },
            onRetryStart: (data) => {
              setRetryStatus("retrying")
              setRetryAttempt(data.attempt)
              setRetryMaxAttempts(data.maxAttempts)
              setRetryDelayMs(data.delayMs)
              setRetryError(data.error)
            },
            onRetryEnd: (data) => {
              setRetryStatus(data.success ? "done" : "error")
              setRetryError(data.finalError)
            },
            onSettled: () => {
              setCompactionStatus("idle")
              setRetryStatus("idle")
            },
            onDone: () => {
              setStreaming(false)
              setCompactionStatus("idle")
              setRetryStatus("idle")
              onReload?.()
            },
            onError: (err) => {
              setStreaming(false)
              const msgs = threadMessagesRef.current.get(sessionId) ?? []
              const last = msgs[msgs.length - 1]
              if (last && last.role === "assistant") {
                msgs[msgs.length - 1] = { ...last, content: last.content + `\n\n[错误] ${err}` }
                threadMessagesRef.current.set(sessionId, msgs)
                setThreadMessages(new Map(threadMessagesRef.current))
              }
            },
          }
        )
      } catch (e: unknown) {
        setStreaming(false)
        const msgs = threadMessagesRef.current.get(sessionId) ?? []
        const last = msgs[msgs.length - 1]
        if (last && last.role === "assistant") {
          msgs[msgs.length - 1] = {
            ...last,
            content: `[错误] ${(e as Error).message || "网络请求失败"}`,
          }
          threadMessagesRef.current.set(sessionId, msgs)
          setThreadMessages(new Map(threadMessagesRef.current))
        }
      }
    },
    [activeThreadId, input, streaming, threadMessages, onActiveThreadChange, onReload, scrollDown]
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
              currentMessages.map((msg, idx) => {
                const isLastAi = msg.role === "assistant" && idx === currentMessages.length - 1
                return (
                  <div
                    key={msg.id}
                    className={`flex w-full gap-3 ${msg.role === "user" ? "flex-row-reverse" : ""}`}
                  >
                    <span
                      className={`flex size-8 shrink-0 items-center justify-center rounded-full text-[11px] font-semibold ${
                        msg.role === "user" ? "bg-ai text-white" : "bg-bg-hover text-white"
                      }`}
                    >
                      {msg.role === "user" ? "U" : "AI"}
                    </span>
                    <div
                      className={`min-w-0 flex-1 text-sm leading-relaxed ${
                        msg.role === "user" ? "flex items-center justify-end" : "text-white/90"
                      }`}
                    >
                      {msg.role === "assistant" ? (
                        <>
                          {msg.isError && (
                            <div className="border-red-500/30 bg-red-500/5 mb-2 flex items-center gap-2 rounded-lg border px-3 py-2">
                              <div className="flex size-5 items-center justify-center rounded-full bg-red-500/20">
                                <span className="text-[10px] font-bold text-red-500">!</span>
                              </div>
                              <span className="text-ink-muted text-[11px]">
                                此消息因错误已自动重试
                              </span>
                            </div>
                          )}
                          <ThinkingBlock
                            content={msg.reasoning ?? ""}
                            isStreaming={streaming && isLastAi}
                          />
                          <ToolCallBlock
                            toolCalls={(msg.toolCalls ?? []).map((tc) => ({
                              id: tc.id,
                              name: tc.name,
                              args: tc.args,
                              result: tc.result,
                              partialResult: tc.partialResult,
                              status: tc.status,
                            }))}
                            isStreaming={streaming && isLastAi}
                          />
                          <ToolResultBlock results={msg.toolResults ?? []} />
                          <TextBlock content={msg.content} isStreaming={streaming && isLastAi} />
                          {isLastAi && (
                            <>
                              <CompactionIndicator
                                status={compactionStatus}
                                reason={compactionReason}
                                error={compactionError}
                              />
                              <RetryIndicator
                                status={retryStatus}
                                attempt={retryAttempt}
                                maxAttempts={retryMaxAttempts}
                                delayMs={retryDelayMs}
                                error={retryError}
                              />
                            </>
                          )}
                        </>
                      ) : (
                        <div className="bg-ink-dim inline-block max-w-[80%] rounded-2xl rounded-tr-sm px-4 py-2.5 text-white">
                          {msg.content}
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
