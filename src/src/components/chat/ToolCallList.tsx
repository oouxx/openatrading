import { useMemo, useState } from "react"
import {
  Check,
  ChevronDown,
  ChevronRight,
  FileEdit,
  Loader2,
  RefreshCw,
  Terminal,
  Wrench,
  XCircle,
} from "lucide-react"
import { fetchJson } from "@/stores/data"

// ── Types ────────────────────────────────────────────────

export interface ToolCall {
  id: string
  name: string
  args?: string
  result?: string
  partialResult?: string
  status?: "streaming" | "running" | "success" | "error"
  output?: { preview: string; truncated: boolean; fullLength: number }
}

interface ToolCallListProps {
  toolCalls: ToolCall[]
  isStreaming?: boolean
  sessionId?: string | null
}

const COLLAPSE_THRESHOLD = 500

/** Coerce any tool result / partial-result payload to a renderable string. */
function asText(v: unknown): string {
  if (v == null) return ""
  if (typeof v === "string") return v
  if (typeof v === "number" || typeof v === "boolean") return String(v)
  try {
    return JSON.stringify(v)
  } catch {
    return String(v)
  }
}

/** snake_case / kebab-case → Title Case. `get_kline` → `Get Kline`. */
function toTitleCase(s: string): string {
  return s
    .replace(/[-_]+/g, " ")
    .replace(/\b\w/g, (c) => c.toUpperCase())
    .replace(/\s+/g, " ")
    .trim()
}

/** Parse the serialized tool `args` string into an object (best-effort). */
function parseArgs(args?: string): Record<string, unknown> | null {
  if (!args) return null
  try {
    const p = JSON.parse(args)
    return p && typeof p === "object" && !Array.isArray(p) ? (p as Record<string, unknown>) : null
  } catch {
    return null
  }
}

/** First non-empty string value among candidate arg keys. */
function argString(a: Record<string, unknown> | null, ...keys: string[]): string {
  if (!a) return ""
  for (const k of keys) {
    const v = a[k]
    if (typeof v === "string" && v.trim()) return v
  }
  return ""
}

/**
 * Generic tool-summary generator (Goose-style). One card serves every tool:
 * a few well-known tools get friendly Chinese phrases derived from their args;
 * everything else falls back to a readable name + the argument keys, so an
 * unknown / MCP tool never requires a bespoke card.
 */
export function summarizeTool(name: string, args?: string): string {
  const a = parseArgs(args)
  const p = (...keys: string[]) => argString(a, ...keys)

  if (/^(read|read_file)$/.test(name)) return p("path", "uri", "file_path") ? `查看 ${p("path", "uri", "file_path")}` : "查看文件"
  if (/^(write|write_file|create_file)$/.test(name)) return p("path", "file_path") ? `写入 ${p("path", "file_path")}` : "写入文件"
  if (/^(edit|edit_file|file_edit)$/.test(name)) return p("path", "file_path") ? `编辑 ${p("path", "file_path")}` : "编辑文件"
  if (/^(bash|shell|execute_command|run)$/.test(name)) return p("command", "cmd") ? `运行 ${p("command", "cmd")}` : "运行命令"
  if (name === "glob") return p("pattern") ? `查找 ${p("pattern")}` : "查找文件"
  if (name === "grep") return p("pattern") ? `搜索 ${p("pattern")}` : "搜索代码"
  if (/^(web_search|search)$/.test(name)) return p("query", "q") ? `搜索 ${p("query", "q")}` : "网页搜索"
  if (/^(web_fetch|fetch)$/.test(name)) return p("url") ? `抓取 ${p("url")}` : "抓取网页"
  if (name === "submit_trading_decision") return "提交交易决策"

  // Generic fallback: readable name + a single concise arg value, or the arg keys.
  const readable = toTitleCase(name)
  if (a) {
    const keys = Object.keys(a)
    if (keys.length === 1) {
      const val = a[keys[0]]
      const short = typeof val === "string" ? val : JSON.stringify(val)
      return short && short !== "{}" ? `${readable} · ${short}` : readable
    }
    if (keys.length > 1) return `${readable} · ${keys.join(", ")}`
  }
  return readable
}

// ── Main Components ───────────────────────────────────────

/**
 * Single tool-call card (Claude.ai activity-timeline style). Each invocation
 * is one compact row (icon + name + status); clicking reveals args/result.
 * `showIcon` hides the inline icon so the timeline node can provide it.
 */
