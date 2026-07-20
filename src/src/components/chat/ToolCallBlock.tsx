import { useState } from "react"
import { Check, Loader2, ChevronDown, ChevronRight, XCircle, Wrench, RefreshCw } from "lucide-react"

interface ToolCall {
  id: string
  name: string
  args?: string
  result?: string
  partialResult?: string
  status?: "streaming" | "running" | "success" | "error"
}

interface ToolCallBlockProps {
  toolCalls: ToolCall[]
  isStreaming?: boolean
}

export default function ToolCallBlock({ toolCalls, isStreaming }: ToolCallBlockProps) {
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
                <Wrench className="text-ink-muted size-3.5 shrink-0" />
                <span className="text-ink-muted font-mono text-[11px] font-medium">{tc.name}</span>
                <StatusBadge status={status} />
                {(tc.args || hasPartial) && (
                  <span className="text-ink-muted/50 ml-auto">
                    {isExpanded ? (
                      <ChevronDown className="size-3" />
                    ) : (
                      <ChevronRight className="size-3" />
                    )}
                  </span>
                )}
              </button>

              {(tc.args || hasPartial) && isExpanded && (
                <div className="border-hairline space-y-2 border-t px-3 py-2">
                  {tc.args && (
                    <Section title="参数">
                      <pre className="text-ink-muted max-h-48 overflow-x-auto font-mono text-[10px] leading-relaxed whitespace-pre-wrap">
                        {formatArgs(tc.args)}
                      </pre>
                    </Section>
                  )}
                  {hasPartial && (
                    <Section title="中间结果">
                      <pre className="text-ink-muted max-h-48 overflow-x-auto font-mono text-[10px] leading-relaxed whitespace-pre-wrap">
                        {tc.partialResult!.length > 800
                          ? tc.partialResult!.slice(0, 800) + "\n…"
                          : tc.partialResult}
                        {status === "running" && (
                          <span className="text-ai ml-0.5 inline-block h-2.5 w-1 animate-pulse align-middle">
                            |
                          </span>
                        )}
                      </pre>
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
