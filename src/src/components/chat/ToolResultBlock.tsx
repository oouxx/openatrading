import { useState } from "react"
import { Check, XCircle, ChevronDown, ChevronRight } from "lucide-react"

interface ToolResult {
  name: string
  result: string
  isError?: boolean
}

interface ToolResultBlockProps {
  results: ToolResult[]
}

export default function ToolResultBlock({ results }: ToolResultBlockProps) {
  const [expanded, setExpanded] = useState<Record<number, boolean>>({})

  if (!results || results.length === 0) return null

  const toggleExpand = (idx: number) => {
    setExpanded((prev) => ({ ...prev, [idx]: !prev[idx] }))
  }

  return (
    <div className="my-2 flex flex-col gap-1.5">
      {results.map((tr, i) => {
        const isExpanded = expanded[i] ?? false
        const truncated = tr.result.length > 500
        const displayText = isExpanded ? tr.result : tr.result.slice(0, 200)

        return (
          <div
            key={i}
            className={`border-hairline overflow-hidden rounded-lg border transition-all duration-200 ${
              tr.isError
                ? "border-red-500/30 bg-red-500/5"
                : "border-emerald-500/20 bg-emerald-500/5"
            }`}
          >
            <button
              className="hover:bg-bg-hover/70 flex w-full items-center gap-2 px-3 py-2 text-left transition-colors"
              onClick={() => toggleExpand(i)}
            >
              {tr.isError ? (
                <XCircle className="size-3.5 shrink-0 text-red-500" />
              ) : (
                <Check className="size-3.5 shrink-0 text-emerald-500" />
              )}
              <span className="text-ink-muted font-mono text-[11px] font-medium">
                {tr.name || "工具结果"}
              </span>
              <span className="text-ink-muted/50 text-[10px]">
                {tr.result.length} 字符
              </span>
              {truncated && (
                <span className="ml-auto">
                  {isExpanded ? (
                    <ChevronDown className="size-3 text-ink-muted/50" />
                  ) : (
                    <ChevronRight className="size-3 text-ink-muted/50" />
                  )}
                </span>
              )}
            </button>

            {tr.result && (
              <div className="border-hairline border-t px-3 py-2">
                <pre className="text-ink-muted max-h-48 overflow-x-auto font-mono text-[10px] leading-relaxed whitespace-pre-wrap">
                  {displayText}
                  {truncated && !isExpanded && (
                    <span className="text-ink-muted/50">…</span>
                  )}
                </pre>
              </div>
            )}
          </div>
        )
      })}
    </div>
  )
}