export function ToolCallItem({
  toolCall: tc,
  isStreaming,
  sessionId,
  showIcon = true,
}: {
  toolCall: ToolCall
  isStreaming?: boolean
  sessionId?: string | null
  showIcon?: boolean
}) {
  const [expanded, setExpanded] = useState(false)
  const status = tc.status ?? (isStreaming ? "streaming" : "success")
  const isRunning = status === "streaming" || status === "running"
  const hasArgs = tc.args && tc.args.length > 0
  const resultText = asText(tc.result ?? "")
  const liveText = asText(tc.partialResult ?? "")
  const hasResult = !!tc.result || !!tc.partialResult

  return (
    <div
      className={`rounded-[10px] border transition-all duration-200 ${
        status === "error" ? "border-danger/30 bg-danger/10" : "border-hairline bg-hover/50"
      }`}
    >
      <button
        className="hover:bg-hover/60 flex w-full items-center gap-2.5 px-3 py-2 text-left transition-colors"
        onClick={() => setExpanded((v) => !v)}
      >
        {showIcon && (
          <span className="bg-hover flex size-5 shrink-0 items-center justify-center rounded-md">
            <ToolIcon name={tc.name} className="size-3" />
          </span>
        )}
        <span className="text-primary min-w-0 flex-1 truncate text-[13px] font-medium">
          {summarizeTool(tc.name, tc.args)}
        </span>
        <span className="text-secondary/40 hidden shrink-0 font-number text-[11px] sm:inline">
          {tc.name}
        </span>
        <StatusIcon status={status} />
        <span className="text-secondary/50 shrink-0">
          {expanded ? <ChevronDown className="size-3.5" /> : <ChevronRight className="size-3.5" />}
        </span>
      </button>

      {(hasArgs || hasResult) && expanded && (
        <div className="border-hairline space-y-2 border-t px-3 py-2.5">
          {hasArgs && (
            <Section title="参数">
              <ToolArgsContent name={tc.name} args={tc.args!} />
            </Section>
          )}
          {hasResult && (
            <Section title="结果">
              <ToolResultContent
                name={tc.name}
                content={isRunning && liveText ? liveText : resultText}
                toolCallId={tc.id}
                sessionId={sessionId}
                output={tc.output}
              />
            </Section>
          )}
        </div>
      )}
    </div>
  )
}

/**
 * Stacked tool-call list (several invocations of one turn, standalone use).
 */
export default function ToolCallList({ toolCalls, isStreaming, sessionId }: ToolCallListProps) {
  if (!toolCalls || toolCalls.length === 0) return null
  return (
    <div className="my-2 flex flex-col gap-1.5">
      {toolCalls
        .filter((tc) => tc.name)
        .map((tc, i) => (
          <ToolCallItem
            key={tc.id || i}
            toolCall={tc}
            isStreaming={isStreaming}
            sessionId={sessionId}
          />
        ))}
    </div>
  )
}

// ── Tool icon by name ─────────────────────────────────

export function ToolIcon({ name, className }: { name: string; className?: string }) {
  const cls = className ?? "size-3.5"
  if (
    name.startsWith("edit") ||
    name === "file_edit" ||
    name === "write_file" ||
    name === "create_file"
  ) {
    return <FileEdit className={`${cls} shrink-0 text-warning`} />
  }
  if (name.startsWith("bash") || name === "execute_command" || name === "run") {
    return <Terminal className={`${cls} shrink-0 text-sky-500`} />
  }
  return <Wrench className={`${cls} shrink-0 text-secondary`} />
}

// ── Tool args content ─────────────────────────────────

function ToolArgsContent({ name, args }: { name: string; args: string }) {
  const safeContent = useMemo(() => formatArgs(args), [args])

  if (name.startsWith("edit") || name === "file_edit") {
    return <DiffContent content={safeContent} />
  }

  return (
    <pre className="text-secondary overflow-x-auto font-number text-caption leading-relaxed whitespace-pre-wrap">
      {safeContent}
    </pre>
  )
}

// ── Tool result content ───────────────────────────────

function ToolResultContent({
  name,
  content,
  toolCallId,
  sessionId,
  output,
}: {
  name: string
  content: string
  toolCallId: string
  sessionId?: string | null
  output?: { preview: string; truncated: boolean; fullLength: number }
}) {
  const safeContent = useMemo(() => asText(content), [content])

  if (name.startsWith("edit") || name === "file_edit") {
    return <DiffContent content={safeContent} />
  }

  return <ResultBlock content={safeContent} toolCallId={toolCallId} sessionId={sessionId} output={output} />
}

