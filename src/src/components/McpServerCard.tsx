import { useEffect, useRef, useState } from "react"
import { Button } from "@/components/ui/button"
import { isReadOnly, type McpServerConfig } from "../types/mcp"
import { mcpErrorMessage } from "@/hooks/query/useMcpServers"

export function McpServerCard({
  server: s,
  onToggle,
  onDelete,
  onEdit,
}: {
  server: McpServerConfig
  onToggle: () => Promise<void>
  onDelete: () => Promise<void>
  onEdit: () => void
}) {
  const [busy, setBusy] = useState<"toggle" | "delete" | null>(null)
  const [confirming, setConfirming] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const timer = useRef<number | null>(null)

  useEffect(
    () => () => {
      if (timer.current !== null) window.clearTimeout(timer.current)
    },
    []
  )

  // 统一延时槽：错误提示 / 删除确认 3s 后自动复位
  const later = (fn: () => void, ms = 3000) => {
    if (timer.current !== null) window.clearTimeout(timer.current)
    timer.current = window.setTimeout(() => {
      timer.current = null
      fn()
    }, ms)
  }

  const run = async (kind: "toggle" | "delete", fn: () => Promise<void>) => {
    setBusy(kind)
    setError(null)
    try {
      await fn()
    } catch (e) {
      setError(mcpErrorMessage(e instanceof Error ? e.message : String(e)))
      later(() => setError(null))
    } finally {
      setBusy(null)
      setConfirming(false)
    }
  }

  const transportSummary = () => {
    if (s.transport.type === "stdio") {
      return `${s.transport.command} ${s.transport.args.join(" ")}`.trim()
    }
    return s.transport.url
  }

  // 项目级条目只读：项目级按名覆盖全局，写全局不会生效（后端也会拒绝）。
  const readOnly = isReadOnly(s)

  return (
    <div className="flex min-h-[52px] items-center gap-3 py-3">
      <span className={`size-1.5 shrink-0 rounded-full ${s.enabled ? "bg-up" : "bg-ink-muted"}`} />
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="text-primary text-sm font-medium">{s.name}</span>
          <span className="text-secondary border-hairline text-caption rounded-md border px-1.5 py-0.5">
            {s.transport.type}
          </span>
          {readOnly && (
            <span
              className="text-secondary border-hairline text-caption rounded-md border px-1.5 py-0.5"
              title={`定义在 ${s.source}，请直接编辑该文件`}
            >
              项目
            </span>
          )}
        </div>
        <p className="text-secondary font-number text-caption mt-0.5 truncate">
          {transportSummary()}
        </p>
      </div>
      {error && (
        <span className="text-down text-caption max-w-[180px] shrink-0 truncate">{error}</span>
      )}
      <div className="flex shrink-0 gap-1">
        <Button
          type="button"
          variant="ghost"
          size="xs"
          className="text-secondary hover:text-primary"
          disabled={readOnly}
          title={readOnly ? `定义在 ${s.source}，请直接编辑该文件` : undefined}
          onClick={onEdit}
        >
          编辑
        </Button>
        <Button
          type="button"
          variant="ghost"
          size="xs"
          className="text-secondary hover:text-primary"
          disabled={busy !== null || readOnly}
          onClick={() => run("toggle", onToggle)}
        >
          {busy === "toggle" ? "切换中…" : s.enabled ? "停用" : "启用"}
        </Button>
        <Button
          type="button"
          variant="ghost"
          size="xs"
          className={`${confirming ? "bg-danger-bg text-down hover:bg-danger-bg hover:text-down" : "text-secondary hover:text-down"}`}
          disabled={busy !== null || readOnly}
          onClick={() => {
            if (confirming) {
              run("delete", onDelete)
            } else {
              setConfirming(true)
              later(() => setConfirming(false))
            }
          }}
        >
          {busy === "delete" ? "删除中…" : confirming ? "确认删除?" : "删除"}
        </Button>
      </div>
    </div>
  )
}
