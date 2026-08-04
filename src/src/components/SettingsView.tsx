import { useState, useEffect, useCallback } from "react"
import { Input } from "@/components/ui/input"
import { Button } from "@/components/ui/button"
import { useUiStore } from "@/stores/ui"
import { useSettingsStore } from "@/stores/settingsStore"
import { fetchJson } from "@/stores/data"
import { McpServerForm } from "./McpServerForm"
import { McpServerCard } from "./McpServerCard"
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

const THEME_LABELS: Record<string, string> = { dark: "暗色", light: "浅色" }

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

  // Load on mount — initialization fetch outside React data flow

  useEffect(() => {
    /* eslint-disable react-hooks/set-state-in-effect -- 初始化加载，不在 React 数据流内 */
    loadServers()
    loadAbout()
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
                  const v = e.target.value
                  setApiUrl(v)
                  setConfig("agent", "api-url", v)
                }}
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
                    try { await setConfig("social", "notify_channel", v) } catch (err) { console.error("保存配置失败", err) }
                  }}
                  className="bg-bg-tertiary border-hairline text-ink w-full rounded-lg border px-3 py-1.5 font-mono text-xs focus:outline-none"
                >
                  <option value="telegram">Telegram</option>
                  <option value="discord">Discord</option>
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
                    try { await setConfig("social.telegram", "bot_token", v) } catch (err) { console.error("保存配置失败", err) }
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
                    try { await setConfig("social.telegram", "chat_id", v) } catch (err) { console.error("保存配置失败", err) }
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
                    try { await setConfig("social.discord", "bot_token", v) } catch (err) { console.error("保存配置失败", err) }
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
                    try { await setConfig("social.discord", "channel_id", v) } catch (err) { console.error("保存配置失败", err) }
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
            {aboutInfo
              ? [
                  ["应用名称", aboutInfo.app_name],
                  ["版本", aboutInfo.version],
                  ["Git 版本", aboutInfo.git_version],
                  ["引擎", aboutInfo.engine],
                ].map(([label, value]) => (
                  <div key={label} className="flex justify-between px-4 py-2 text-xs">
                    <span className="text-ink-muted">{label}</span>
                    <span className="font-mono text-[11px] text-white">{value}</span>
                  </div>
                ))
              : [
                  ["应用名称", "OpenATrading"],
                  ["版本", "加载中…"],
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
