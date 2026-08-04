import { useState } from "react"
import type { McpServerConfig } from "../types/mcp"

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
  const [toggling, setToggling] = useState(false)
  const [deleting, setDeleting] = useState(false)

  const transportSummary = () => {
    if (s.transport.type === "stdio") {
      return `${s.transport.command ?? ""} ${(s.transport.args ?? []).join(" ")}`.trim()
    }
    return s.transport.url ?? ""
  }

  return (
    <div className="bg-bg-surface border-hairline flex items-center gap-3 rounded-lg border p-3 transition-colors hover:border-white/10">
      <span className={`size-1.5 shrink-0 rounded-full ${s.enabled ? "bg-up" : "bg-ink-muted"}`} />
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="text-sm text-white">{s.name}</span>
          <span className="text-ink-muted rounded border border-white/10 px-1.5 py-0.5 text-[10px]">
            {s.transport.type}
          </span>
        </div>
        <p className="text-ink-muted truncate font-mono text-[11px]">{transportSummary()}</p>
      </div>
      <div className="flex shrink-0 gap-1">
        <button
          className="hover:bg-bg-hover text-ink-muted rounded px-2 py-1 text-[10px] transition-colors hover:text-white"
          onClick={onEdit}
        >
          编辑
        </button>
        <button
          className="hover:bg-bg-hover text-ink-muted rounded px-2 py-1 text-[10px] transition-colors hover:text-white disabled:opacity-40"
          disabled={toggling}
          onClick={async () => {
            setToggling(true)
            try {
              await onToggle()
            } finally {
              setToggling(false)
            }
          }}
        >
          {s.enabled ? "停用" : "启用"}
        </button>
        <button
          className="hover:bg-bg-hover hover:text-down text-ink-muted rounded px-2 py-1 text-[10px] transition-colors disabled:opacity-40"
          disabled={deleting}
          onClick={async () => {
            setDeleting(true)
            try {
              await onDelete()
            } finally {
              setDeleting(false)
            }
          }}
        >
          删除
        </button>
      </div>
    </div>
  )
}
