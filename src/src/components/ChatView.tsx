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
import { Search, Square, ArrowDown, ArrowUp, FileText, Loader2, Paperclip, X } from "lucide-react"
import SystemNoticeBlock from "@/components/chat/SystemNoticeBlock"
import AgentStatusBar from "@/components/chat/AgentStatusBar"
import { TurnActions } from "@/components/chat/TurnActions"
import ActivityTimeline, { type ActivityItem } from "@/components/chat/ActivityTimeline"
import { summarizeTool } from "@/lib/tool-summary"
import type { AgentPhase } from "@/components/chat/AgentStatusBar"
import type { HistoryMessage } from "@/agents/types"
import { SidebarTrigger } from "@/components/ui/sidebar"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Textarea } from "@/components/ui/textarea"
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "@/components/ui/dialog"
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandItem,
  CommandList,
} from "@/components/ui/command"
import { defaultFilter } from "cmdk"

import { fetchJson } from "@/stores/data"
import { fetchCommands } from "@/hooks/useChatThreads"
import { Skeleton } from "@/components/ui/skeleton"
import { useStockTabStore } from "@/stores/stockTabStore"
import { useMultiAgentWebSocket } from "@/agents/useMultiAgentWebSocket"
import { TauriTransport, type AgentTransport } from "@/agents/transport"
import { appendUserTurn, applyEvent, historyToTurns } from "@/agents/reducer"
import type { PromptFile, PromptImage, TranscriptBlock, TranscriptTurn } from "@/agents/types"
import {
  formatFileSize,
  isFileAttachment,
  isImageAttachment,
  mergeComposerAttachments,
  readComposerAttachments,
  type ComposerAttachment,
} from "@/lib/composer-attachments"

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

