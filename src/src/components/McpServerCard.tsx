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
    <div className="bg-surface border-hairline flex items-center gap-3 rounded-lg border p-3 transition-colors hover:border-white/10">
      <span className={`size-1.5 shrink-0 rounded-full ${s.enabled ? "bg-up" : "bg-ink-muted"}`} />
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="text-body text-primary">{s.name}</span>
          <span className="text-secondary rounded border border-white/10 px-2 py-0.5 text-caption">
            {s.transport.type}
          </span>
        </div>
        <p className="text-secondary truncate font-mono text-caption">{transportSummary()}</p>
      </div>
      <div className="flex shrink-0 gap-1">
        <button
          className="hover:bg-hover text-secondary rounded px-2 py-1 text-caption transition-colors hover:text-primary"
          onClick={onEdit}
        >
          编辑
        </button>
        <button
          className="hover:bg-hover text-secondary rounded px-2 py-1 text-caption transition-colors hover:text-primary disabled:opacity-40"
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
          className="hover:bg-hover hover:text-down text-secondary rounded px-2 py-1 text-caption transition-colors disabled:opacity-40"
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
