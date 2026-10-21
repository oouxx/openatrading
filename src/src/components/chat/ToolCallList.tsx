import { useMemo, useState } from "react"
import { Button } from "@/components/ui/button"
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
import { summarizeTool } from "@/lib/tool-summary"

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
      className={`rounded-lg border transition-colors ${
        status === "error" ? "border-danger/30 bg-danger/10" : "border-hairline bg-hover/50"
      }`}
    >
      <Button
        variant="ghost"
        className="hover:bg-hover/60 h-auto w-full justify-start gap-2.5 rounded-none px-3 py-2 text-left font-normal whitespace-normal"
        onClick={() => setExpanded((v) => !v)}
      >
        {showIcon && (
          <span className="bg-hover flex size-5 shrink-0 items-center justify-center rounded-md">
            <ToolIcon name={tc.name} className="size-3" />
          </span>
        )}
        <span className="text-primary text-body min-w-0 flex-1 truncate font-medium">
          {summarizeTool(tc.name, tc.args)}
        </span>
        <span className="text-tertiary font-number text-caption hidden shrink-0 sm:inline">
          {tc.name}
        </span>
        <StatusIcon status={status} />
        <span className="text-tertiary shrink-0">
          {expanded ? <ChevronDown className="size-3.5" /> : <ChevronRight className="size-3.5" />}
        </span>
      </Button>

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
    return <FileEdit className={`${cls} text-warning shrink-0`} />
  }
  if (name.startsWith("bash") || name === "execute_command" || name === "run") {
    return <Terminal className={`${cls} text-info shrink-0`} />
  }
  return <Wrench className={`${cls} text-secondary shrink-0`} />
}

// ── Tool args content ─────────────────────────────────

function ToolArgsContent({ name, args }: { name: string; args: string }) {
  const safeContent = useMemo(() => formatArgs(args), [args])

  if (name.startsWith("edit") || name === "file_edit") {
    return <DiffContent content={safeContent} />
  }

  return (
    <pre className="text-secondary font-number text-caption overflow-x-auto leading-relaxed whitespace-pre-wrap">
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

  return (
    <ResultBlock
      content={safeContent}
      toolCallId={toolCallId}
      sessionId={sessionId}
      output={output}
    />
  )
}

// ── Diff content ─────────────────────────────────────

function DiffContent({ content }: { content: string }) {
  const safeContent = useMemo(() => asText(content), [content])

  const diffResult = useMemo(() => {
    const hasDiffMarkers = safeContent
      .split("\n")
      .some((l) => l.startsWith("+") || l.startsWith("-") || l.startsWith("@"))
    if (hasDiffMarkers) {
      return { type: "diff" as const }
    }
    return { type: "raw" as const, content: safeContent }
  }, [safeContent])

  if (diffResult.type === "diff") {
    return (
      <pre className="font-number text-caption overflow-x-auto leading-relaxed whitespace-pre-wrap">
        {safeContent.split("\n").map((line, i) => {
          let className = ""
          if (line.startsWith("+")) className = "text-success"
          else if (line.startsWith("-")) className = "text-danger"
          else if (line.startsWith("@")) className = "text-tertiary"
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
      (isTruncated
        ? (output?.preview ?? content)
        : isLong && !showFull
          ? content.slice(0, COLLAPSE_THRESHOLD) + "\n…"
          : content)
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
      <pre className="text-secondary font-number text-caption max-h-96 overflow-y-auto leading-relaxed whitespace-pre-wrap">
        {displayText}
      </pre>
      {/* Server-side truncation: expand via API */}
      {isTruncated && !fullContent && (
        <Button
          variant="link"
          size="xs"
          className="text-tertiary hover:text-secondary font-number h-auto p-0"
          onClick={expandFromServer}
          disabled={loading}
        >
          {loading ? "加载中..." : `展开全部（${output?.fullLength ?? content.length} 字符）`}
        </Button>
      )}
      {/* Client-side truncation: expand locally */}
      {isLong && !showFull && !isTruncated && (
        <Button
          variant="link"
          size="xs"
          className="text-tertiary hover:text-secondary font-number mt-1 h-auto p-0"
          onClick={() => setShowFull(true)}
        >
          展开全部（{content.length} 字符）
        </Button>
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
      return <RefreshCw className="text-warning size-3.5 shrink-0 animate-spin" />
    case "success":
      return <Check className="text-success size-3.5 shrink-0" />
    case "error":
      return <XCircle className="text-danger size-3.5 shrink-0" />
    default:
      return null
  }
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div>
      <div className="text-tertiary font-number text-caption mb-1 font-medium">{title}</div>
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
