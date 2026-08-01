import { useState, useEffect } from "react"
import { Input } from "@/components/ui/input"
import { Button } from "@/components/ui/button"
import { useUiStore } from "@/stores/ui"
import { useSettingsStore } from "@/stores/settingsStore"
import { fetchJson } from "@/stores/data"
import type { SettingsTab } from "@/components/SettingsSidebar"

import { isTauri } from "@/lib/env"
async function setConfig(section: string, key: string, value: string) {
  if (isTauri) {
    const { invoke } = await import("@tauri-apps/api/core")
    await invoke("update_config", { section, key, value })
  }
  localStorage.setItem(`openatrading-${key}`, value)
}

interface McpServerConfig {
  name: string
  transport: McpTransportConfig
  enabled: boolean
}

interface McpTransportConfig {
  type: "streamable-http" | "stdio" | "sse"
  url?: string
  auth_token?: string
  post_url?: string
  command?: string
  args?: string[]
  env?: Record<string, string>
}

const THEME_LABELS: Record<string, string> = { dark: "暗色", light: "浅色" }

const TRANSPORT_OPTIONS = [
  { value: "streamable-http" as const, label: "HTTP (Streamable)" },
  { value: "stdio" as const, label: "STDIO" },
  { value: "sse" as const, label: "SSE" },
]

