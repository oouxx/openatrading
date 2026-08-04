import { useState } from "react"
import { Input } from "@/components/ui/input"
import { Button } from "@/components/ui/button"
import { KeyValueList } from "./KeyValueList"
import type { McpServerConfig, McpTransportConfig } from "../types/mcp"

const TRANSPORT_OPTIONS = [
  { value: "streamable-http" as const, label: "HTTP (Streamable)" },
  { value: "stdio" as const, label: "STDIO" },
  { value: "sse" as const, label: "SSE" },
]

export function McpServerForm({
  server,
  onChange,
  onCancel,
  onSave,
}: {
  server: McpServerConfig
  onChange: (s: McpServerConfig) => void
  onCancel: () => void
  onSave: () => Promise<void>
}) {
  const [saving, setSaving] = useState(false)

  const updateTransport = (patch: Partial<McpTransportConfig>) => {
    onChange({ ...server, transport: { ...server.transport, ...patch } })
  }

  const setArgs = (args: string[]) => {
    updateTransport({ args })
  }

  const setEnv = (env: Record<string, string>) => {
    updateTransport({ env })
  }

  const isStdio = server.transport.type === "stdio"

  return (
    <div className="bg-bg-surface border-hairline mb-4 space-y-4 rounded-lg border p-4">
      {/* 名称 */}
      <div>
        <label className="text-ink-muted mb-1 block text-xs">名称</label>
        <Input
          value={server.name}
          onChange={(e) => onChange({ ...server, name: e.target.value })}
          placeholder="例如: 文件系统"
          className="text-xs"
        />
      </div>

      {/* 通信方式 */}
      <div>
        <label className="text-ink-muted mb-1 block text-xs">通信方式</label>
        <div className="border-hairline flex gap-0 overflow-hidden rounded-md border">
          {TRANSPORT_OPTIONS.map((opt) => (
            <button
              key={opt.value}
              type="button"
              onClick={() =>
                onChange({
                  ...server,
                  transport:
                    opt.value === "streamable-http"
                      ? { type: "streamable-http", url: "", auth_token: undefined }
                      : opt.value === "sse"
                        ? { type: "sse", url: "", post_url: undefined }
                        : { type: "stdio", command: "", args: [], env: {} },
                })
              }
              className={`flex-1 px-3 py-1.5 text-xs transition-colors ${
                server.transport.type === opt.value
                  ? "bg-ai text-white"
                  : "text-ink-muted hover:bg-bg-hover bg-transparent hover:text-white"
              }`}
            >
              {opt.label}
            </button>
          ))}
        </div>
      </div>

      {/* URL / 命令 */}
      {isStdio ? (
        <div>
          <label className="text-ink-muted mb-1 block text-xs">命令</label>
          <Input
            value={server.transport.command ?? ""}
            onChange={(e) => updateTransport({ command: e.target.value })}
            placeholder="npx"
            className="font-mono text-xs"
          />
        </div>
      ) : (
        <div>
          <label className="text-ink-muted mb-1 block text-xs">
            URL {server.transport.type === "sse" && "(SSE Endpoint)"}
          </label>
          <Input
            value={server.transport.url ?? ""}
            onChange={(e) => updateTransport({ url: e.target.value })}
            placeholder={
              server.transport.type === "streamable-http"
                ? "http://localhost:8081/mcp"
                : "http://localhost:8081/sse"
            }
            className="font-mono text-xs"
          />
          {server.transport.type === "sse" && (
            <div className="mt-2">
              <label className="text-ink-muted mb-1 block text-xs">POST URL (可选)</label>
              <Input
                value={server.transport.post_url ?? ""}
                onChange={(e) => updateTransport({ post_url: e.target.value || undefined })}
                placeholder="留空则自动推断"
                className="font-mono text-xs"
              />
            </div>
          )}
        </div>
      )}

      {/* Auth Token (仅 HTTP) */}
      {!isStdio && (
        <div>
          <label className="text-ink-muted mb-1 block text-xs">Auth Token (可选)</label>
          <Input
            type="password"
            value={server.transport.auth_token ?? ""}
            onChange={(e) => updateTransport({ auth_token: e.target.value || undefined })}
            placeholder="Bearer token..."
            className="font-mono text-xs"
          />
        </div>
      )}

      {/* 参数列表 (仅 STDIO) */}
      {isStdio && (
        <div>
          <label className="text-ink-muted mb-1 block text-xs">参数</label>
          <KeyValueList
            items={(server.transport.args ?? []).map((v, i) => `arg_${i}=${v}`)}
            onChange={(entries) =>
              setArgs(
                entries.map((e) => {
                  const idx = e.indexOf("=")
                  return idx === -1 ? e : e.slice(idx + 1)
                })
              )
            }
            keyPlaceholder="序号"
            valuePlaceholder=""
            hideKey
          />
        </div>
      )}

      {/* 环境变量 (仅 STDIO) */}
      {isStdio && (
        <div>
          <label className="text-ink-muted mb-1 block text-xs">环境变量</label>
          <KeyValueList
            items={Object.entries(server.transport.env ?? {}).map(([k, v]) => `${k}=${v}`)}
            onChange={(entries) => {
              const env: Record<string, string> = {}
              for (const e of entries) {
                const idx = e.indexOf("=")
                if (idx > 0) env[e.slice(0, idx)] = e.slice(idx + 1)
              }
              setEnv(env)
            }}
            keyPlaceholder="KEY"
            valuePlaceholder="VALUE"
          />
        </div>
      )}

      {/* 操作按钮 */}
      <div className="flex justify-end gap-2 pt-1">
        <Button variant="outline" size="sm" onClick={onCancel}>
          取消
        </Button>
        <Button
          size="sm"
          disabled={saving || !server.name}
          onClick={async () => {
            setSaving(true)
            try {
              await onSave()
            } finally {
              setSaving(false)
            }
          }}
        >
          {saving ? "保存中…" : "保存"}
        </Button>
      </div>
    </div>
  )
}

export type { McpServerConfig }
