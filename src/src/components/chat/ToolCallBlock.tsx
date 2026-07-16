import { useState } from "react"
import { Check, Loader2, ChevronDown, ChevronRight, XCircle } from "lucide-react"

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

  const getStatusIcon = (tc: ToolCall) => {
    const status = tc.status ?? (isStreaming ? "streaming" : "success")
    switch (status) {
      case "streaming":
        return <Loader2 className="text-ai size-3.5 shrink-0 animate-spin" />
      case "running":
        return <Loader2 className="text-amber-500 size-3.5 shrink-0 animate-spin" />
      case "success":
        return <Check className="text-emerald-500 size-3.5 shrink-0" />
      case "error":
        return <XCircle className="text-red-500 size-3.5 shrink-0" />
    }
  }

  const getStatusLabel = (tc: ToolCall): string => {
    const status = tc.status ?? (isStreaming ? "streaming" : "success")
    switch (status) {
      case "streaming":
        return "等待执行…"
      case "running":
        return "执行中…"
      case "success":
        return "已完成"
      case "error":
        return "失败"
    }
  }

  return (
    <div className="mb-3 flex flex-col gap-2">
      {toolCalls
        .filter((tc) => tc.name)
        .map((tc, i) => {
          const isExpanded = expanded[tc.id] ?? false
          return (
            <div
              key={tc.id || i}
              className="border-hairline bg-bg-hover/50 rounded-lg border transition-colors"
            >
              <button
                className="flex w-full items-center gap-2 px-3 py-2 text-left text-xs transition-colors hover:brightness-110"
                onClick={() => toggleExpand(tc.id)}
              >
                {getStatusIcon(tc)}

                <span className="text-ink-muted font-mono text-[11px] font-medium">
                  {tc.name}
                </span>

                <span className="text-ink-muted/50 text-[10px]">{getStatusLabel(tc)}</span>

                {(tc.args || tc.result) && (
                  <span className="ml-auto">
                    {isExpanded ? (
                      <ChevronDown className="text-ink-muted size-3" />
                    ) : (
                      <ChevronRight className="text-ink-muted size-3" />
                    )}
                  </span>
                )}
              </button>

              {/* Arguments (collapsible) */}
              {(tc.args || tc.result) && isExpanded && (
                <div className="border-hairline border-t px-3 py-2">
                  {tc.args && (
                    <>
                      <div className="text-ink-muted mb-1 font-mono text-[10px] font-medium">
                        参数
                      </div>
                      <pre className="text-ink-muted max-h-48 overflow-x-auto whitespace-pre-wrap font-mono text-[10px] leading-relaxed">
                        {formatArgs(tc.args)}
                      </pre>
                    </>
                  )}
                  {tc.result && (
                    <>
                      <div className="text-ink-muted mb-1 mt-2 font-mono text-[10px] font-medium">
                        结果
                      </div>
                      <pre className="text-ink-muted max-h-48 overflow-x-auto whitespace-pre-wrap font-mono text-[10px] leading-relaxed">
                        {tc.result.length > 500
                          ? tc.result.slice(0, 500) + "\n..."
                          : tc.result}
                      </pre>
                    </>
                  )}
                </div>
              )}
            </div>
          )
        })}
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
