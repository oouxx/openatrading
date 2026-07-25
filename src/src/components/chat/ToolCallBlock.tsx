import { useState, useMemo } from "react"
import { Check, Loader2, ChevronDown, ChevronRight, XCircle, Wrench, RefreshCw, Terminal, FileEdit } from "lucide-react"
import { diffLines, formatLines } from "diff"
import { getApiBaseUrl } from "@/stores/data"

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

  // Auto-expand running/streaming tool calls
  const isAutoExpanded = (tc: ToolCall) => {
    return tc.status === "running" || tc.status === "streaming"
  }

  return (
    <div className="my-2 flex flex-col gap-1.5">
      {toolCalls
        .filter((tc) => tc.name)
        .map((tc, i) => {
          const isExpanded = expanded[tc.id] ?? isAutoExpanded(tc) ?? false
          const status = tc.status ?? (isStreaming ? "streaming" : "success")
          const hasPartial = tc.partialResult && tc.partialResult.length > 0
          const hasArgs = tc.args && tc.args.length > 0
          const hasContent = hasArgs || hasPartial

          return (
            <div
              key={tc.id || i}
              className={`border-hairline overflow-hidden rounded-lg border transition-all duration-200 ${
                status === "running"
                  ? "border-amber-500/30 bg-amber-500/5"
                  : status === "streaming"
                    ? "border-ai/20 bg-ai/5"
                    : status === "error"
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

              {hasContent && (
                <div
                  className={`border-hairline space-y-2 border-t px-3 py-2 ${
                    isExpanded ? "" : "hidden"
                  }`}
                >
                  {hasArgs && (
                    <Section title="参数">
                      <ToolArgsContent name={tc.name} args={tc.args!} />
                    </Section>
                  )}
                  {hasPartial && (
                    <Section title="中间结果">
                      <ContentBlock
                        content={tc.partialResult!}
                        showCursor={status === "running"}
                      />
                    </Section>
                  )}
                  {/* Tool result with truncation support (§15.2) */}
                  {tc.result && (
                    <Section title="结果">
                      <ToolResultContent
                        name={tc.name}
                        content={tc.result}
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

// ── Tool card dispatch by name (§6.3) ──────────────────────────────

function ToolIcon({ name }: { name: string }) {
  switch (name) {
    case "bash":
    case "command":
    case "terminal":
      return <Terminal className="text-ink-muted size-3.5 shrink-0" />
    case "edit":
    case "write":
    case "edit_file":
    case "write_file":
      return <FileEdit className="text-ink-muted size-3.5 shrink-0" />
    default:
      return <Wrench className="text-ink-muted size-3.5 shrink-0" />
  }
}

function ToolArgsContent({ name, args }: { name: string; args: string }) {
  switch (name) {
    case "bash":
    case "command":
    case "terminal":
      // Show bash commands in a terminal-like code block
      return (
        <pre className="bg-bg-hover overflow-x-auto rounded-md p-2 font-mono text-[10px] leading-relaxed whitespace-pre-wrap">
          <code>$ {formatArgs(args)}</code>
        </pre>
      )
    case "edit":
    case "write":
    case "edit_file":
    case "write_file":
      // Show edit args as structured content
      return <ContentBlock content={formatArgs(args)} />
    default:
      return <ContentBlock content={formatArgs(args)} />
  }
}

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
  switch (name) {
    case "edit":
    case "write":
    case "edit_file":
    case "write_file": {
      // For edit tools, try to render a diff view
      return <DiffView content={content} />
    }
    default:
      return (
        <ResultBlock
          content={content}
          toolCallId={toolCallId}
          sessionId={sessionId}
          output={output}
        />
      )
  }
}

/** Simple diff view for edit tool results (§6.3). */
function DiffView({ content }: { content: string }) {
  const diffResult = useMemo(() => {
    // Try to parse the content as a unified diff or structured edit result
    // If it contains "---" and "+++" lines, render as diff
    const lines = content.split("\n")
    const hasDiffMarkers = lines.some((l) => l.startsWith("---") || l.startsWith("+++"))
    if (hasDiffMarkers) {
      try {
        const changes = diffLines(content, "")
        return { type: "diff" as const, changes }
      } catch {
        return { type: "raw" as const, content }
      }
    }
    return { type: "raw" as const, content }
  }, [content])

  if (diffResult.type === "diff") {
    return (
      <pre className="overflow-x-auto font-mono text-[10px] leading-relaxed whitespace-pre-wrap">
        {content.split("\n").map((line, i) => {
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

  return <ResultBlock content={content} toolCallId="" />
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
  const displayText = fullContent ?? (isTruncated ? (output?.preview ?? content) : content)

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

/** Renders content, collapsing if too long */
function ContentBlock({
  content,
  showCursor,
}: {
  content: string
  showCursor?: boolean
}) {
  const [showFull, setShowFull] = useState(false)
  const isLong = content.length > COLLAPSE_THRESHOLD

  // Default: collapsed if long, expanded if short
  const effectivelyExpanded = isLong ? showFull : true
  const displayText = effectivelyExpanded ? content : content.slice(0, 300)

  return (
    <div>
      <pre
        className={`text-ink-muted overflow-x-auto font-mono text-[10px] leading-relaxed whitespace-pre-wrap ${
          effectivelyExpanded ? "max-h-none" : "max-h-48"
        }`}
      >
        {displayText}
        {isLong && !showFull && (
          <span className="text-ink-muted/50">…</span>
        )}
        {showCursor && (
          <span className="text-ai ml-0.5 inline-block h-2.5 w-1 animate-pulse align-middle">
            |
          </span>
        )}
      </pre>
      {isLong && (
        <button
          className="text-ink-muted/50 hover:text-ink-muted mt-1 font-mono text-[10px] transition-colors"
          onClick={() => setShowFull(!showFull)}
        >
          {showFull ? "收起" : `展开全部 (${content.length} 字符)`}
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