interface ImagePreview {
  src: string
  alt: string
  name: string
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

/** 一条消息最多带几个附件（图片与文件共用名额）。 */
const MAX_ATTACHMENTS = 6
/** 单张图片上限 5MB：base64 后随一条 WebSocket 文本帧一起发出。 */
const MAX_ATTACHMENT_BYTES = 5 * 1024 * 1024
/** 单个文件上限 20MB，与后端 `MAX_FILE_BYTES` 一致。 */
const MAX_FILE_ATTACHMENT_BYTES = 20 * 1024 * 1024
/** 输入框自动增高的上限，需与 textarea 上的 `max-h-[200px]` 保持一致。 */
const TEXTAREA_MAX_HEIGHT = 200

export default function AiCopilot({
  activeThreadId,
  onActiveThreadChange,
  onReload,
}: AiCopilotProps) {
  const [input, setInput] = useState("")
  const [attachments, setAttachments] = useState<ComposerAttachment[]>([])
  // 异步读取完成时必须合并到最新附件列表，而不是创建读取任务时的旧快照。
  const attachmentsRef = useRef<ComposerAttachment[]>([])
  const pendingAttachmentReadsRef = useRef(0)
  // state 更新不会同步反映到下一次事件回调，发送锁必须用 ref 防止连点重复发送。
  const sendingRef = useRef(false)
  const [isReadingAttachments, setIsReadingAttachments] = useState(false)
  const [previewImage, setPreviewImage] = useState<ImagePreview | null>(null)
  // 输入区的可见错误（发送失败 / 附件被拒）。输入框是唯一能重试的地方，
  // 静默失败等于把消息吞掉。
  const [composerError, setComposerError] = useState<string | null>(null)
  // 拖拽文件悬停在输入卡片上（给一个可放置的视觉反馈）。
  const [draggingFiles, setDraggingFiles] = useState(false)
  // 输入卡片实测高度：消息列表的底部留白要跟着它走（输入变高时不能压住
  // 最后一条消息）。用 ref 回调观测，卡片在「首屏居中 / 底部悬浮」之间
  // 换位置时 observer 会重新绑定。
  const [composerHeight, setComposerHeight] = useState(0)
  const composerObserverRef = useRef<ResizeObserver | null>(null)
  const setComposerCard = useCallback((node: HTMLDivElement | null) => {
    composerObserverRef.current?.disconnect()
    composerObserverRef.current = null
    if (!node) {
      setComposerHeight(0)
      return
    }
    const observer = new ResizeObserver(([entry]) => setComposerHeight(entry.contentRect.height))
    observer.observe(node)
    composerObserverRef.current = observer
  }, [])
  const fileInputRef = useRef<HTMLInputElement | null>(null)
  const replaceAttachments = useCallback((next: ComposerAttachment[]) => {
    attachmentsRef.current = next
    setAttachments(next)
  }, [])
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
  //
  // 错误态只留一个尾部标记：错误原因由上方的红色 notice 卡承载，状态条再重复
  // 一遍同一段文本只会在同一屏里出现两次。
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
  // Composer wrapper (textarea + attach + slash menu) for outside-click closing.
  const composerRef = useRef<HTMLDivElement | null>(null)
  // Set when a command was inserted programmatically: the caret must land after
  // the command name so the user can type arguments straight away.
  const caretToEndRef = useRef(false)

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
  /** Optimistically append the user's own turn (text + attachments) so
   *  sending has immediate visual feedback before the assistant turn streams.
   *  The turn is replaced by persisted history once the agent settles. */
  const appendLocalUserTurn = useCallback(
    (sessionId: string, text: string, images: PromptImage[], files: PromptFile[]) => {
      const current = threadTurnsRef.current
      const turns = current.get(sessionId) ?? []
      const next = appendUserTurn(turns, text, images, files)
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
      if (pendingAttachmentReadsRef.current > 0) {
        setComposerError("附件还在读取，请稍候再发送")
        return
      }
      const text = (override ?? input).trim()
      const attachments = attachmentsRef.current
      if (!text && attachments.length === 0) return
      if (sendingRef.current) return
      sendingRef.current = true
      // 图片与文件分开走两条通道：图片进 pi-rs 的 image blocks，
      // 文件由后端按字节提取文本。
      const images: PromptImage[] = attachments
        .filter(isImageAttachment)
        .map(({ data, mimeType }) => ({ data, mimeType }))
      const files: PromptFile[] = attachments
        .filter(isFileAttachment)
        .map(({ name, data, mimeType, size }) => ({ name, data, mimeType, size }))
      // 发送前是否已有回合在跑（steer）：失败回滚时要恢复这个状态，不能把
      // 仍在运行的回合当成已结束。（与 `isStreamingActive` 同义，但那是在
      // 本回调之后才声明的派生值。）
      const wasStreaming = streaming && streamThreadId === activeThreadId

      // 在第一个 await 之前清空输入，确保新会话的 React 重渲染期间也无法
      // 再次发送同一份内容。发送失败时再从快照恢复。
      setInput("")
      replaceAttachments([])
      setShowSlash(false)
      setComposerError(null)
      setStreaming(true)

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

      streamThreadRef.current = sessionId
      setStreamThreadId(sessionId)

      // 发送失败时把内容还给用户，并在输入区说明原因：两条传输通道的失败
      // 表现不同（Tauri 抛 IPC 错误、Web 抛连接超时或在 socket 关闭时静默丢弃），
      // 但用户侧必须看到同一件事——「没发出去，内容还在，可以重试」。
      try {
        if (isTauri) {
          await startTauriTransport(sessionId)
          await transportRef.current?.sendPrompt(text, images, files)
          appendLocalUserTurn(sessionId, text, images, files)
        } else {
          // Use wsAgent directly with the resolved sessionId so we don't
          // depend on wsAgent.sendPrompt being updated after onActiveThreadChange.
          // The transport appends the optimistic user turn to its authoritative
          // transcript so live assistant events cannot overwrite it.
          await wsAgent.sendPrompt(sessionId, text, images, files)
        }
      } catch (e) {
        console.error("发送失败:", e)
        // 发送期间用户可能已经输入了新内容或添加了新附件。只补回缺失的
        // 部分，不能覆盖用户此刻正在编辑的状态。
        setInput((current) => (current.length === 0 ? text : current))
        // 以发送期间用户新加的附件为准，只把丢失的旧附件补到剩余容量里。
        const restored = mergeComposerAttachments(attachmentsRef.current, attachments, MAX_ATTACHMENTS)
        replaceAttachments(restored.attachments)
        setStreaming(wasStreaming)
        streamThreadRef.current = wasStreaming ? streamThreadId : null
        setStreamThreadId(wasStreaming ? streamThreadId : null)
        const reason = e instanceof Error && e.message ? e.message : "连接已断开"
        const restoreWarning = restored.rejected.join("；")
        setComposerError(
          `发送失败：${reason}。内容已保留，可重试。${restoreWarning ? `；${restoreWarning}` : ""}`
        )
        textareaRef.current?.focus()
      } finally {
        sendingRef.current = false
      }

      // 点「发送」按钮时焦点在按钮上，按钮随即消失；把焦点还给输入框，
      // 用户可以直接接着打字。
      textareaRef.current?.focus()
    },
    [
      input,
      activeThreadId,
      streaming,
      streamThreadId,
      onActiveThreadChange,
      startTauriTransport,
      wsAgent,
      appendLocalUserTurn,
      replaceAttachments,
    ]
  )

  // ── Attachment handlers (multimodal) ─────────────────────────────────
  /**
   * 附件的唯一入口：文件选择、粘贴、拖拽都走这里。图片会被识别成图片块，
   * 其余文件由后端按字节类型提取文本。
   *
   * 校验（类型 / 体积 / 数量）只写一遍，被拒的文件要给出可见原因——用户没法
   * 从「什么都没发生」里推测出是文件太大还是格式不支持。
   */
  const addAttachments = useCallback(
    async (files: File[]) => {
      if (files.length === 0) return
      pendingAttachmentReadsRef.current += 1
      setIsReadingAttachments(true)
      try {
        const result = await readComposerAttachments(files, {
          currentCount: attachmentsRef.current.length,
          maxAttachments: MAX_ATTACHMENTS,
          maxImageBytes: MAX_ATTACHMENT_BYTES,
          maxFileBytes: MAX_FILE_ATTACHMENT_BYTES,
          readFile: readFileAsDataURL,
          createId: () => crypto.randomUUID(),
        })
        const merged = mergeComposerAttachments(
          attachmentsRef.current,
          result.attachments,
          MAX_ATTACHMENTS
        )
        if (merged.added > 0) {
          replaceAttachments(merged.attachments)
          setShowSlash(false)
          textareaRef.current?.focus()
        }
        const rejected = [...new Set([...result.rejected, ...merged.rejected])]
        setComposerError(rejected.length > 0 ? rejected.join("；") : null)
      } finally {
        pendingAttachmentReadsRef.current = Math.max(0, pendingAttachmentReadsRef.current - 1)
        if (pendingAttachmentReadsRef.current === 0) setIsReadingAttachments(false)
      }
    },
    [replaceAttachments]
  )

  const handleAttachFiles = useCallback(
    async (e: React.ChangeEvent<HTMLInputElement>) => {
      const files = Array.from(e.target.files ?? [])
      e.target.value = ""
      await addAttachments(files)
    },
    [addAttachments]
  )

  /** 粘贴截图或文件。剪贴板同时带文字时保留默认粘贴。 */
  const onComposerPaste = useCallback(
    (e: React.ClipboardEvent<HTMLTextAreaElement>) => {
      const clipboardFiles = Array.from(e.clipboardData.items)
        .filter((item) => item.kind === "file")
        .map((item) => item.getAsFile())
        .filter((file): file is File => file !== null)
      const files =
        clipboardFiles.length > 0 ? clipboardFiles : Array.from(e.clipboardData.files)
      if (files.length === 0) return
      if (!e.clipboardData.getData("text/plain")) e.preventDefault()
      void addAttachments(files)
    },
    [addAttachments]
  )

  // 拖拽文件进输入卡片。`types` 里没有 Files 的拖拽（选中文字等）不拦截。
  const onComposerDragOver = useCallback((e: React.DragEvent) => {
    if (!e.dataTransfer.types.includes("Files")) return
    e.preventDefault()
    e.dataTransfer.dropEffect = "copy"
    setDraggingFiles(true)
  }, [])

  const onComposerDragLeave = useCallback((e: React.DragEvent) => {
    // 鼠标在卡片内部子元素之间移动也会触发 dragleave，用 relatedTarget 过滤。
    if (e.currentTarget.contains(e.relatedTarget as Node | null)) return
    setDraggingFiles(false)
  }, [])

  const onComposerDrop = useCallback(
    (e: React.DragEvent) => {
      setDraggingFiles(false)
      if (!e.dataTransfer.types.includes("Files")) return
      e.preventDefault()
      void addAttachments(Array.from(e.dataTransfer.files))
    },
    [addAttachments]
  )

  const removeAttachment = useCallback(
    (id: string) => {
      replaceAttachments(attachmentsRef.current.filter((attachment) => attachment.id !== id))
    },
    [replaceAttachments]
  )

  // ── Stop / interrupt the active stream ───────────────────────────────
  const stop = useCallback(async () => {
    if (!activeThreadId) return
    if (isTauri) {
      await transportRef.current?.interrupt()
    } else {
      await wsAgent.interrupt(activeThreadId)
    }
  }, [wsAgent, activeThreadId])

  // ── Slash commands ────────────────────────────────────────────────────
  // 匹配在父层算：菜单渲染什么、Enter 是「执行命令」还是「发送消息」必须是
  // 同一份结果（cmdk 关闭自带过滤，见 SlashCommandMenu 的 shouldFilter）。
  // 命令数量在几十条量级，每次输入重算一次过滤，不需要 memo。
  const slashFilter = input.startsWith("/") ? input.slice(1) : ""
  const slashMatches = showSlash
    ? commands.filter((cmd) => defaultFilter(cmd.name, slashFilter) > 0)
    : []

  // ── Input handlers ────────────────────────────────────────────────────
  const onInputChange = useCallback((e: React.ChangeEvent<HTMLTextAreaElement>) => {
    const value = e.target.value
    setInput(value)
    setComposerError(null)

    // 斜杠命令只在「整段输入仍然是一个命令名」时出现：一旦出现空格/换行，
    // 说明用户已经在写参数或正文，菜单让位给输入。
    setShowSlash(value.startsWith("/") && !/\s/.test(value))
  }, [])

  const onInputKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
      // 输入法组合中的 Enter 是「选词确认」，不是发送/执行（中文输入必踩）。
      if (e.nativeEvent.isComposing || e.keyCode === 229) return

      if (showSlash) {
        if (e.key === "Escape") {
          e.preventDefault()
          setShowSlash(false)
          return
        }
        // 只有真的匹配到命令才接管方向键与回车。没有匹配时 Enter 必须落到
        // 下面的发送分支，否则 `/` 开头的普通文本会卡住发不出去。
        if (slashMatches.length > 0 && SLASH_MENU_KEYS.has(e.key)) {
          e.preventDefault()
          slashMenuRef.current?.dispatchEvent(
            new KeyboardEvent("keydown", { key: e.key, bubbles: true })
          )
          return
        }
      }
      if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault()
        send()
      }
    },
    [send, showSlash, slashMatches.length]
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

  // 输入框随内容增高：CSS `field-sizing: content` 在 Safari 18.4 之前不支持，
  // 用 JS 兜底（两种机制结果一致，内联高度优先生效）。超过上限后内部滚动。
  useLayoutEffect(() => {
    const el = textareaRef.current
    if (!el) return
    el.style.height = "auto"
    el.style.height = `${Math.min(el.scrollHeight, TEXTAREA_MAX_HEIGHT)}px`
  }, [input])

  // 插入命令后把光标放到末尾（浏览器对受控 textarea 的光标位置不可靠）。
  useEffect(() => {
    if (!caretToEndRef.current) return
    caretToEndRef.current = false
    const el = textareaRef.current
    if (!el) return
    el.focus()
    el.setSelectionRange(el.value.length, el.value.length)
  }, [input])

  // 文件被拖到输入卡片以外的地方时，浏览器默认会直接打开该文件（等于离开
  // 应用、丢掉当前会话）。全局拦掉这个默认行为；真正的落点仍由输入卡片处理。
  useEffect(() => {
    const preventFileNavigation = (e: DragEvent) => {
      if (e.dataTransfer?.types.includes("Files")) e.preventDefault()
    }
    document.addEventListener("dragover", preventFileNavigation)
    document.addEventListener("drop", preventFileNavigation)
    return () => {
      document.removeEventListener("dragover", preventFileNavigation)
      document.removeEventListener("drop", preventFileNavigation)
    }
  }, [])

  // 点击输入区之外收起斜杠菜单；点在菜单里不算「外面」，事件仍在容器内。
  useEffect(() => {
    if (!showSlash) return
    const onPointerDown = (e: MouseEvent) => {
      if (!composerRef.current?.contains(e.target as Node)) setShowSlash(false)
    }
    document.addEventListener("mousedown", onPointerDown)
    return () => document.removeEventListener("mousedown", onPointerDown)
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
  //
  //    一张卡片装下所有输入物：附件预览、正文、底部工具行（附图 / 提示 / 发送）。
  //    按钮不再绝对定位压在正文上——输入变高时按钮跟着卡片走，文字也不会被
  //    padding 挤成窄条；附件、粘贴、拖拽落点也都在同一张卡里。
  const hasPayload = input.trim().length > 0 || attachments.length > 0
  const renderInputField = () => (
    <div ref={composerRef} className="relative flex-1">
      {composerError && (
        <p role="status" className="text-down text-caption mb-1.5 px-1">
          {composerError}
        </p>
      )}

      <div
        ref={setComposerCard}
        onDragOver={onComposerDragOver}
        onDragLeave={onComposerDragLeave}
        onDrop={onComposerDrop}
        className={`bg-surface border-hairline shadow-card focus-within:border-brand/60 focus-within:ring-brand/20 relative flex flex-col gap-1.5 rounded-lg border px-2 pt-2 pb-1.5 transition-colors focus-within:ring-3 ${
          draggingFiles ? "border-brand/60 ring-brand/20 ring-3" : ""
        }`}
      >
        {/* Attached image previews + file chips */}
        {attachments.length > 0 && (
          <div className="flex flex-wrap items-center gap-1.5 px-0.5">
            {attachments.map((attachment, i) => {
              if (isImageAttachment(attachment)) {
                const src = `data:${attachment.mimeType};base64,${attachment.data}`
                return (
                  <div key={attachment.id} className="relative">
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon"
                      onClick={() =>
                        setPreviewImage({
                          src,
                          alt: `附件 ${i + 1}：${attachment.name}`,
                          name: attachment.name,
                        })
                      }
                      className="border-hairline h-16 w-16 cursor-zoom-in overflow-hidden rounded-md border p-0 hover:opacity-90"
                      aria-label={`预览附件 ${i + 1}：${attachment.name}`}
                      title={attachment.name}
                    >
                      <img src={src} alt="" className="h-full w-full object-cover" />
                    </Button>
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon-xs"
                      onClick={() => removeAttachment(attachment.id)}
                      className="bg-scrim-strong text-brand-foreground hover:bg-scrim absolute -top-1.5 -right-1.5 size-5 rounded-full"
                      aria-label={`移除附件 ${i + 1}`}
                    >
                      <X />
                    </Button>
                  </div>
                )
              }

              return (
                <div
                  key={attachment.id}
                  className="border-hairline bg-canvas flex h-16 max-w-[14rem] min-w-0 items-center gap-2 rounded-md border px-2.5"
                  title={attachment.name}
                >
                  <FileText className="text-secondary size-4 shrink-0" />
                  <div className="min-w-0 flex-1">
                    <p className="text-caption text-primary truncate">{attachment.name}</p>
                    <p className="text-micro text-tertiary">{formatFileSize(attachment.size)}</p>
                  </div>
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon-xs"
                    onClick={() => removeAttachment(attachment.id)}
                    className="text-tertiary hover:text-primary hover:bg-hover shrink-0 rounded-full"
                    aria-label={`移除附件 ${i + 1}`}
                  >
                    <X />
                  </Button>
                </div>
              )
            })}
          </div>
        )}

        <Textarea
          ref={textareaRef}
          className="text-body text-primary placeholder:text-tertiary max-h-[200px] min-h-[24px] w-full resize-none border-0 bg-transparent px-0.5 py-0 shadow-none dark:bg-transparent"
          placeholder="有什么可以帮你？"
          aria-label="输入消息"
          aria-expanded={showSlash}
          aria-controls={showSlash ? SLASH_MENU_ID : undefined}
          rows={1}
          value={input}
          onChange={onInputChange}
          onKeyDown={onInputKeyDown}
          onPaste={onComposerPaste}
        />

        <div className="flex items-center gap-1">
          <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            onClick={() => fileInputRef.current?.click()}
            className="text-secondary hover:text-brand hover:bg-hover rounded-md"
            aria-label="添加附件"
            title={`添加附件（最多 ${MAX_ATTACHMENTS} 个，图片 ${MAX_ATTACHMENT_BYTES / 1024 / 1024}MB、文件 ${MAX_FILE_ATTACHMENT_BYTES / 1024 / 1024}MB；也可粘贴或拖入）`}
            disabled={isStreamingActive}
          >
            <Paperclip className="size-4" />
          </Button>
          {isReadingAttachments ? (
            <span className="text-tertiary text-micro flex items-center gap-1">
              <Loader2 className="size-3 animate-spin" />
              正在读取附件…
            </span>
          ) : isStreamingActive ? (
            <span className="text-tertiary text-micro">Enter 可引导当前回答</span>
          ) : null}
          <div className="flex-1" />
          {isStreamingActive ? (
            <Button
              size="icon"
              variant="secondary"
              className="rounded-full"
              onClick={() => void stop()}
              aria-label="停止生成"
              title="停止生成"
            >
              <Square className="size-3.5" fill="currentColor" />
            </Button>
          ) : (
            hasPayload && (
              <Button
                size="icon"
                onClick={() => void send()}
                className="rounded-full"
                aria-label={isReadingAttachments ? "附件读取中" : "发送"}
                title={isReadingAttachments ? "附件读取中" : "发送（Enter）"}
                disabled={isReadingAttachments}
                aria-busy={isReadingAttachments}
              >
                {isReadingAttachments ? (
                  <Loader2 className="size-4 animate-spin" />
                ) : (
                  <ArrowUp className="size-4" />
                )}
              </Button>
            )
          )}
        </div>

        {draggingFiles && (
          <div className="bg-surface/95 border-brand/60 pointer-events-none absolute inset-0 z-20 flex items-center justify-center rounded-lg border-2">
            <span className="text-brand text-caption flex items-center gap-2 font-medium">
              <Paperclip className="size-4" />
              松开以添加附件
            </span>
          </div>
        )}
      </div>

      {showSlash && (
        <div id={SLASH_MENU_ID} className="absolute right-0 bottom-full left-0 z-20 mb-1">
          <SlashCommandMenu
            commands={slashMatches}
            ref={slashMenuRef}
            onSelect={(cmd) => {
              caretToEndRef.current = true
              setInput(`/${cmd.name} `)
              setShowSlash(false)
            }}
          />
        </div>
      )}

      {/* Attachment picker — hidden input opened by the paperclip button. */}
      <input
        ref={fileInputRef}
        type="file"
        multiple
        className="hidden"
        onChange={handleAttachFiles}
      />
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
            className="flex flex-1 scrollbar-gutter-stable flex-col overflow-y-auto py-5"
          >
            {/* Claude.ai 消息列宽：固定 48rem 居中（不随宽屏放大） */}
            <div
              className="mx-auto flex w-full max-w-[48rem] flex-col gap-6 px-6"
              // 悬浮输入卡片（含渐变留白）盖住列表底部，留白按实测高度给足。
              style={{ paddingBottom: composerHeight + 80 }}
            >
              {currentTurns.map((turn, idx) => {
                const isLastAi = turn.role === "assistant" && idx === currentTurns.length - 1

                return (
                  <div key={turn.id}>
                    {turn.role === "user" ? (
                      // Claude 样式：用户消息右侧气泡（无头像）
                      <div className="flex w-full justify-end">
                        <div className="bg-hover text-primary max-w-[85%] rounded-lg px-4 py-2.5">
                          {turn.blocks.some((b) => b.type === "file") && (
                            <div className="mb-2 flex flex-wrap gap-2">
                              {turn.blocks.map((b) => {
                                if (b.type !== "file") return null
                                return (
                                  <span
                                    key={b.id}
                                    className="bg-surface border-hairline text-secondary text-caption flex max-w-[14rem] items-center gap-1.5 rounded-md border px-2 py-1"
                                    title={b.name}
                                  >
                                    <FileText className="size-3.5 shrink-0" />
                                    <span className="truncate">{b.name}</span>
                                  </span>
                                )
                              })}
                            </div>
                          )}
                          {turn.blocks.some((b) => b.type === "image") && (
                            <div className="mb-2 flex flex-wrap gap-2">
                              {turn.blocks.map((b) => {
                                if (b.type !== "image") return null
                                const src = `data:${b.mimeType};base64,${b.data}`
                                return (
                                  <Button
                                    key={b.id}
                                    type="button"
                                    variant="ghost"
                                    size="icon"
                                    onClick={() =>
                                      setPreviewImage({
                                        src,
                                        alt: "历史消息图片",
                                        name: "历史图片",
                                      })
                                    }
                                    className="border-hairline h-24 w-24 cursor-zoom-in overflow-hidden rounded-lg border p-0 hover:opacity-90"
                                    aria-label="预览历史消息图片"
                                  >
                                    <img src={src} alt="" className="h-full w-full object-cover" />
                                  </Button>
                                )
                              })}
                            </div>
                          )}
                          <p className="text-body leading-[1.7] break-words whitespace-pre-wrap">
                            {turn.blocks.map((b) => ("content" in b ? b.content : "")).join("")}
                          </p>
                        </div>
                      </div>
                    ) : (
                      // 助手消息直排、无头像；角色靠左右版式区分，不再逐条打「AI 助手」标签
                      <div className="group/turn flex w-full flex-col">
                        <div className="text-body w-full min-w-0 leading-[1.7] break-words">
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
                        {turn.status === "complete" && (
                          <TurnActions text={turn.blocks.map(answerText).join("\n\n")} />
                        )}
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
              // 输入卡片会随内容长高，按钮位置跟着实测高度走，否则会被压住。
              style={{ bottom: composerHeight + 72 }}
              className="bg-surface border-hairline shadow-modal hover:border-brand/40 hover:text-brand text-secondary absolute left-1/2 z-20 -translate-x-1/2 rounded-full"
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

      <Dialog
        open={previewImage !== null}
        onOpenChange={(open) => {
          if (!open) setPreviewImage(null)
        }}
      >
        <DialogContent
          showCloseButton={false}
          className="bg-surface border-hairline shadow-modal max-h-[92vh] w-auto max-w-[92vw] gap-0 overflow-hidden rounded-lg border p-0 sm:max-w-[92vw]"
        >
          <DialogTitle className="sr-only">{previewImage?.name ?? "图片预览"}</DialogTitle>
          <DialogDescription className="sr-only">图片预览，可按 Esc 关闭</DialogDescription>
          {previewImage && (
            <img
              src={previewImage.src}
              alt={previewImage.alt}
              className="max-h-[82vh] max-w-[90vw] object-contain"
            />
          )}
          <DialogClose asChild>
            <Button
              type="button"
              variant="secondary"
              size="icon-sm"
              className="shadow-card absolute top-3 right-3"
              aria-label="关闭图片预览"
            >
              <X />
            </Button>
          </DialogClose>
        </DialogContent>
      </Dialog>
    </div>
  )
}

/** 一轮回答里的正文文本（跳过思考、工具调用、系统通知），供「复制」使用。 */
function answerText(block: TranscriptBlock): string {
  return block.type === "text" ? block.content : ""
}

// ── Slash command menu ─────────────────────────────────────────────────────

/** 菜单容器 id：textarea 的 `aria-controls` 指向它。 */
const SLASH_MENU_ID = "slash-command-menu"

/** 菜单打开时由 textarea 转交给 cmdk 的按键（Escape 单独处理）。 */
const SLASH_MENU_KEYS = new Set(["ArrowDown", "ArrowUp", "Enter"])

/** 分组标题：后端 `command_type` 是 snake_case（slash / extension / skill）。 */
const SLASH_GROUP_LABELS: Record<string, string> = {
  slash: "基础命令",
  extension: "扩展命令",
  skill: "技能",
}

function groupByType(commands: SlashCommand[]): [string, SlashCommand[]][] {
  const map = new Map<string, SlashCommand[]>()
  for (const cmd of commands) {
    const key = (cmd.command_type || "slash").toLowerCase()
    const bucket = map.get(key)
    if (bucket) bucket.push(cmd)
    else map.set(key, [cmd])
  }
  return [...map.entries()]
}

/**
 * 斜杠命令候选列表。
 *
 * `commands` 已经按输入过滤好（`shouldFilter={false}`）：父层要用同一份匹配
 * 结果决定 Enter 是执行命令还是发送消息，两边各筛一次必然出现分歧。
 */
function SlashCommandMenu({
  commands,
  onSelect,
  ref,
}: {
  commands: SlashCommand[]
  onSelect: (cmd: SlashCommand) => void
  ref?: React.Ref<HTMLDivElement>
}) {
  const groups = useMemo(() => groupByType(commands), [commands])

  return (
    <Command
      ref={ref}
      shouldFilter={false}
      label="斜杠命令"
      className="bg-popover text-popover-foreground border-hairline shadow-modal overflow-hidden rounded-lg! border p-1"
    >
      <CommandList
        label="斜杠命令"
        // 菜单向上弹出：矮窗口下别顶出可视区域
        className="max-h-[min(18rem,50vh)] p-0"
      >
        {groups.length === 0 ? (
          <p className="text-tertiary text-caption px-2 py-3 text-center">
            没有匹配的命令，按 Enter 直接发送
          </p>
        ) : (
          groups.map(([type, cmds]) => (
            <CommandGroup key={type} heading={SLASH_GROUP_LABELS[type] ?? type}>
              {cmds.map((cmd) => (
                <CommandItem
                  key={cmd.name}
                  value={cmd.name}
                  onSelect={() => onSelect(cmd)}
                  className="hover:bg-hover text-caption flex cursor-pointer items-center gap-3 rounded-md px-2 py-1.5 [&>svg]:hidden"
                >
                  <span className="font-number text-brand shrink-0">/{cmd.name}</span>
                  {cmd.description && (
                    <span className="text-tertiary min-w-0 flex-1 truncate text-right">
                      {cmd.description}
                    </span>
                  )}
                </CommandItem>
              ))}
            </CommandGroup>
          ))
        )}
      </CommandList>
      {groups.length > 0 && (
        <div className="border-hairline text-tertiary text-micro mt-1 flex items-center gap-3 border-t px-2 pt-1.5 pb-0.5">
          <span>↑↓ 选择</span>
          <span>Enter 运行</span>
          <span>Esc 关闭</span>
        </div>
      )}
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
