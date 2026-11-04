import { useState } from "react"
import { Input } from "@/components/ui/input"
import { Button } from "@/components/ui/button"
import { KeyValueList } from "./KeyValueList"
import { mcpErrorMessage } from "@/hooks/query/useMcpServers"
import {
  emptyTransport,
  type McpExposure,
  type McpServerPayload,
  type McpTransportConfig,
  type McpTransportType,
} from "../types/mcp"

const TRANSPORT_OPTIONS: { value: McpTransportType; label: string }[] = [
  { value: "http", label: "HTTP (Streamable)" },
  { value: "stdio", label: "STDIO" },
  { value: "sse", label: "SSE" },
]

const EXPOSURE_OPTIONS: { value: Exclude<McpExposure, "codemode">; label: string; hint: string }[] =
  [
    { value: "direct", label: "直接可用", hint: "工具直接出现在模型工具列表里" },
    { value: "deferred", label: "按需搜索", hint: "由 tool_search 按需加载" },
    { value: "hidden", label: "隐藏", hint: "注册但不可调用" },
  ]

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
  // pi-rs 的 server 命名规则：仅字母、数字、下划线、连字符。
  const invalidNameChars = trimmedName !== "" && !/^[A-Za-z0-9_-]+$/.test(trimmedName)
  const duplicateName = existingNames.includes(trimmedName)
  const missingEndpoint = isStdio ? transport.command.trim() === "" : transport.url.trim() === ""
  const valid = trimmedName !== "" && !invalidNameChars && !duplicateName && !missingEndpoint

  const nameError = attempted
    ? trimmedName === ""
      ? "请输入服务器名称"
      : invalidNameChars
        ? "仅支持字母、数字、下划线和连字符"
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
    <div className="space-y-4">
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
            <Button
              key={opt.value}
              type="button"
              variant="ghost"
              onClick={() => updateTransport(emptyTransport(opt.value))}
              className={`text-caption h-auto flex-1 rounded-none px-3 py-2 ${
                transport.type === opt.value
                  ? "bg-brand text-brand-foreground hover:bg-brand hover:text-brand-foreground"
                  : "text-secondary hover:bg-hover hover:text-primary"
              }`}
            >
              {opt.label}
            </Button>
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

      {/* 工具暴露方式 */}
      <div>
        <label className="text-secondary text-caption mb-1 block">工具暴露</label>
        <div className="border-hairline flex gap-0 overflow-hidden rounded-md border">
          {EXPOSURE_OPTIONS.map((opt) => {
            // codemode 与 deferred 在本应用里等价，统一显示为「按需搜索」
            const current =
              server.exposure === "codemode" ? "deferred" : (server.exposure ?? "direct")
            const active = current === opt.value
            return (
              <Button
                key={opt.value}
                type="button"
                variant="ghost"
                onClick={() => onChange({ ...server, exposure: opt.value })}
                title={opt.hint}
                className={`text-caption h-auto flex-1 rounded-none px-3 py-2 ${
                  active
                    ? "bg-brand text-brand-foreground hover:bg-brand hover:text-brand-foreground"
                    : "text-secondary hover:bg-hover hover:text-primary"
                }`}
              >
                {opt.label}
              </Button>
            )
          })}
        </div>
      </div>

      {/* 操作按钮 */}
      <div className="border-hairline flex items-center justify-end gap-2 border-t pt-4">
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
