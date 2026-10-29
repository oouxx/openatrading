import { AlertCircle, Inbox, LoaderCircle } from "lucide-react"

import { Button } from "@/components/ui/button"
import { Skeleton } from "@/components/ui/skeleton"
import { cn } from "@/lib/utils"

type DataStateSize = "panel" | "table" | "compact"

const SIZE_CLASS: Record<DataStateSize, string> = {
  panel: "min-h-52 px-6 py-10",
  table: "px-4 py-12",
  compact: "px-3 py-6",
}

const TITLE_CLASS: Record<DataStateSize, string> = {
  panel: "text-body",
  table: "text-caption",
  compact: "text-caption",
}

function Shell({
  children,
  size,
  role,
}: {
  children: React.ReactNode
  size: DataStateSize
  role?: "status" | "alert"
}) {
  return (
    <div
      role={role}
      className={cn(
        "text-secondary flex flex-col items-center justify-center gap-3 text-center",
        SIZE_CLASS[size]
      )}
    >
      {children}
    </div>
  )
}

export function DataLoading({
  label = "加载中…",
  size = "panel",
  rows = 4,
}: {
  label?: string
  size?: DataStateSize
  rows?: number
}) {
  if (size === "compact") {
    return (
      <div role="status" aria-live="polite" className="text-tertiary text-caption flex items-center justify-center gap-2 py-4">
        <LoaderCircle className="size-3.5 animate-spin" aria-hidden />
        {label}
      </div>
    )
  }

  return (
    <div
      role="status"
      aria-live="polite"
      className={cn(
        "space-y-2",
        SIZE_CLASS[size] === SIZE_CLASS.table ? "p-4" : "p-5"
      )}
    >
      <span className="sr-only">{label}</span>
      {Array.from({ length: rows }, (_, index) => (
        <Skeleton
          key={index}
          className={cn("w-full rounded-md", index === 0 ? "h-7" : "h-9")}
        />
      ))}
    </div>
  )
}

export function DataEmpty({
  title = "暂无数据",
  description,
  size = "panel",
  icon,
  action,
}: {
  title?: string
  description?: string
  size?: DataStateSize
  icon?: React.ReactNode
  action?: React.ReactNode
}) {
  return (
    <Shell size={size} role="status">
      <div className="bg-hover text-tertiary flex size-9 items-center justify-center rounded-full">
        {icon ?? <Inbox className="size-4" aria-hidden />}
      </div>
      <div className="space-y-1">
        <p className={cn("text-primary font-medium", TITLE_CLASS[size])}>{title}</p>
        {description && <p className="text-tertiary text-caption">{description}</p>}
      </div>
      {action}
    </Shell>
  )
}

export function DataError({
  title = "加载失败",
  description,
  size = "panel",
  onRetry,
  retryLabel = "重试",
}: {
  title?: string
  description?: string
  size?: DataStateSize
  onRetry?: () => void
  retryLabel?: string
}) {
  return (
    <Shell size={size} role="alert">
      <div className="bg-down/10 text-down flex size-9 items-center justify-center rounded-full">
        <AlertCircle className="size-4" aria-hidden />
      </div>
      <div className="space-y-1">
        <p className={cn("text-primary font-medium", TITLE_CLASS[size])}>{title}</p>
        {description && <p className="text-tertiary text-caption max-w-sm break-words">{description}</p>}
      </div>
      {onRetry && (
        <Button type="button" variant="outline" size="sm" onClick={onRetry}>
          {retryLabel}
        </Button>
      )}
    </Shell>
  )
}

export function DataStale({
  label = "上游延迟，展示最近数据",
  className,
}: {
  label?: string
  className?: string
}) {
  return (
    <span
      role="status"
      className={cn(
        "border-warning/25 bg-warning/8 text-warning text-micro inline-flex items-center gap-1.5 rounded-sm border px-2 py-0.5",
        className
      )}
    >
      <span className="bg-warning size-1.5 rounded-full" aria-hidden />
      {label}
    </span>
  )
}