export default function SettingsView({ activeTab }: { activeTab: SettingsTab }) {
  const theme = useUiStore((s) => s.theme)
  const setTheme = useUiStore((s) => s.setTheme)

  const apiUrl = useSettingsStore((s) => s.apiUrl)
  const setApiUrl = useSettingsStore((s) => s.setApiUrl)
  const [restarting, setRestarting] = useState(false)
  // Social notification state
  const [social, setSocial] = useState({
    notify_channel: "telegram",
    notify_on_collect: true,
    notify_on_decision: true,
    notify_on_trade: true,
    notify_on_news: true,
    telegram_bot_token: "",
    telegram_chat_id: "",
    discord_bot_token: "",
    discord_channel_id: "",
  })
  const [servers, setServers] = useState<McpServerConfig[]>([])
  const [editingServer, setEditingServer] = useState<McpServerConfig | null>(null)
  const [editingOriginalName, setEditingOriginalName] = useState<string | null>(null)
  const [showNewForm, setShowNewForm] = useState(false)

  const newServerTemplate = (): McpServerConfig => ({
    name: "",
    enabled: true,
    transport: { type: "streamable-http", url: "" },
  })

  const loadServers = async () => {
    try {
      const data = await fetchJson<McpServerConfig[]>("/api/mcp/servers")
      setServers(data)
    } catch {
      // MCP API may not be available
    }
  }

  // Load on mount — initialization fetch outside React data flow

  useEffect(() => {
    /* eslint-disable react-hooks/set-state-in-effect */
    loadServers()
    if (isTauri) {
      ;(async () => {
        const { invoke } = await import("@tauri-apps/api/core")
        try {
          const cfg = (await invoke("get_config")) as any
          if (cfg?.social) {
            setSocial((s) => ({ ...s, ...cfg.social }))
          }
        } catch {
          /* ignore */
        }
      })()
    } else {
      // ponytail: load social settings from localStorage (written by setConfig)
      const keys = [
        "notify_channel",
        "notify_on_collect",
        "notify_on_decision",
        "notify_on_trade",
        "notify_on_news",
        "telegram_bot_token",
        "telegram_chat_id",
        "discord_bot_token",
        "discord_channel_id",
      ] as const
      const stored: Record<string, string | boolean> = {}
      for (const key of keys) {
        const val = localStorage.getItem(`openatrading-${key}`)
        if (val !== null) {
          stored[key] = key.startsWith("notify_on_") ? val === "true" : val
        }
      }
      if (Object.keys(stored).length > 0) setSocial((s) => ({ ...s, ...stored }))
    }
  }, [])

  return (
    <div className="flex h-full flex-1 flex-col gap-5 overflow-y-auto p-6">
      {activeTab === "appearance" && (
        <section>
          <h2 className="text-ink-muted mb-3 text-xs font-medium tracking-wider uppercase">外观</h2>
          <div className="bg-bg-surface border-hairline rounded-lg border p-4">
            <div className="flex items-center justify-between">
              <div>
                <span className="text-sm text-white">主题</span>
                <p className="text-ink-muted mt-0.5 text-[11px]">选择深色或浅色主题</p>
              </div>
              <div className="border-hairline flex gap-0 overflow-hidden rounded-md border">
                {(["dark", "light"] as const).map((t) => (
                  <button
                    key={t}
                    onClick={() => setTheme(t)}
                    className={`px-3 py-1.5 text-xs transition-colors ${
                      theme === t
                        ? "bg-ai text-white"
                        : "text-ink-muted hover:bg-bg-hover bg-transparent hover:text-white"
                    }`}
                  >
                    {THEME_LABELS[t]}
                  </button>
                ))}
              </div>
            </div>
          </div>
        </section>
      )}

      {activeTab === "agent" && (
        <>
          <section>
            <h2 className="text-ink-muted mb-3 text-xs font-medium tracking-wider uppercase">
              数据源
            </h2>
            <div className="bg-bg-surface border-hairline rounded-lg border p-4">
              <label className="text-ink-muted mb-1 block text-xs">API 地址</label>
              <Input
                value={apiUrl}
                onChange={(e) => {
                  setApiUrl(e.target.value)
                }}
                placeholder="http://localhost:8080"
                className="font-mono text-xs"
              />
            </div>
          </section>

          {isTauri && (
            <section>
              <h2 className="text-ink-muted mb-3 text-xs font-medium tracking-wider uppercase">
                操作
              </h2>
              <div className="bg-bg-surface border-hairline rounded-lg border p-4">
                <div className="flex items-center justify-between">
                  <div>
                    <span className="text-sm text-white">重启客户端</span>
                    <p className="text-ink-muted mt-0.5 text-[11px]">
                      修改 Provider/Model 后重启生效
                    </p>
                  </div>
                  <Button
                    size="sm"
                    disabled={restarting}
                    onClick={async () => {
                      setRestarting(true)
                      const { invoke } = await import("@tauri-apps/api/core")
                      await invoke("restart_app")
                    }}
                  >
                    {restarting ? "重启中…" : "重启"}
                  </Button>
                </div>
              </div>
            </section>
          )}
        </>
      )}

      {activeTab === "social" && (
        <div className="space-y-5">
          <section>
            <h2 className="text-ink-muted mb-3 text-xs font-medium tracking-wider uppercase">
              通知渠道
            </h2>
            <div className="bg-bg-surface border-hairline space-y-3 rounded-lg border p-4">
              <div>
                <label className="text-ink-muted mb-1 block text-xs">默认通知渠道</label>
                <select
                  value={social.notify_channel}
                  onChange={async (e) => {
                    const v = e.target.value
                    setSocial((s) => ({ ...s, notify_channel: v }))
                    if (isTauri) await setConfig("social", "notify_channel", v)
                  }}
                  className="bg-bg-tertiary border-hairline text-ink w-full rounded-lg border px-3 py-1.5 font-mono text-xs focus:outline-none"
                >
                  <option value="telegram">Telegram</option>
                  <option value="discord">Discord</option>
                  <option value="slack">Slack</option>
                  <option value="lark">飞书 (Lark)</option>
                </select>
              </div>
              <div className="flex flex-wrap gap-4">
                {(["collect", "decision", "trade", "news"] as const).map((k) => (
                  <label key={k} className="flex items-center gap-2 text-xs text-white">
                    <input
                      type="checkbox"
                      checked={social[`notify_on_${k}` as keyof typeof social] as boolean}
                      onChange={async (e) => {
                        const v = e.target.checked
                        setSocial((s) => ({ ...s, [`notify_on_${k}`]: v }))
                        if (isTauri) await setConfig("social", `notify_on_${k}`, String(v))
                      }}
                    />
                    {k === "collect"
                      ? "采集"
                      : k === "decision"
                        ? "决策"
                        : k === "trade"
                          ? "交易"
                          : "新闻"}
                  </label>
                ))}
              </div>
            </div>
          </section>

          <section>
            <h2 className="text-ink-muted mb-3 text-xs font-medium tracking-wider uppercase">
              Telegram
            </h2>
            <div className="bg-bg-surface border-hairline space-y-3 rounded-lg border p-4">
              <div>
                <label className="text-ink-muted mb-1 block text-xs">Bot Token</label>
                <Input
                  type="password"
                  value={social.telegram_bot_token}
                  onChange={async (e) => {
                    const v = e.target.value
                    setSocial((s) => ({ ...s, telegram_bot_token: v }))
                    if (isTauri) await setConfig("social.telegram", "bot_token", v)
                  }}
                  placeholder="123456:ABC-DEF1234gh..."
                  className="font-mono text-xs"
                />
              </div>
              <div>
                <label className="text-ink-muted mb-1 block text-xs">Chat ID</label>
                <Input
                  value={social.telegram_chat_id}
                  onChange={async (e) => {
                    const v = e.target.value
                    setSocial((s) => ({ ...s, telegram_chat_id: v }))
                    if (isTauri) await setConfig("social.telegram", "chat_id", v)
                  }}
                  placeholder="-1001234567890"
                  className="font-mono text-xs"
                />
              </div>
            </div>
          </section>

          <section>
            <h2 className="text-ink-muted mb-3 text-xs font-medium tracking-wider uppercase">
              Discord
            </h2>
            <div className="bg-bg-surface border-hairline space-y-3 rounded-lg border p-4">
              <div>
                <label className="text-ink-muted mb-1 block text-xs">Bot Token</label>
                <Input
                  type="password"
                  value={social.discord_bot_token}
                  onChange={async (e) => {
                    const v = e.target.value
                    setSocial((s) => ({ ...s, discord_bot_token: v }))
                    if (isTauri) await setConfig("social.discord", "bot_token", v)
                  }}
                  placeholder="MTE4..."
                  className="font-mono text-xs"
                />
              </div>
              <div>
                <label className="text-ink-muted mb-1 block text-xs">Channel ID</label>
                <Input
                  value={social.discord_channel_id}
                  onChange={async (e) => {
                    const v = e.target.value
                    setSocial((s) => ({ ...s, discord_channel_id: v }))
                    if (isTauri) await setConfig("social.discord", "channel_id", v)
                  }}
                  placeholder="123456789012345678"
                  className="font-mono text-xs"
                />
              </div>
            </div>
          </section>
        </div>
      )}

      {activeTab === "mcp" && (
        <>
          <div className="mb-3 flex items-center justify-between">
            <h2 className="text-ink-muted text-xs font-medium tracking-wider uppercase">
              MCP 服务器 ({servers.length})
            </h2>
            <Button
              variant="outline"
              size="sm"
              onClick={() => {
                setEditingServer(newServerTemplate())
                setEditingOriginalName(null)
                setShowNewForm(true)
              }}
            >
              + 添加
            </Button>
          </div>

          {/* ── 编辑/新建表单 ── */}
          {editingServer && showNewForm && (
            <McpServerForm
              server={editingServer}
              onChange={setEditingServer}
              onCancel={() => {
                setShowNewForm(false)
                setEditingServer(null)
              }}
              onSave={async () => {
                const isEditing = editingOriginalName !== null
                const url = isEditing
                  ? `/api/mcp/servers/${encodeURIComponent(editingOriginalName)}`
                  : "/api/mcp/servers"
                await fetchJson(url, {
                  method: isEditing ? "PUT" : "POST",
                  body: JSON.stringify(editingServer),
                })
                setShowNewForm(false)
                setEditingServer(null)
                setEditingOriginalName(null)
                loadServers()
              }}
            />
          )}

          {/* ── 已配置列表 ── */}
          <div className="space-y-2">
            {servers.length === 0 && !showNewForm && (
              <div className="bg-bg-surface border-hairline rounded-lg border p-8 text-center">
                <p className="text-ink-muted text-xs">
                  暂无 MCP 服务器配置，点击右上角「+ 添加」开始
                </p>
              </div>
            )}
            {servers.map((s) => (
              <McpServerCard
                key={s.name}
                server={s}
                onToggle={async () => {
                  await fetchJson(`/api/mcp/servers/${encodeURIComponent(s.name)}/toggle`)
                  loadServers()
                }}
                onDelete={async () => {
                  await fetchJson(`/api/mcp/servers/${encodeURIComponent(s.name)}`, {
                    method: "DELETE",
                  })
                  loadServers()
                }}
                onEdit={() => {
                  setEditingServer(s)
                  setEditingOriginalName(s.name)
                  setShowNewForm(true)
                }}
              />
            ))}
          </div>
        </>
      )}

      {activeTab === "about" && (
        <section>
          <h2 className="text-ink-muted mb-3 text-xs font-medium tracking-wider uppercase">关于</h2>
          <div className="bg-bg-surface border-hairline divide-y rounded-lg border">
            {[
              ["应用名称", "OpenATrading"],
              ["版本", "0.12.0"],
              ["构建日期", "2026.06"],
              ["引擎", "pi-rs v1.78.0"],
            ].map(([label, value]) => (
              <div key={label} className="flex justify-between px-4 py-2 text-xs">
                <span className="text-ink-muted">{label}</span>
                <span className="font-mono text-[11px] text-white">{value}</span>
              </div>
            ))}
          </div>
        </section>
      )}
    </div>
  )
}

