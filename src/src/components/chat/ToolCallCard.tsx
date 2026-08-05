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
        return <Loader2 className="size-3.5 animate-spin text-warning" />
      case "success":
        return <CheckCircle2 className="size-3.5 text-success" />
      case "error":
        return <XCircle className="size-3.5 text-danger" />
      default:
        return <Loader2 className="text-secondary size-3.5" />
    }
  }

  const argsStr = args ? (typeof args === "string" ? args : JSON.stringify(args, null, 2)) : ""

  return (
    <div className="bg-surface border-hairline my-2 overflow-hidden rounded-lg border text-caption">
      {/* Header */}
      <div className="border-border/50 flex items-center gap-2 border-b px-3 py-2">
        <Terminal className="text-secondary size-3.5" />
        <span className="text-primary font-medium">{name}</span>
        <div className="ml-auto flex items-center gap-2">
          {statusIcon()}
          <span className="text-secondary text-caption capitalize">
            {status ?? "pending"}
          </span>
        </div>
      </div>

      {/* Arguments (collapsible) */}
      {argsStr && (
        <div className="border-border/50 border-b">
          <button
            className="hover:bg-hover flex w-full items-center gap-2 px-3 py-2 text-left transition-colors"
            onClick={() => setShowArgs(!showArgs)}
          >
            {showArgs ? <ChevronDown className="size-3" /> : <ChevronRight className="size-3" />}
            <span className="text-secondary font-number text-caption">Arguments</span>
          </button>
          {showArgs && (
            <pre className="bg-hover text-secondary overflow-x-auto px-3 py-2 font-number text-caption leading-relaxed">
              {argsStr}
            </pre>
          )}
        </div>
      )}

      {/* Result (collapsible) */}
      {result && (
        <div>
          <button
            className={`hover:bg-hover flex w-full items-center gap-2 px-3 py-2 text-left transition-colors ${
              isError ? "text-danger" : ""
            }`}
            onClick={() => setShowResult(!showResult)}
          >
            {showResult ? <ChevronDown className="size-3" /> : <ChevronRight className="size-3" />}
            <span className="font-number text-caption">
              {isError ? "Error" : "Result"}
            </span>
          </button>
          {showResult && (
            <pre className={`overflow-x-auto px-3 py-2 font-number text-caption leading-relaxed ${
              isError ? "bg-red-950/20 text-red-400" : "bg-hover text-secondary"
            }`}>
              {result}
            </pre>
          )}
        </div>
      )}
    </div>
  )
}
