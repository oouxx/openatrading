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

const COLLAPSE_THRESHOLD = 500

export default function ToolResultBlock({ results }: ToolResultBlockProps) {
  const [expanded, setExpanded] = useState<Record<number, boolean>>({})
  const [showFull, setShowFull] = useState<Record<number, boolean>>({})

  if (!results || results.length === 0) return null

  const toggleExpand = (idx: number) => {
    setExpanded((prev) => ({ ...prev, [idx]: !prev[idx] }))
  }

  return (
    <div className="my-2 flex flex-col gap-2">
      {results.map((tr, i) => {
        const isExpanded = expanded[i] ?? false
        const isLong = tr.result.length > COLLAPSE_THRESHOLD
        const isFullShown = showFull[i] ?? false

        return (
          <div
            key={i}
            className={`border-hairline overflow-hidden rounded-lg border transition-all duration-200 ${
              tr.isError
                ? "border-danger/30 bg-danger/10"
                : "border-success/20 bg-success/10"
            }`}
          >
            <button
              className="hover:bg-hover/70 flex w-full items-center gap-2 px-3 py-2 text-left transition-colors"
              onClick={() => toggleExpand(i)}
            >
              {tr.isError ? (
                <XCircle className="size-3.5 shrink-0 text-danger" />
              ) : (
                <Check className="size-3.5 shrink-0 text-success" />
              )}
              <span className="text-secondary font-number text-caption font-medium">
                {tr.name || "工具结果"}
              </span>
              {!isExpanded && (
                <span className="text-secondary/40 font-number text-caption">
                  {tr.result.length} 字符
                </span>
              )}
              <span className="ml-auto">
                {isExpanded ? (
                  <ChevronDown className="size-3 text-secondary/50" />
                ) : (
                  <ChevronRight className="size-3 text-secondary/50" />
                )}
              </span>
            </button>

            {tr.result && isExpanded && (
              <div className="border-hairline border-t px-3 py-2">
                <pre className="text-secondary max-h-96 overflow-y-auto font-number text-caption leading-relaxed whitespace-pre-wrap">
                  {isLong && !isFullShown
                    ? tr.result.slice(0, COLLAPSE_THRESHOLD) + "\n…"
                    : tr.result}
                </pre>
                {isLong && !isFullShown && (
                  <button
                    className="text-secondary/50 hover:text-secondary mt-1 font-number text-caption transition-colors"
                    onClick={() => setShowFull((prev) => ({ ...prev, [i]: true }))}
                  >
                    展开全部（{tr.result.length} 字符）
                  </button>
                )}
              </div>
            )}
          </div>
        )
      })}
    </div>
  )
}
