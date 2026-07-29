import {
  AlertTriangle,
  CheckCircle2,
  Compress,
  Loader2,
  RefreshCw,
  XCircle,
  ArrowRight,
} from "lucide-react"

// ── Types ────────────────────────────────────────────────

interface SystemNotice {
  id: string
  kind: "compaction" | "retry" | "turn" | "notice"
  detail: string
  isError?: boolean
  status?: "active" | "done" | "error"
  attempt?: number
  maxAttempts?: number
  delayMs?: number
}

interface SystemNoticeBlockProps {
  notices: SystemNotice[]
}

// ── Icons per kind ───────────────────────────────────────

function KindIcon({ kind, status }: { kind: SystemNotice["kind"]; status?: SystemNotice["status"] }) {
  switch (kind) {
    case "compaction":
      if (status === "active") return <Compress className="size-3.5 animate-pulse text-blue-500" />
      if (status === "error") return <XCircle className="size-3.5 text-red-500" />
      return <CheckCircle2 className="size-3.5 text-blue-500" />
    case "retry":
      if (status === "active") return <RefreshCw className="size-3.5 animate-spin text-amber-500" />
      if (status === "error") return <XCircle className="size-3.5 text-red-500" />
      return <CheckCircle2 className="size-3.5 text-emerald-500" />
    case "turn":
      return <ArrowRight className="size-3 text-ink-muted/30" />
    case "notice":
      if (status === "error") return <AlertTriangle className="size-3.5 text-red-500" />
      return <CheckCircle2 className="size-3.5 text-emerald-500" />
  }
}

// ── Tailwind classes per kind ────────────────────────────

function cardStyle(kind: SystemNotice["kind"], status?: SystemNotice["status"], isError?: boolean): string {
  if (isError || status === "error") return "border-red-500/25 bg-red-500/[0.04]"
  switch (kind) {
    case "compaction":
      if (status === "active") return "border-blue-500/25 bg-blue-500/[0.04]"
      return "border-blue-500/15 bg-blue-500/[0.02]"
    case "retry":
      if (status === "active") return "border-amber-500/25 bg-amber-500/[0.04]"
      if (status === "done") return "border-emerald-500/15 bg-emerald-500/[0.02]"
      return "border-amber-500/20 bg-amber-500/[0.02]"
    case "turn":
      return "border-transparent"
    case "notice":
      if (status === "error") return "border-red-500/25 bg-red-500/[0.04]"
      return "border-emerald-500/15 bg-emerald-500/[0.02]"
  }
}

// ── Main Component ───────────────────────────────────────

export default function SystemNoticeBlock({ notices }: SystemNoticeBlockProps) {
  if (!notices || notices.length === 0) return null

  return (
    <div className="my-1 flex flex-col gap-1">
      {notices.map((n, i) => (
        <NoticeCard key={n.id || i} notice={n} />
      ))}
    </div>
  )
}

// ── Notice Card ──────────────────────────────────────────

function NoticeCard({ notice }: { notice: SystemNotice }) {
  const { kind, detail, isError, status, attempt, maxAttempts, delayMs } = notice

  // ── Compaction ──────────────────────────────────────────
  if (kind === "compaction") {
    const isActive = status === "active"
    const failed = status === "error"
    return (
      <div className={`border-hairline overflow-hidden rounded-lg border ${cardStyle(kind, status)} transition-all duration-500`}>
        <div className="flex items-center gap-2 px-3 py-1.5">
          <KindIcon kind={kind} status={status} />
          <span className={`font-mono text-[11px] font-medium ${isActive ? "text-blue-500" : failed ? "text-red-500" : "text-ink-muted/70"}`}>
            会话压缩
          </span>
          {isActive && (
            <span className="ml-auto flex items-center gap-1.5">
              <span className="bg-blue-500/30 size-1.5 animate-pulse rounded-full" />
              <span className="text-ink-muted/50 font-mono text-[10px]">压缩中…</span>
            </span>
          )}
          {failed && <span className="ml-auto text-red-500/70 font-mono text-[10px]">压缩失败</span>}
          {status === "done" && <span className="ml-auto text-blue-500/50 font-mono text-[10px]">已完成</span>}
        </div>
        {detail && !isActive && (
          <div className="border-hairline border-t px-3 py-1.5">
            <p className="text-ink-muted/60 font-mono text-[10px] leading-relaxed">{detail}</p>
          </div>
        )}
      </div>
    )
  }

  // ── Retry ───────────────────────────────────────────────
  if (kind === "retry") {
    const isActive = status === "active"
    const failed = status === "error"
    return (
      <div className={`border-hairline overflow-hidden rounded-lg border ${cardStyle(kind, status)} transition-all duration-500`}>
        <div className="flex items-center gap-2 px-3 py-1.5">
          <KindIcon kind={kind} status={status} />
          <span className={`font-mono text-[11px] font-medium ${isActive ? "text-amber-500" : failed ? "text-red-500" : "text-emerald-500"}`}>
            自动重试
          </span>
          {attempt != null && maxAttempts != null && (
            <span className="text-ink-muted/60 font-mono text-[10px]">{attempt}/{maxAttempts}</span>
          )}
          {isActive && (
            <span className="ml-auto flex items-center gap-1.5">
              <Loader2 className="size-3 animate-spin text-amber-500" />
              <span className="text-ink-muted/50 font-mono text-[10px]">重试中…</span>
            </span>
          )}
          {status === "done" && <span className="ml-auto text-emerald-500/70 font-mono text-[10px]">重试成功</span>}
          {failed && <span className="ml-auto text-red-500/70 font-mono text-[10px]">已达上限</span>}
        </div>
        {(detail || delayMs != null) && (
          <div className="border-hairline border-t px-3 py-1.5">
            {detail && <p className="text-ink-muted/60 font-mono text-[10px] leading-relaxed">{detail}</p>}
            {delayMs != null && (
              <p className="text-ink-muted/40 mt-0.5 font-mono text-[10px]">等待 {formatDelay(delayMs)}</p>
            )}
          </div>
        )}
      </div>
    )
  }

  // ── Turn end: minimal separator ─────────────────────────
  if (kind === "turn") {
    return (
      <div className="flex items-center gap-2 px-0 py-0.5">
        <div className="flex-1 border-t border-hairline" />
        {detail && (
          <span className="text-ink-muted/30 shrink-0 font-mono text-[9px] tracking-wider uppercase">
            {detail}
          </span>
        )}
        <ArrowRight className="size-2.5 shrink-0 text-ink-muted/30" />
      </div>
    )
  }

  // ── Generic notice ──────────────────────────────────────
  return (
    <div className={`border-hairline overflow-hidden rounded-lg border ${cardStyle(kind, status, isError)}`}>
      <div className="flex items-center gap-2 px-3 py-1.5">
        <KindIcon kind={kind} status={status} />
        <span className={`text-ink-muted/70 font-mono text-[11px] ${isError || status === "error" ? "text-red-500" : ""}`}>
          {detail || "通知"}
        </span>
      </div>
    </div>
  )
}

// ── Helpers ──────────────────────────────────────────────

function formatDelay(ms: number): string {
  if (ms < 1000) return `${ms}ms`
  if (ms < 60000) return `${Math.round(ms / 1000)}s`
  return `${Math.floor(ms / 60000)}m ${Math.round((ms % 60000) / 1000)}s`
}
