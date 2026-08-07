import { useState, useEffect, useCallback } from "react"
import { Input } from "@/components/ui/input"
import { Button } from "@/components/ui/button"
import { useUiStore } from "@/stores/ui"
import { useSettingsStore } from "@/stores/settingsStore"
import { fetchJson } from "@/stores/data"
import { McpServerForm } from "./McpServerForm"
import { McpServerCard } from "./McpServerCard"
import ModelPicker from "./ModelPicker"
import type { McpServerConfig } from "../types/mcp"
import type { SettingsTab } from "@/components/SettingsSidebar"

import { isTauri } from "@/lib/env"
async function setConfig(section: string, key: string, value: string) {
  if (isTauri) {
    const { invoke } = await import("@tauri-apps/api/core")
    await invoke("update_config", { section, key, value })
  }
  // localStorage key 包含 section 信息避免冲突（social.telegram + bot_token → telegram_bot_token）
  const lsKey = section.includes(".") ? `${section.split(".").pop()}_${key}` : key
  localStorage.setItem(`openatrading-${lsKey}`, value)
}

interface SocialConfig {
  notify_channel: string
  notify_on_collect: boolean
  notify_on_decision: boolean
  notify_on_trade: boolean
  notify_on_news: boolean
  telegram_bot_token: string
  telegram_chat_id: string
  discord_bot_token: string
  discord_channel_id: string
}

interface AgentSettings {
  defaultProvider: string
  defaultModel: string
  thinkingLevel: string
}

const THEME_LABELS: Record<string, string> = { dark: "暗色", light: "浅色" }

const THINKING_LEVELS: { value: string; label: string }[] = [
  { value: "off", label: "关闭" },
  { value: "minimal", label: "极简" },
  { value: "low", label: "低" },
  { value: "medium", label: "中" },
  { value: "high", label: "高" },
]

