import { type ReactNode } from "react"
import { Loader2, Sparkles, CheckCircle2, XCircle } from "lucide-react"

// ── Types ────────────────────────────────────────────────

export type AgentPhase = "idle" | "starting" | "processing" | "settled" | "error"

interface AgentStatusBarProps {
  /** Current agent lifecycle phase. */
  phase: AgentPhase
  /** Optional error message when phase is "error". */
  errorMessage?: string
}

// ── Phase descriptors ────────────────────────────────────

interface PhaseVisual {
  icon: ReactNode
  label: string
  color: string                       /* Tailwind text color */
  barColor: string                    /* Tailwind bg for progress bar */
  pulse: boolean
}

function phaseVisual(phase: AgentPhase): PhaseVisual {
  switch (phase) {
    case "idle":
      return {
        icon: <Sparkles className="size-3 text-ink-muted/40" />,
        label: "等待输入",
        color: "text-ink-muted/40",
        barColor: "bg-transparent",
        pulse: false,
      }
    case "starting":
      return {
        icon: <Sparkles className="size-3.5 animate-pulse text-ai" />,
        label: "开始处理",
        color: "text-ai",
        barColor: "bg-ai/40",
        pulse: true,
      }
    case "processing":
      return {
        icon: <Loader2 className="size-3.5 animate-spin text-ai" />,
        label: "处理中",
        color: "text-ai",
        barColor: "bg-ai/40",
        pulse: true,
      }
    case "settled":
      return {
        icon: <CheckCircle2 className="size-3.5 text-emerald-500" />,
        label: "处理完成",
        color: "text-emerald-500",
        barColor: "bg-emerald-500/60",
        pulse: false,
      }
    case "error":
      return {
        icon: <XCircle className="size-3.5 text-red-500" />,
        label: "处理出错",
        color: "text-red-500",
        barColor: "bg-red-500/60",
        pulse: false,
      }
  }
}

// ── Main Component ───────────────────────────────────────

export default function AgentStatusBar({ phase, errorMessage }: AgentStatusBarProps) {
  const vis = phaseVisual(phase)

  // Only render while the agent is actively working or has errored. The
  // completed assistant message itself signals success, so a lingering
  // "处理完成" banner (like `idle`) is hidden — matching Claude/ChatGPT,
  // which show no persistent status bar once a turn is done.
  if (phase === "idle" || phase === "settled") return null

  return (
    <div className="flex items-center gap-2 px-0 py-1">
      {/* Phase indicator dot + label */}
      <div className={`flex items-center gap-2 ${vis.color} transition-colors duration-300`}>
        {vis.icon}
        <span className="font-mono text-caption font-medium tracking-wide">{vis.label}</span>
      </div>

      {/* Animated progress bar (during active phases) */}
      {(phase === "starting" || phase === "processing") && (
        <div className="flex-1 h-[2px] rounded-full bg-hairline overflow-hidden">
          <div
            className={`h-full rounded-full ${vis.barColor} transition-all duration-1000`}
            style={{
              width: phase === "processing" ? "60%" : "20%",
              animation: phase === "processing"
                ? "agent-progress 2s ease-in-out infinite"
                : "agent-progress-start 1s ease-in-out infinite",
            }}
          />
        </div>
      )}

      {/* Error message */}
      {phase === "error" && errorMessage && (
        <span className="text-ink-muted/60 ml-1 truncate font-mono text-caption">{errorMessage}</span>
      )}
    </div>
  )
}
