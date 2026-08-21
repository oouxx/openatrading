import { useState } from "react"
import { Input } from "@/components/ui/input"
import { Button } from "@/components/ui/button"
import { KeyValueList } from "./KeyValueList"
import { mcpErrorMessage } from "@/hooks/query/useMcpServers"
import type { McpServerConfig, McpTransportConfig } from "../types/mcp"

const TRANSPORT_OPTIONS = [
  { value: "streamable-http" as const, label: "HTTP (Streamable)" },
  { value: "stdio" as const, label: "STDIO" },
  { value: "sse" as const, label: "SSE" },
]

function FieldError({ children }: { children: string }) {
  return <p className="text-down text-caption mt-1">{children}</p>
}

export function McpServerForm({
  server,
  onChange,
  onCancel,
  onSave,
  existingNames,
}: {
  server: McpServerConfig
  onChange: (s: McpServerConfig) => void
  onCancel: () => void
  onSave: () => Promise<void>
  /** 已存在（除当前编辑项外）的服务器名称，用于重名校验 */
  existingNames: string[]
}) {
  const [saving, setSaving] = useState(false)
  const [attempted, setAttempted] = useState(false)
  const [saveError, setSaveError] = useState<string | null>(null)

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

  // ── 校验 ──
  const trimmedName = server.name.trim()
  const duplicateName = existingNames.includes(trimmedName)
  const missingEndpoint = isStdio
    ? !(server.transport.command ?? "").trim()
    : !(server.transport.url ?? "").trim()
  const valid = trimmedName !== "" && !duplicateName && !missingEndpoint

  const nameError = attempted
    ? trimmedName === ""
      ? "请输入服务器名称"
      : duplicateName
        ? "已存在同名服务器"
        : null
    : null
  const endpointError =
    attempted && missingEndpoint ? (isStdio ? "请输入命令" : "请输入 URL") : null

  const handleSave = async () => {
    setAttempted(true)
    setSaveError(null)
    if (!valid) return
    setSaving(true)
    try {
      await onSave()
    } catch (e) {
      setSaveError(mcpErrorMessage(e instanceof Error ? e.message : String(e)))
    } finally {
      setSaving(false)
    }
  }

  return (
    <div className="bg-surface border-hairline space-y-4 rounded-lg border p-4">
      {/* 名称 */}
      <div>
        <label className="text-secondary text-caption mb-1 block">名称</label>
        <Input
          value={server.name}
          onChange={(e) => onChange({ ...server, name: e.target.value })}
          placeholder="例如: 文件系统"
          aria-invalid={nameError ? true : undefined}
          className="text-caption"
        />
        {nameError && <FieldError>{nameError}</FieldError>}
      </div>

      {/* 通信方式 */}
      <div>
        <label className="text-secondary text-caption mb-1 block">通信方式</label>
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
              className={`text-caption flex-1 px-3 py-2 transition-colors ${
                server.transport.type === opt.value
                  ? "bg-brand text-primary"
                  : "text-secondary hover:bg-hover hover:text-primary bg-transparent"
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
          <label className="text-secondary text-caption mb-1 block">命令</label>
          <Input
            value={server.transport.command ?? ""}
            onChange={(e) => updateTransport({ command: e.target.value })}
            placeholder="npx"
            aria-invalid={endpointError ? true : undefined}
            className="font-number text-caption"
          />
          {endpointError && <FieldError>{endpointError}</FieldError>}
        </div>
      ) : (
        <div>
          <label className="text-secondary text-caption mb-1 block">
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
            aria-invalid={endpointError ? true : undefined}
            className="font-number text-caption"
          />
          {endpointError && <FieldError>{endpointError}</FieldError>}
          {server.transport.type === "sse" && (
            <div className="mt-2">
              <label className="text-secondary text-caption mb-1 block">POST URL (可选)</label>
              <Input
                value={server.transport.post_url ?? ""}
                onChange={(e) => updateTransport({ post_url: e.target.value || undefined })}
                placeholder="留空则自动推断"
                className="font-number text-caption"
              />
            </div>
          )}
        </div>
      )}

      {/* Auth Token (仅 HTTP) */}
      {!isStdio && (
        <div>
          <label className="text-secondary text-caption mb-1 block">Auth Token (可选)</label>
          <Input
            type="password"
            value={server.transport.auth_token ?? ""}
            onChange={(e) => updateTransport({ auth_token: e.target.value || undefined })}
            placeholder="Bearer token..."
            className="font-number text-caption"
          />
        </div>
      )}

      {/* 参数列表 (仅 STDIO) */}
      {isStdio && (
        <div>
          <label className="text-secondary text-caption mb-1 block">参数</label>
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
          <label className="text-secondary text-caption mb-1 block">环境变量</label>
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
      <div className="flex items-center justify-end gap-2 pt-1">
        {saveError && <span className="text-down text-caption mr-auto">{saveError}</span>}
        <Button variant="outline" size="sm" onClick={onCancel}>
          取消
        </Button>
        <Button size="sm" disabled={saving} onClick={handleSave}>
          {saving ? "保存中…" : "保存"}
        </Button>
      </div>
    </div>
  )
}

export type { McpServerConfig }