export default function SettingsView({ activeTab }: { activeTab: SettingsTab }) {
  const theme = useUiStore((s) => s.theme)
  const setTheme = useUiStore((s) => s.setTheme)

  const apiUrl = useSettingsStore((s) => s.apiUrl)
  const setApiUrl = useSettingsStore((s) => s.setApiUrl)
  const [restarting, setRestarting] = useState(false)
  // Social notification state
  const [social, setSocial] = useState<SocialConfig>({
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
  // pi-rs settings.json 默认值（provider / model / thinkingLevel）
  const [agentSettings, setAgentSettings] = useState<AgentSettings>({
    defaultProvider: "",
    defaultModel: "",
    thinkingLevel: "medium",
  })
  // About info
  const [aboutInfo, setAboutInfo] = useState<Record<string, string> | null>(null)

  const newServerTemplate = (): McpServerConfig => ({
    name: "",
    enabled: true,
    transport: { type: "streamable-http", url: "" },
  })

  const loadServers = useCallback(async () => {
    try {
      const data = await fetchJson<McpServerConfig[]>("/api/mcp/servers")
      setServers(data)
    } catch {
      // MCP API may not be available
    }
  }, [])

  const loadAbout = useCallback(async () => {
    try {
      const data = await fetchJson<Record<string, string>>("/api/about")
      setAboutInfo(data)
    } catch {
      // API may not be available
    }
  }, [])

  const loadAgentSettings = useCallback(async () => {
    try {
      const data = await fetchJson<Partial<AgentSettings>>("/api/agent/settings")
      setAgentSettings({
        defaultProvider: data.defaultProvider ?? "",
        defaultModel: data.defaultModel ?? "",
        thinkingLevel: data.thinkingLevel ?? "medium",
      })
    } catch {
      // API 可能不可用（Web 端网关未启动等）
    }
  }, [])

  const saveAgentSettings = useCallback(
    async (patch: Partial<AgentSettings>) => {
      const next = { ...agentSettings, ...patch }
      setAgentSettings(next)
      try {
        await fetchJson("/api/agent/settings", {
          method: "POST",
          body: JSON.stringify({
            defaultProvider: next.defaultProvider,
            defaultModel: next.defaultModel,
            thinkingLevel: next.thinkingLevel,
          }),
        })
      } catch (e) {
        console.error("保存 Agent 设置失败", e)
      }
    },
    [agentSettings]
  )

  // Load on mount — initialization fetch outside React data flow

  useEffect(() => {
    /* eslint-disable react-hooks/set-state-in-effect -- 初始化加载，不在 React 数据流内 */
    loadServers()
    loadAbout()
    loadAgentSettings()
    if (isTauri) {
      ;(async () => {
        const { invoke } = await import("@tauri-apps/api/core")
        try {
          const cfg = (await invoke("get_config")) as { social?: Partial<SocialConfig> }
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
  }, [loadServers])

  return (
    <div className="readable-content flex h-full flex-1 flex-col gap-5 overflow-y-auto p-6">
      {activeTab === "appearance" && (
        <section>
          <h2 className="text-secondary mb-3 text-caption font-medium tracking-wider uppercase">外观</h2>
          <div className="bg-surface border-hairline rounded-lg border p-4">
            <div className="flex items-center justify-between">
              <div>
                <span className="text-body text-primary">主题</span>
                <p className="text-secondary mt-0.5 text-caption">选择深色或浅色主题</p>
              </div>
              <div className="border-hairline flex gap-0 overflow-hidden rounded-md border">
                {(["dark", "light"] as const).map((t) => (
                  <button
                    key={t}
                    onClick={() => setTheme(t)}
                    className={`px-3 py-2 text-caption transition-colors ${
                      theme === t
                        ? "bg-brand text-primary"
                        : "text-secondary hover:bg-hover bg-transparent hover:text-primary"
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
            <h2 className="text-secondary mb-3 text-caption font-medium tracking-wider uppercase">
              默认模型
            </h2>
            <div className="bg-surface border-hairline space-y-3 rounded-lg border p-4">
              <ModelPicker
                provider={agentSettings.defaultProvider}
                model={agentSettings.defaultModel}
                onChange={(provider, model) =>
                  saveAgentSettings({ defaultProvider: provider, defaultModel: model })
                }
              />
              <div>
                <label className="text-secondary mb-1 block text-caption">思考级别</label>
                <select
                  value={agentSettings.thinkingLevel}
                  onChange={(e) => saveAgentSettings({ thinkingLevel: e.target.value })}
                  className="bg-elevated border-hairline text-primary w-full rounded-lg border px-3 py-2 font-number text-caption focus:outline-none"
                >
                  {THINKING_LEVELS.map((t) => (
                    <option key={t.value} value={t.value}>
                      {t.label}
                    </option>
                  ))}
                </select>
              </div>
            </div>
          </section>

          <section>
            <h2 className="text-secondary mb-3 text-caption font-medium tracking-wider uppercase">
              数据源
            </h2>
            <div className="bg-surface border-hairline rounded-lg border p-4">
              <label className="text-secondary mb-1 block text-caption">API 地址</label>
              <Input
                value={apiUrl}
                onChange={(e) => {
                  const v = e.target.value
                  setApiUrl(v)
                  setConfig("agent", "api-url", v)
                }}
              />
            </div>
          </section>

          {isTauri && (
            <section>
              <h2 className="text-secondary mb-3 text-caption font-medium tracking-wider uppercase">
                操作
              </h2>
              <div className="bg-surface border-hairline rounded-lg border p-4">
                <div className="flex items-center justify-between">
                  <div>
                    <span className="text-body text-primary">重启客户端</span>
                    <p className="text-secondary mt-0.5 text-caption">
                      修改 Provider/Model 后重启生效
                    </p>
                  </div>
                  <Button
                    size="sm"
                    disabled={restarting}
                    onClick={async () => {
                      setRestarting(true)
                      try {
                        const { invoke } = await import("@tauri-apps/api/core")
                        await invoke("restart_app")
                      } catch (e) {
                        console.error("重启失败", e)
                        setRestarting(false)
                      }
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
            <h2 className="text-secondary mb-3 text-caption font-medium tracking-wider uppercase">
              通知渠道
            </h2>
            <div className="bg-surface border-hairline space-y-3 rounded-lg border p-4">
              <div>
                <label className="text-secondary mb-1 block text-caption">默认通知渠道</label>
                <select
                  value={social.notify_channel}
                  onChange={async (e) => {
                    const v = e.target.value
                    setSocial((s) => ({ ...s, notify_channel: v }))
                    try { await setConfig("social", "notify_channel", v) } catch (err) { console.error("保存配置失败", err) }
                  }}
                  className="bg-elevated border-hairline text-primary w-full rounded-lg border px-3 py-2 font-number text-caption focus:outline-none"
                >
                  <option value="telegram">Telegram</option>
                  <option value="discord">Discord</option>
                </select>
              </div>
              <div className="flex flex-wrap gap-4">
                {(["collect", "decision", "trade", "news"] as const).map((k) => (
                  <label key={k} className="flex items-center gap-2 text-caption text-primary">
                    <input
                      type="checkbox"
                      checked={social[`notify_on_${k}` as keyof typeof social] as boolean}
                      onChange={async (e) => {
                        const v = e.target.checked
                        setSocial((s) => ({ ...s, [`notify_on_${k}`]: v }))
                        try { await setConfig("social", `notify_on_${k}`, String(v)) } catch (err) { console.error("保存配置失败", err) }
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
            <h2 className="text-secondary mb-3 text-caption font-medium tracking-wider uppercase">
              Telegram
            </h2>
            <div className="bg-surface border-hairline space-y-3 rounded-lg border p-4">
              <div>
                <label className="text-secondary mb-1 block text-caption">Bot Token</label>
                <Input
                  type="password"
                  value={social.telegram_bot_token}
                  onChange={async (e) => {
                    const v = e.target.value
                    setSocial((s) => ({ ...s, telegram_bot_token: v }))
                    try { await setConfig("social.telegram", "bot_token", v) } catch (err) { console.error("保存配置失败", err) }
                  }}
                  placeholder="123456:ABC-DEF1234gh..."
                  className="font-number text-caption"
                />
              </div>
              <div>
                <label className="text-secondary mb-1 block text-caption">Chat ID</label>
                <Input
                  value={social.telegram_chat_id}
                  onChange={async (e) => {
                    const v = e.target.value
                    setSocial((s) => ({ ...s, telegram_chat_id: v }))
                    try { await setConfig("social.telegram", "chat_id", v) } catch (err) { console.error("保存配置失败", err) }
                  }}
                  placeholder="-1001234567890"
                  className="font-number text-caption"
                />
              </div>
            </div>
          </section>

          <section>
            <h2 className="text-secondary mb-3 text-caption font-medium tracking-wider uppercase">
              Discord
            </h2>
            <div className="bg-surface border-hairline space-y-3 rounded-lg border p-4">
              <div>
                <label className="text-secondary mb-1 block text-caption">Bot Token</label>
                <Input
                  type="password"
                  value={social.discord_bot_token}
                  onChange={async (e) => {
                    const v = e.target.value
                    setSocial((s) => ({ ...s, discord_bot_token: v }))
                    try { await setConfig("social.discord", "bot_token", v) } catch (err) { console.error("保存配置失败", err) }
                  }}
                  placeholder="MTE4..."
                  className="font-number text-caption"
                />
              </div>
              <div>
                <label className="text-secondary mb-1 block text-caption">Channel ID</label>
                <Input
                  value={social.discord_channel_id}
                  onChange={async (e) => {
                    const v = e.target.value
                    setSocial((s) => ({ ...s, discord_channel_id: v }))
                    try { await setConfig("social.discord", "channel_id", v) } catch (err) { console.error("保存配置失败", err) }
                  }}
                  placeholder="123456789012345678"
                  className="font-number text-caption"
                />
              </div>
            </div>
          </section>
        </div>
      )}

      {activeTab === "mcp" && (
        <>
          <div className="mb-3 flex items-center justify-between">
            <h2 className="text-secondary text-caption font-medium tracking-wider uppercase">
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
              key={editingOriginalName ?? "new"}
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
              <div className="bg-surface border-hairline rounded-lg border p-8 text-center">
                <p className="text-secondary text-caption">
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
          <h2 className="text-secondary mb-3 text-caption font-medium tracking-wider uppercase">关于</h2>
          <div className="bg-surface border-hairline divide-y rounded-lg border">
            {aboutInfo
              ? [
                  ["应用名称", aboutInfo.app_name],
                  ["版本", aboutInfo.version],
                  ["Git 版本", aboutInfo.git_version],
                  ["引擎", aboutInfo.engine],
                ].map(([label, value]) => (
                  <div key={label} className="flex justify-between px-4 py-2 text-caption">
                    <span className="text-secondary">{label}</span>
                    <span className="font-number text-caption text-primary">{value}</span>
                  </div>
                ))
              : [
                  ["应用名称", "OpenATrading"],
                  ["版本", "加载中…"],
                ].map(([label, value]) => (
                  <div key={label} className="flex justify-between px-4 py-2 text-caption">
                    <span className="text-secondary">{label}</span>
                    <span className="font-number text-caption text-primary">{value}</span>
                  </div>
                ))}
          </div>
        </section>
      )}
    </div>
  )
}