// ── Diff content ─────────────────────────────────────

function DiffContent({ content }: { content: string }) {
  const safeContent = useMemo(() => asText(content), [content])

  const diffResult = useMemo(() => {
    const hasDiffMarkers = safeContent.split("\n").some((l) => l.startsWith("+") || l.startsWith("-") || l.startsWith("@"))
    if (hasDiffMarkers) {
      return { type: "diff" as const }
    }
    return { type: "raw" as const, content: safeContent }
  }, [safeContent])

  if (diffResult.type === "diff") {
    return (
      <pre className="overflow-x-auto font-number text-caption leading-relaxed whitespace-pre-wrap">
        {safeContent.split("\n").map((line, i) => {
          let className = ""
          if (line.startsWith("+")) className = "text-success"
          else if (line.startsWith("-")) className = "text-danger"
          else if (line.startsWith("@")) className = "text-secondary/50"
          return (
            <div key={i} className={className}>
              {line}
            </div>
          )
        })}
      </pre>
    )
  }

  return <ResultBlock content={safeContent} toolCallId="" />
}

/** Renders tool result content with client-side truncation and server-side expand support. */
function ResultBlock({
  content,
  toolCallId,
  sessionId,
  output,
}: {
  content: string
  toolCallId: string
  sessionId?: string | null
  output?: { preview: string; truncated: boolean; fullLength: number }
}) {
  const [fullContent, setFullContent] = useState<string | null>(null)
  const [showFull, setShowFull] = useState(false)
  const [loading, setLoading] = useState(false)

  const isTruncated = output?.truncated ?? false
  const isLong = content.length > COLLAPSE_THRESHOLD && !isTruncated

  // Server-truncated: use preview, can expand via API
  // Client-long: show first part, can expand locally
  const displayText = asText(
    fullContent ??
    (isTruncated ? (output?.preview ?? content) :
     isLong && !showFull ? content.slice(0, COLLAPSE_THRESHOLD) + "\n…" :
     content)
  )

  const expandFromServer = async () => {
    if (!sessionId) return
    setLoading(true)
    try {
      const data = await fetchJson<{ content?: string }>(
        `/api/ai/sessions/${sessionId}/tool-results/${toolCallId}`
      )
      setFullContent(data.content ?? content)
    } catch {
      // Fall back to the content we already have
    } finally {
      setLoading(false)
    }
  }

  return (
    <div>
      <pre className="text-secondary max-h-96 overflow-y-auto font-number text-caption leading-relaxed whitespace-pre-wrap">
        {displayText}
      </pre>
      {/* Server-side truncation: expand via API */}
      {isTruncated && !fullContent && (
        <button
          className="text-secondary/50 hover:text-secondary mt-1 font-number text-caption transition-colors"
          onClick={expandFromServer}
          disabled={loading}
        >
          {loading
            ? "加载中..."
            : `展开全部（${output?.fullLength ?? content.length} 字符）`}
        </button>
      )}
      {/* Client-side truncation: expand locally */}
      {isLong && !showFull && !isTruncated && (
        <button
          className="text-secondary/50 hover:text-secondary mt-1 font-number text-caption transition-colors"
          onClick={() => setShowFull(true)}
        >
          展开全部（{content.length} 字符）
        </button>
      )}
    </div>
  )
}

// Claude.ai 风格状态指示：仅图标，无文字标签
function StatusIcon({ status }: { status: ToolCall["status"] }) {
  switch (status) {
    case "streaming":
      return <Loader2 className="text-brand size-3.5 shrink-0 animate-spin" />
    case "running":
      return <RefreshCw className="size-3.5 shrink-0 animate-spin text-warning" />
    case "success":
      return <Check className="text-success size-3.5 shrink-0" />
    case "error":
      return <XCircle className="size-3.5 shrink-0 text-danger" />
    default:
      return null
  }
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div>
      <div className="text-secondary/70 mb-1 font-number text-caption font-medium">{title}</div>
      {children}
    </div>
  )
}

function formatArgs(args: string): string {
  try {
    const parsed = JSON.parse(args)
    return JSON.stringify(parsed, null, 2)
  } catch {
    return args
  }
}
