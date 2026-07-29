import { useState, useMemo } from "react"
import { Check, Loader2, ChevronDown, ChevronRight, XCircle, Wrench, RefreshCw, Terminal, FileEdit } from "lucide-react"
import { diffLines } from "diff"
import { getApiBaseUrl } from "@/stores/data"

/**
 * Coerce any tool result / partial-result payload to a renderable string.
 *
 * The backend serializes tool results as `serde_json::Value`, so they may
 * arrive as objects/arrays (e.g. `{ content, details }`). Rendering a non-string
 * directly crashes React ("Objects are not valid as a React child"). This is a
 * defensive guard at the render layer — the reducer also normalizes, but stale
 * in-memory turns (from before a hot reload) or future event-shape changes must
 * never white-screen the whole app.
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
  /** Preview info for truncated tool results (§15). */
  output?: { preview: string; truncated: boolean; fullLength: number }
}

interface ToolCallBlockProps {
  toolCalls: ToolCall[]
  isStreaming?: boolean
  sessionId?: string
}

const COLLAPSE_THRESHOLD = 500

export default function ToolCallBlock({ toolCalls, isStreaming, sessionId }: ToolCallBlockProps) {
  const [expanded, setExpanded] = useState<Record<string, boolean>>({})

  if (!toolCalls || toolCalls.length === 0) return null

  const toggleExpand = (id: string) => {
    setExpanded((prev) => ({ ...prev, [id]: !prev[id] }))
  }

  // Auto-expand completed tool calls (Claude-style: show details only after done)
  const isAutoExpanded = (tc: ToolCall) => {
    return tc.status === "success" || tc.status === "error"
  }

  return (
    <div className="my-2 flex flex-col gap-1.5">
      {toolCalls
        .filter((tc) => tc.name)
        .map((tc, i) => {
          const isExpanded = expanded[tc.id] ?? isAutoExpanded(tc) ?? false
          const status = tc.status ?? (isStreaming ? "streaming" : "success")
          const isRunning = status === "streaming" || status === "running"
          const hasArgs = tc.args && tc.args.length > 0
          const hasResult = !!tc.result
          const hasPartial = !!tc.partialResult
          const hasContent = hasArgs || hasResult

          // ── Running state: compact card + optional partial result ──
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
                {/* ── Partial result inline (ToolExecutionUpdate) ── */}
                {hasPartial && (
                  <div className="border-hairline animate-notice-fade-in border-t px-3 py-1.5">
                    <div className="text-ink-muted/50 mb-0.5 font-mono text-[9px] font-medium tracking-wider">
                      实时输出
                    </div>
                    <pre className="text-ink-muted/80 max-h-[120px] overflow-y-auto font-mono text-[10px] leading-relaxed whitespace-pre-wrap">
                      {asText(tc.partialResult)}
                    </pre>
                    {/* Streaming cursor for live partial result */}
                    <span className="mt-0.5 inline-flex">
                      <span className="bg-ai inline-block size-1.5 animate-pulse rounded-full" />
                    </span>
                  </div>
                )}
              </div>
            )
          }

          // ── Completed state: expandable card ──
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
                {hasContent && (
                  <span className="text-ink-muted/50 ml-auto">
                    {isExpanded ? (
                      <ChevronDown className="size-3" />
                    ) : (
                      <ChevronRight className="size-3" />
                    )}
                  </span>
                )}
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
                        content={asText(tc.result)}
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

// ── Tool icon ──────────────────────────────────────────

function ToolIcon({ name }: { name: string }) {
  const icon = useMemo(() => {
    if (name.startsWith("bash") || name.startsWith("execute") || name.startsWith("run"))
      return <Terminal className="size-3.5" />
    if (name.startsWith("edit") || name === "file_edit" || name.startsWith("write") || name.startsWith("create"))
      return <FileEdit className="size-3.5" />
    return <Wrench className="size-3.5" />
  }, [name])

  return <span className="text-ink-muted/60 shrink-0">{icon}</span>
}

// ── Tool args content ─────────────────────────────────

function ToolArgsContent({ name, args }: { name: string; args: string }) {
  const safeContent = useMemo(() => asText(args), [args])

  // For edit-type tools, show args as a diff preview
  if (name.startsWith("edit") || name === "file_edit") {
    return (
      <pre className="text-ink-muted overflow-x-auto font-mono text-[10px] leading-relaxed whitespace-pre-wrap">
        {safeContent}
      </pre>
    )
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

  // For edit-type tools, render result as a diff
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

/** Renders tool result content with truncation and expand-from-server support (§15.2). */
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
  const [loading, setLoading] = useState(false)

  const isTruncated = output?.truncated ?? false
  const displayText = asText(fullContent ?? (isTruncated ? (output?.preview ?? content) : content))

  const expand = async () => {
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
      <pre className="text-ink-muted overflow-x-auto font-mono text-[10px] leading-relaxed whitespace-pre-wrap">
        {displayText}
      </pre>
      {isTruncated && !fullContent && (
        <button
          className="text-ink-muted/50 hover:text-ink-muted mt-1 font-mono text-[10px] transition-colors"
          onClick={expand}
          disabled={loading}
        >
          {loading
            ? "加载中..."
            : `展开全部（${output?.fullLength ?? content.length} 字符）`}
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
