import { useState } from "react"
import { Globe, Pencil, Radio, Terminal, Trash2 } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { Switch } from "@/components/ui/switch"
import { mcpErrorMessage } from "@/hooks/query/useMcpServers"
import {
  isReadOnly,
  mcpTransportLabel,
  mcpTransportTarget,
  type McpServerConfig,
} from "../types/mcp"

const TRANSPORT_ICON = {
  stdio: Terminal,
  http: Globe,
  sse: Radio,
} as const

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
  const [error, setError] = useState<string | null>(null)
  const [confirmOpen, setConfirmOpen] = useState(false)

  const readOnly = isReadOnly(s)
  const Icon = TRANSPORT_ICON[s.transport.type]
  const target = mcpTransportTarget(s.transport)
  const transportLabel = mcpTransportLabel(s.transport)

  const run = async (kind: "toggle" | "delete", fn: () => Promise<void>) => {
    setBusy(kind)
    setError(null)
    try {
      await fn()
      if (kind === "delete") setConfirmOpen(false)
    } catch (e) {
      setError(mcpErrorMessage(e instanceof Error ? e.message : String(e)))
    } finally {
      setBusy(null)
    }
  }

  const readOnlyTip = `定义在 ${s.source}，请直接编辑该文件`

  return (
    <div className="bg-surface ring-foreground/5 flex items-start gap-3 rounded-xl p-3.5 ring-1">
      <span className="bg-hover text-secondary grid size-9 shrink-0 place-items-center rounded-lg">
        <Icon className="size-4" />
      </span>

      <div className="min-w-0 flex-1">
        <div className="flex flex-wrap items-center gap-1.5">
          <span className="text-primary max-w-full truncate text-sm font-medium">{s.name}</span>
          <span className="text-tertiary border-hairline font-number text-caption rounded-md border px-1.5 py-0.5 leading-none">
            {transportLabel}
          </span>
          {readOnly && (
            <span
              className="text-tertiary border-hairline text-caption rounded-md border px-1.5 py-0.5 leading-none"
              title={readOnlyTip}
            >
              项目
            </span>
          )}
          {!readOnly && (
            <span
              className="text-tertiary border-hairline text-caption rounded-md border px-1.5 py-0.5 leading-none"
              title={s.source}
            >
              全局
            </span>
          )}
        </div>
        <p
          className="text-tertiary font-number text-caption mt-1 truncate"
          title={target || "未配置命令或地址"}
        >
          {target || "未配置命令或地址"}
        </p>
        {error && !confirmOpen && <p className="text-down text-caption mt-1">{error}</p>}
      </div>

      <div className="flex shrink-0 items-center gap-0.5">
        <Switch
          checked={s.enabled}
          disabled={readOnly || busy !== null}
          onCheckedChange={() => {
            void run("toggle", onToggle)
          }}
          aria-label={`${s.enabled ? "停用" : "启用"} ${s.name}`}
          title={readOnly ? readOnlyTip : s.enabled ? "停用" : "启用"}
          className="mr-1"
        />
        <Button
          type="button"
          variant="ghost"
          size="icon-sm"
          className="text-secondary hover:text-primary"
          disabled={readOnly}
          title={readOnly ? readOnlyTip : "编辑"}
          aria-label={`编辑 ${s.name}`}
          onClick={onEdit}
        >
          <Pencil className="size-3.5" />
        </Button>

        <Popover
          open={confirmOpen}
          onOpenChange={(open) => {
            setConfirmOpen(open)
            if (open) setError(null)
          }}
        >
          <PopoverTrigger asChild>
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              className="text-secondary hover:text-down"
              disabled={readOnly || busy !== null}
              title={readOnly ? readOnlyTip : "删除"}
              aria-label={`删除 ${s.name}`}
            >
              <Trash2 className="size-3.5" />
            </Button>
          </PopoverTrigger>
          <PopoverContent align="end" className="w-64 p-3">
            <p className="text-primary text-sm font-medium">删除「{s.name}」？</p>
            <p className="text-secondary text-caption mt-1">
              此操作不可撤销，之后新建的会话将无法再使用该服务器。
            </p>
            {error && <p className="text-down text-caption mt-2">{error}</p>}
            <div className="mt-3 flex justify-end gap-2">
              <Button type="button" variant="ghost" size="xs" onClick={() => setConfirmOpen(false)}>
                取消
              </Button>
              <Button
                type="button"
                variant="destructive"
                size="xs"
                disabled={busy === "delete"}
                onClick={() => {
                  void run("delete", onDelete)
                }}
              >
                {busy === "delete" ? "删除中…" : "删除"}
              </Button>
            </div>
          </PopoverContent>
        </Popover>
      </div>
    </div>
  )
}
