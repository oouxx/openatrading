import { useState } from "react"
import { Check, Loader2, ChevronDown, ChevronRight, XCircle, Wrench } from "lucide-react"

interface ToolCall {
  id: string
  name: string
  args?: string
  result?: string
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

  return (
    <div className="my-2 flex flex-col gap-1.5">
      {toolCalls
        .filter((tc) => tc.name)
        .map((tc, i) => {
          const isExpanded = expanded[tc.id] ?? false
          const status = tc.status ?? (isStreaming ? "streaming" : "success")
          return (
            <div
              key={tc.id || i}
              className="border-hairline bg-bg-hover/40 overflow-hidden rounded-lg border"
            >
              <button
                className="hover:bg-bg-hover/70 flex w-full items-center gap-2 px-3 py-2 text-left transition-colors"
                onClick={() => toggleExpand(tc.id)}
              >
                <Wrench className="text-ink-muted size-3.5 shrink-0" />
                <span className="text-ink-muted font-mono text-[11px] font-medium">{tc.name}</span>
                <StatusBadge status={status} />
                {(tc.args || tc.result) && (
                  <span className="text-ink-muted/50 ml-auto">
                    {isExpanded ? (
                      <ChevronDown className="size-3" />
                    ) : (
                      <ChevronRight className="size-3" />
                    )}
                  </span>
                )}
              </button>

              {(tc.args || tc.result) && isExpanded && (
                <div className="border-hairline space-y-2 border-t px-3 py-2">
                  {tc.args && (
                    <Section title="参数">
                      <pre className="text-ink-muted max-h-48 overflow-x-auto font-mono text-[10px] leading-relaxed whitespace-pre-wrap">
                        {formatArgs(tc.args)}
                      </pre>
                    </Section>
                  )}
                  {tc.result && (
                    <Section title="结果">
                      <pre className="text-ink-muted max-h-60 overflow-x-auto font-mono text-[10px] leading-relaxed whitespace-pre-wrap">
                        {tc.result.length > 800 ? tc.result.slice(0, 800) + "\n…" : tc.result}
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
          <Loader2 className="size-3 animate-spin" />
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
