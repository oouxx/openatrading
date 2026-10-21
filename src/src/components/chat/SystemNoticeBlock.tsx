import {
  AlertTriangle,
  CheckCircle2,
  Shrink,
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

function KindIcon({
  kind,
  status,
}: {
  kind: SystemNotice["kind"]
  status?: SystemNotice["status"]
}) {
  switch (kind) {
    case "compaction":
      if (status === "active") return <Shrink className="text-info size-3.5 animate-pulse" />
      if (status === "error") return <XCircle className="text-danger size-3.5" />
      return <CheckCircle2 className="text-info size-3.5" />
    case "retry":
      if (status === "active") return <RefreshCw className="text-warning size-3.5 animate-spin" />
      if (status === "error") return <XCircle className="text-danger size-3.5" />
      return <CheckCircle2 className="text-success size-3.5" />
    case "turn":
      return <ArrowRight className="text-tertiary size-3" />
    case "notice":
      if (status === "error") return <AlertTriangle className="text-danger size-3.5" />
      return <CheckCircle2 className="text-success size-3.5" />
  }
}

// ── Tailwind classes per kind ────────────────────────────

function cardStyle(
  kind: SystemNotice["kind"],
  status?: SystemNotice["status"],
  isError?: boolean
): string {
  if (isError || status === "error") return "border-danger/25 bg-danger/10"
  switch (kind) {
    case "compaction":
      if (status === "active") return "border-info/25 bg-info/10"
      return "border-info/15 bg-info/5"
    case "retry":
      if (status === "active") return "border-warning/25 bg-warning/10"
      if (status === "done") return "border-success/15 bg-success/5"
      return "border-warning/20 bg-warning/5"
    case "turn":
      return "border-transparent"
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
      <div
        className={`border-hairline overflow-hidden rounded-lg border ${cardStyle(kind, status)} transition-colors duration-500`}
      >
        <div className="flex items-center gap-2 px-3 py-2">
          <KindIcon kind={kind} status={status} />
          <span
            className={`font-number text-caption font-medium ${isActive ? "text-info" : failed ? "text-danger" : "text-tertiary"}`}
          >
            会话压缩
          </span>
          {isActive && (
            <span className="ml-auto flex items-center gap-2">
              <span className="bg-info/30 size-1.5 animate-pulse rounded-full" />
              <span className="text-tertiary font-number text-caption">压缩中…</span>
            </span>
          )}
          {failed && (
            <span className="font-number text-caption text-danger/70 ml-auto">压缩失败</span>
          )}
          {status === "done" && (
            <span className="font-number text-caption text-info/50 ml-auto">已完成</span>
          )}
        </div>
        {detail && !isActive && (
          <div className="border-hairline border-t px-3 py-2">
            <p className="text-tertiary font-number text-caption leading-relaxed">{detail}</p>
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
      <div
        className={`border-hairline overflow-hidden rounded-lg border ${cardStyle(kind, status)} transition-colors duration-500`}
      >
        <div className="flex items-center gap-2 px-3 py-2">
          <KindIcon kind={kind} status={status} />
          <span
            className={`font-number text-caption font-medium ${isActive ? "text-warning" : failed ? "text-danger" : "text-success"}`}
          >
            自动重试
          </span>
          {attempt != null && maxAttempts != null && (
            <span className="text-tertiary font-number text-caption">
              {attempt}/{maxAttempts}
            </span>
          )}
          {isActive && (
            <span className="ml-auto flex items-center gap-2">
              <Loader2 className="text-warning size-3 animate-spin" />
              <span className="text-tertiary font-number text-caption">重试中…</span>
            </span>
          )}
          {status === "done" && (
            <span className="font-number text-caption text-success/70 ml-auto">重试成功</span>
          )}
          {failed && (
            <span className="font-number text-caption text-danger/70 ml-auto">已达上限</span>
          )}
        </div>
        {(detail || delayMs != null) && (
          <div className="border-hairline border-t px-3 py-2">
            {detail && (
              <p className="text-tertiary font-number text-caption leading-relaxed">{detail}</p>
            )}
            {delayMs != null && (
              <p className="text-tertiary font-number text-caption mt-0.5">
                等待 {formatDelay(delayMs)}
              </p>
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
        <div className="border-hairline flex-1 border-t" />
        {detail && (
          <span className="text-tertiary font-number text-caption shrink-0">{detail}</span>
        )}
        <ArrowRight className="text-tertiary size-2.5 shrink-0" />
      </div>
    )
  }

  // ── Generic notice ──────────────────────────────────────
  return (
    <div
      className={`border-hairline overflow-hidden rounded-lg border ${cardStyle(kind, status, isError)}`}
    >
      <div className="flex items-center gap-2 px-3 py-2">
        <KindIcon kind={kind} status={status} />
        <span
          className={`text-tertiary font-number text-caption ${isError || status === "error" ? "text-danger" : ""}`}
        >
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
