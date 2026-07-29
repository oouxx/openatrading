import { useState, useMemo } from "react"
import { Check, Loader2, ChevronDown, ChevronRight, XCircle, Wrench, RefreshCw, Terminal, FileEdit } from "lucide-react"
import { diffLines } from "diff"
import { getApiBaseUrl } from "@/stores/data"

/**
 * Coerce any tool result / partial-result payload to a renderable string.
 */
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

interface ToolCall {
  id: string
  name: string
  args?: string
  result?: string
  partialResult?: string
  status?: "streaming" | "running" | "success" | "error"
  output?: { preview: string; truncated: boolean; fullLength: number }
}

interface ToolCallBlockProps {
  toolCalls: ToolCall[]
  isStreaming?: boolean
  sessionId?: string
}

const COLLAPSE_THRESHOLD = 500
const RESULT_PREVIEW_LENGTH = 200

export default function ToolCallBlock({ toolCalls, isStreaming, sessionId }: ToolCallBlockProps) {
  const [expanded, setExpanded] = useState<Record<string, boolean>>({})

  if (!toolCalls || toolCalls.length === 0) return null

  const toggleExpand = (id: string) => {
    setExpanded((prev) => ({ ...prev, [id]: !prev[id] }))
  }

  // Default collapsed for all completed tool calls.
  const isAutoExpanded = () => false

  return (
    <div className="my-2 flex flex-col gap-1.5">
      {toolCalls
        .filter((tc) => tc.name)
        .map((tc, i) => {
          const isExpanded = expanded[tc.id] ?? isAutoExpanded() ?? false
          const status = tc.status ?? (isStreaming ? "streaming" : "success")
          const isRunning = status === "streaming" || status === "running"
          const hasArgs = tc.args && tc.args.length > 0
          const hasResult = !!tc.result
          const hasContent = hasArgs || hasResult

          // ── Running state: compact card ──
          if (isRunning) {
            return (
              <div
                key={tc.id || i}
                className="border-hairline overflow-hidden rounded-lg border bg-bg-hover/40 transition-all duration-200"
              >
                <div className="flex items-center gap-2 px-3 py-2">
                  <ToolIcon name={tc.name} />
                  <span className="text-ink-muted font-mono text-[11px] font-medium">{tc.name}</span>
                  <StatusBadge status={status} />
                </div>
              </div>
            )
          }

          // ── Completed state: collapsed by default, expandable ──
          const resultText = asText(tc.result ?? "")
          const preview = resultText.length > RESULT_PREVIEW_LENGTH
            ? resultText.slice(0, RESULT_PREVIEW_LENGTH) + "…"
            : ""

          return (
            <div
              key={tc.id || i}
              className={`border-hairline overflow-hidden rounded-lg border transition-all duration-200 ${
                status === "error"
                  ? "border-red-500/30 bg-red-500/5"
                  : "bg-bg-hover/40"
              }`}
            >
              <button
                className="hover:bg-bg-hover/70 flex w-full items-center gap-2 px-3 py-2 text-left transition-colors"
                onClick={() => toggleExpand(tc.id)}
              >
                <ToolIcon name={tc.name} />
                <span className="text-ink-muted font-mono text-[11px] font-medium">{tc.name}</span>
                <StatusBadge status={status} />
                {/* Show result length in collapsed state */}
                {hasResult && !isExpanded && (
                  <span className="text-ink-muted/40 font-mono text-[10px]">
                    {resultText.length} 字符
                  </span>
                )}
                <span className="text-ink-muted/50 ml-auto">
                  {isExpanded ? (
                    <ChevronDown className="size-3" />
                  ) : (
                    <ChevronRight className="size-3" />
                  )}
                </span>
              </button>

              {hasContent && isExpanded && (
                <div className="border-hairline space-y-2 border-t px-3 py-2">
                  {hasArgs && (
                    <Section title="参数">
                      <ToolArgsContent name={tc.name} args={tc.args!} />
                    </Section>
                  )}
                  {hasResult && (
                    <Section title="结果">
                      <ToolResultContent
                        name={tc.name}
                        content={resultText}
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
        })}
    </div>
  )
}

// ── Tool icon by name ─────────────────────────────────

function ToolIcon({ name }: { name: string }) {
  if (name.startsWith("edit") || name === "file_edit" || name === "write_file" || name === "create_file") {
    return <FileEdit className="size-3.5 shrink-0 text-amber-500" />
  }
  if (name.startsWith("bash") || name === "execute_command" || name === "run") {
    return <Terminal className="size-3.5 shrink-0 text-sky-500" />
  }
  return <Wrench className="size-3.5 shrink-0 text-ink-muted" />
}

// ── Tool args content ─────────────────────────────────

function ToolArgsContent({ name, args }: { name: string; args: string }) {
  const safeContent = useMemo(() => formatArgs(args), [args])

  if (name.startsWith("edit") || name === "file_edit") {
    return <DiffContent content={safeContent} />
  }

  return (
    <pre className="text-ink-muted overflow-x-auto font-mono text-[10px] leading-relaxed whitespace-pre-wrap">
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
  sessionId?: string
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
      <pre className="overflow-x-auto font-mono text-[10px] leading-relaxed whitespace-pre-wrap">
        {safeContent.split("\n").map((line, i) => {
          let className = ""
          if (line.startsWith("+")) className = "text-emerald-500"
          else if (line.startsWith("-")) className = "text-red-500"
          else if (line.startsWith("@")) className = "text-ink-muted/50"
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
  sessionId?: string
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
      const base = getApiBaseUrl()
      const res = await fetch(`${base}/api/ai/sessions/${sessionId}/tool-results/${toolCallId}`)
      if (res.ok) {
        const data = await res.json()
        setFullContent(data.content ?? content)
      }
    } catch {
      // Fall back to the content we already have
    } finally {
      setLoading(false)
    }
  }

  return (
    <div>
      <pre className="text-ink-muted max-h-96 overflow-y-auto font-mono text-[10px] leading-relaxed whitespace-pre-wrap">
        {displayText}
      </pre>
      {/* Server-side truncation: expand via API */}
      {isTruncated && !fullContent && (
        <button
          className="text-ink-muted/50 hover:text-ink-muted mt-1 font-mono text-[10px] transition-colors"
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
          className="text-ink-muted/50 hover:text-ink-muted mt-1 font-mono text-[10px] transition-colors"
          onClick={() => setShowFull(true)}
        >
          展开全部（{content.length} 字符）
        </button>
      )}
    </div>
  )
}

function StatusBadge({ status }: { status: ToolCall["status"] }) {
  switch (status) {
    case "streaming":
      return (
        <span className="text-ai flex items-center gap-1 text-[10px]">
          <Loader2 className="size-3 animate-spin" />
          调用中
        </span>
      )
    case "running":
      return (
        <span className="flex items-center gap-1 text-[10px] text-amber-500">
          <RefreshCw className="size-3 animate-spin" />
          执行中
        </span>
      )
    case "success":
      return (
        <span className="flex items-center gap-1 text-[10px] text-emerald-500">
          <Check className="size-3" />
          完成
        </span>
      )
    case "error":
      return (
        <span className="flex items-center gap-1 text-[10px] text-red-500">
          <XCircle className="size-3" />
          失败
        </span>
      )
    default:
      return null
  }
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div>
      <div className="text-ink-muted/70 mb-1 font-mono text-[10px] font-medium">{title}</div>
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
