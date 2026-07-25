import { useState, useEffect } from "react"
import { Loader2, Check, XCircle } from "lucide-react"

interface CompactionIndicatorProps {
  status: "idle" | "compacting" | "done" | "error"
  reason?: string
  error?: string | null
}

export default function CompactionIndicator({ status, reason, error }: CompactionIndicatorProps) {
  const [visible, setVisible] = useState(status !== "idle")

  useEffect(() => {
    if (status === "compacting") {
      setVisible(true)
    } else if (status === "done" || status === "error") {
      const timer = setTimeout(() => setVisible(false), 2000)
      return () => clearTimeout(timer)
    }
  }, [status])

  if (!visible) return null

  return (
    <div
      className={`border-hairline mx-2 mb-2 overflow-hidden rounded-lg border transition-all duration-300 ${
        status === "compacting"
          ? "border-amber-500/30 bg-amber-500/5"
          : status === "error"
            ? "border-red-500/30 bg-red-500/5"
            : "border-emerald-500/20 bg-emerald-500/5"
      }`}
    >
      <div className="flex items-center gap-2 px-3 py-2">
        {status === "compacting" ? (
          <Loader2 className="size-3.5 shrink-0 animate-spin text-amber-500" />
        ) : status === "error" ? (
          <XCircle className="size-3.5 shrink-0 text-red-500" />
        ) : (
          <Check className="size-3.5 shrink-0 text-emerald-500" />
        )}
        <span className="text-ink-muted flex-1 text-[11px] font-medium">
          {status === "compacting"
            ? "正在压缩会话历史…"
            : status === "error"
              ? `压缩失败: ${error || "未知错误"}`
              : "会话压缩完成"}
        </span>
        {reason && status === "compacting" && (
          <span className="text-ink-muted/50 text-[10px]">
            {reason === "threshold" ? "阈值触发" : reason === "overflow" ? "上下文溢出" : "手动"}
          </span>
        )}
      </div>
      {status === "compacting" && (
        <div className="bg-amber-500/20 h-0.5">
          <div
            className="h-full animate-pulse rounded-full bg-amber-500 transition-all duration-500"
            style={{
              width: "60%",
              animation: "compaction-progress 2s ease-in-out infinite",
            }}
          />
        </div>
      )}
    </div>
  )
}
