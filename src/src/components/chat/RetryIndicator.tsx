import { useState, useEffect } from "react"
import { RefreshCw, Loader2, Check, XCircle, Clock } from "lucide-react"

interface RetryIndicatorProps {
  status: "idle" | "retrying" | "will-retry" | "done" | "error"
  attempt?: number
  maxAttempts?: number
  delayMs?: number
  error?: string | null
}

export default function RetryIndicator({
  status,
  attempt,
  maxAttempts,
  delayMs,
  error,
}: RetryIndicatorProps) {
  const [visible, setVisible] = useState(status !== "idle")

  useEffect(() => {
    if (status === "retrying" || status === "will-retry") {
      setVisible(true)
    } else if (status === "done" || status === "error") {
      const timer = setTimeout(() => setVisible(false), 3000)
      return () => clearTimeout(timer)
    }
  }, [status])

  if (!visible) return null

  return (
    <div
      className={`border-hairline mx-2 mb-2 overflow-hidden rounded-lg border transition-all duration-300 ${
        status === "retrying" || status === "will-retry"
          ? "border-sky-500/30 bg-sky-500/5"
          : status === "error"
            ? "border-red-500/30 bg-red-500/5"
            : "border-emerald-500/20 bg-emerald-500/5"
      }`}
    >
      <div className="flex items-center gap-2 px-3 py-2">
        {status === "retrying" ? (
          <Loader2 className="size-3.5 shrink-0 animate-spin text-sky-500" />
        ) : status === "will-retry" ? (
          <RefreshCw className="size-3.5 shrink-0 animate-pulse text-sky-500" />
        ) : status === "error" ? (
          <XCircle className="size-3.5 shrink-0 text-red-500" />
        ) : (
          <Check className="size-3.5 shrink-0 text-emerald-500" />
        )}
        <span className="text-ink-muted flex-1 text-[11px] font-medium">
          {status === "will-retry"
            ? "遇到错误，正在准备重试…"
            : status === "retrying"
              ? `正在重试 (第 ${attempt}/${maxAttempts} 次)…`
              : status === "error"
                ? `重试失败: ${error || "已达最大重试次数"}`
                : "重试成功，继续生成"}
        </span>
        {delayMs && status === "retrying" && (
          <span className="text-ink-muted/50 flex items-center gap-1 text-[10px]">
            <Clock className="size-3" />
            {(delayMs / 1000).toFixed(1)}s
          </span>
        )}
      </div>
      {status === "retrying" && (
        <div className="bg-sky-500/20 h-0.5">
          <div
            className="h-full animate-pulse rounded-full bg-sky-500 transition-all duration-500"
            style={{
              width: `${((attempt ?? 1) / (maxAttempts ?? 3)) * 100}%`,
              animation: "compaction-progress 1.5s ease-in-out infinite",
            }}
          />
        </div>
      )}
    </div>
  )
}