/* ── MCP Server 编辑表单 ── */
function McpServerForm({
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
                entries
                  .map((e) => {
                    const idx = e.indexOf("=")
                    // hideKey 模式：edited → 裸值；untouched → "arg_i=VALUE"，切掉前缀
                    return idx === -1 ? e : e.slice(idx + 1)
                  })
                  .filter(Boolean)
              )
            }
            keyPlaceholder="序号"
            valuePlaceholder="--port"
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

/* ── MCP Server 卡片 ── */
function McpServerCard({
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

/* ── 键值对列表编辑器（通用） ── */
function KeyValueList({
  items,
  onChange,
  keyPlaceholder,
  valuePlaceholder,
  hideKey,
}: {
  items: string[]
  onChange: (entries: string[]) => void
  keyPlaceholder?: string
  valuePlaceholder?: string
  hideKey?: boolean
}) {
  const entries = items.length > 0 ? items : [""]
  // Normalize: store as "key=value" strings
  const normalize = (list: string[]) => (hideKey ? list : list.filter((e) => e.trim()))

  return (
    <div className="space-y-1.5">
      {entries.map((entry, i) => {
        const eqIdx = entry.indexOf("=")
        const key = eqIdx > 0 ? entry.slice(0, eqIdx) : ""
        const val = eqIdx > 0 ? entry.slice(eqIdx + 1) : entry
        return (
          <div key={i} className="flex gap-1.5">
            {!hideKey && (
              <Input
                value={key}
                onChange={(e) => {
                  const next = [...entries]
                  next[i] = `${e.target.value}=${val}`
                  onChange(normalize(next))
                }}
                placeholder={keyPlaceholder ?? "KEY"}
                className="w-28 font-mono text-xs"
              />
            )}
            <Input
              value={val}
              onChange={(e) => {
                const next = [...entries]
                next[i] = hideKey ? e.target.value : `${key}=${e.target.value}`
                onChange(normalize(next))
              }}
              placeholder={valuePlaceholder ?? "VALUE"}
              className="flex-1 font-mono text-xs"
            />
            <button
              type="button"
              className="hover:text-down text-ink-muted shrink-0 self-center px-1 text-xs"
              onClick={() => onChange(normalize(entries.filter((_, j) => j !== i)))}
            >
              ✕
            </button>
          </div>
        )
      })}
      <button
        type="button"
        className="text-ink-muted text-xs hover:text-white"
        onClick={() => onChange(normalize([...entries, ""]))}
      >
        + 添加
      </button>
    </div>
  )
}
