import { useState } from "react"
import { ChevronDown, ChevronRight, Terminal, CheckCircle2, XCircle, Loader2 } from "lucide-react"

interface ToolCallCardProps {
  name: string
  args?: any
  status?: "running" | "success" | "error"
  result?: string
  isError?: boolean
}

export default function ToolCallCard({ name, args, status, result, isError }: ToolCallCardProps) {
  const [showArgs, setShowArgs] = useState(false)
  const [showResult, setShowResult] = useState(false)

  const statusIcon = () => {
    switch (status) {
      case "running":
        return <Loader2 className="size-3.5 animate-spin text-amber-500" />
      case "success":
        return <CheckCircle2 className="size-3.5 text-emerald-500" />
      case "error":
        return <XCircle className="size-3.5 text-red-500" />
      default:
        return <Loader2 className="text-muted-foreground size-3.5" />
    }
  }

  const argsStr = args ? (typeof args === "string" ? args : JSON.stringify(args, null, 2)) : ""

  return (
    <div className="bg-bg-surface border-hairline my-2 overflow-hidden rounded-lg border text-xs">
      {/* Header */}
      <div className="border-border/50 flex items-center gap-2 border-b px-3 py-2">
        <Terminal className="text-muted-foreground size-3.5" />
        <span className="text-foreground font-medium">{name}</span>
        <div className="ml-auto flex items-center gap-2">
          {statusIcon()}
          <span className="text-muted-foreground text-caption capitalize">
            {status ?? "pending"}
          </span>
        </div>
      </div>

      {/* Arguments (collapsible) */}
      {argsStr && (
        <div className="border-border/50 border-b">
          <button
            className="hover:bg-bg-hover flex w-full items-center gap-2 px-3 py-2 text-left transition-colors"
            onClick={() => setShowArgs(!showArgs)}
          >
            {showArgs ? <ChevronDown className="size-3" /> : <ChevronRight className="size-3" />}
            <span className="text-muted-foreground font-mono text-caption">Arguments</span>
          </button>
          {showArgs && (
            <pre className="bg-bg-hover text-muted-foreground overflow-x-auto px-3 py-2 font-mono text-caption leading-relaxed">
              {argsStr}
            </pre>
          )}
        </div>
      )}

      {/* Result (collapsible) */}
      {result && (
        <div>
          <button
            className={`hover:bg-bg-hover flex w-full items-center gap-2 px-3 py-2 text-left transition-colors ${
              isError ? "text-red-500" : ""
            }`}
            onClick={() => setShowResult(!showResult)}
          >
            {showResult ? <ChevronDown className="size-3" /> : <ChevronRight className="size-3" />}
            <span className="font-mono text-caption">
              {isError ? "Error" : "Result"}
            </span>
          </button>
          {showResult && (
            <pre className={`overflow-x-auto px-3 py-2 font-mono text-caption leading-relaxed ${
              isError ? "bg-red-950/20 text-red-400" : "bg-bg-hover text-muted-foreground"
            }`}>
              {result}
            </pre>
          )}
        </div>
      )}
    </div>
  )
}
