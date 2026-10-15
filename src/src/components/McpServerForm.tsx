import { useState } from "react"
import { Input } from "@/components/ui/input"
import { Button } from "@/components/ui/button"
import { KeyValueList } from "./KeyValueList"
import { mcpErrorMessage } from "@/hooks/query/useMcpServers"
import type { McpServerPayload, McpTransportConfig, McpTransportType } from "../types/mcp"

const TRANSPORT_OPTIONS: { value: McpTransportType; label: string }[] = [
  { value: "http", label: "HTTP (Streamable)" },
  { value: "stdio", label: "STDIO" },
  { value: "sse", label: "SSE" },
]

/** 切换传输类型时重置为该类型的空配置（判别联合，整体替换） */
function emptyTransport(type: McpTransportType): McpTransportConfig {
  switch (type) {
    case "stdio":
      return { type: "stdio", command: "", args: [], env: {} }
    case "sse":
      return { type: "sse", url: "", headers: {} }
    default:
      return { type: "http", url: "", headers: {} }
  }
}

/** Map<string, string> → KeyValueList 的 `key=value` 行 */
function toEntries(map: Record<string, string>): string[] {
  return Object.entries(map).map(([k, v]) => `${k}=${v}`)
}

function fromEntries(entries: string[]): Record<string, string> {
  const out: Record<string, string> = {}
  for (const e of entries) {
    const idx = e.indexOf("=")
    if (idx > 0) out[e.slice(0, idx)] = e.slice(idx + 1)
  }
  return out
}

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
  server: McpServerPayload
  onChange: (s: McpServerPayload) => void
  onCancel: () => void
  onSave: () => Promise<void>
  /** 已存在（除当前编辑项外）的服务器名称，用于重名校验 */
  existingNames: string[]
}) {
  const [saving, setSaving] = useState(false)
  const [attempted, setAttempted] = useState(false)
  const [saveError, setSaveError] = useState<string | null>(null)

  const transport = server.transport
  const isStdio = transport.type === "stdio"
  const isSse = transport.type === "sse"

  const updateTransport = (next: McpTransportConfig) => {
    onChange({ ...server, transport: next })
  }

  // ── 校验 ──
  const trimmedName = server.name.trim()
  const duplicateName = existingNames.includes(trimmedName)
  const missingEndpoint = isStdio
    ? transport.command.trim() === ""
    : transport.url.trim() === ""
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
          placeholder="例如: akshare"
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
              onClick={() => updateTransport(emptyTransport(opt.value))}
              className={`text-caption flex-1 px-3 py-2 transition-colors ${
                transport.type === opt.value
                  ? "bg-brand text-primary"
                  : "text-secondary hover:bg-hover hover:text-primary bg-transparent"
              }`}
            >
              {opt.label}
            </button>
          ))}
        </div>
      </div>

      {/* 命令 / URL */}
      {transport.type === "stdio" ? (
        <>
          <div>
            <label className="text-secondary text-caption mb-1 block">命令</label>
            <Input
              value={transport.command}
              onChange={(e) => updateTransport({ ...transport, command: e.target.value })}
              placeholder="npx"
              aria-invalid={endpointError ? true : undefined}
              className="font-number text-caption"
            />
            {endpointError && <FieldError>{endpointError}</FieldError>}
          </div>
          <div>
            <label className="text-secondary text-caption mb-1 block">参数</label>
            <KeyValueList
              items={transport.args.map((v, i) => `arg_${i}=${v}`)}
              onChange={(entries) =>
                updateTransport({
                  ...transport,
                  args: entries.map((e) => {
                    const idx = e.indexOf("=")
                    return idx === -1 ? e : e.slice(idx + 1)
                  }),
                })
              }
              keyPlaceholder="序号"
              valuePlaceholder=""
              hideKey
            />
          </div>
          <div>
            <label className="text-secondary text-caption mb-1 block">环境变量</label>
            <KeyValueList
              items={toEntries(transport.env)}
              onChange={(entries) => updateTransport({ ...transport, env: fromEntries(entries) })}
              keyPlaceholder="KEY"
              valuePlaceholder="VALUE"
            />
          </div>
          <div>
            <label className="text-secondary text-caption mb-1 block">工作目录 (可选)</label>
            <Input
              value={transport.cwd ?? ""}
              onChange={(e) => updateTransport({ ...transport, cwd: e.target.value || undefined })}
              placeholder="留空则用会话工作目录"
              className="font-number text-caption"
            />
          </div>
        </>
      ) : (
        <>
          <div>
            <label className="text-secondary text-caption mb-1 block">
              URL {isSse && "(SSE Endpoint)"}
            </label>
            <Input
              value={transport.url}
              onChange={(e) => updateTransport({ ...transport, url: e.target.value })}
              placeholder={isSse ? "http://localhost:8081/sse" : "http://localhost:8081/mcp"}
              aria-invalid={endpointError ? true : undefined}
              className="font-number text-caption"
            />
            {endpointError && <FieldError>{endpointError}</FieldError>}
          </div>
          <div>
            <label className="text-secondary text-caption mb-1 block">Headers (可选)</label>
            <KeyValueList
              items={toEntries(transport.headers)}
              onChange={(entries) =>
                updateTransport({ ...transport, headers: fromEntries(entries) })
              }
              keyPlaceholder="Authorization"
              valuePlaceholder="Bearer ..."
            />
            <p className="text-secondary text-caption mt-1">
              鉴权放在 Headers 里，值支持 <code>{"${VAR}"}</code> 环境变量引用
            </p>
          </div>
        </>
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

export type { McpServerPayload }
