import { useState, useEffect } from "react"
import { Input } from "@/components/ui/input"
import { Button } from "@/components/ui/button"
import { useUiStore } from "@/stores/ui"
import { fetchJson } from "@/stores/data"
import type { SettingsTab } from "@/components/SettingsSidebar"

const isTauri = "__TAURI_INTERNALS__" in window

async function setConfig(section: string, key: string, value: string) {
  if (isTauri) {
    const { invoke } = await import("@tauri-apps/api/core")
    await invoke("update_config", { section, key, value })
  }
  localStorage.setItem(`openatrading-${key}`, value)
}

interface McpServerConfig {
  name: string
  transport: { type: string; url?: string; command?: string; args?: string[] }
  enabled: boolean
}

const THEME_LABELS: Record<string, string> = { dark: "暗色", light: "浅色" }

export default function SettingsView({ activeTab }: { activeTab: SettingsTab }) {
  const theme = useUiStore((s) => s.theme)
  const setTheme = useUiStore((s) => s.setTheme)

  const [apiKey, setApiKey] = useState(() => localStorage.getItem("openatrading-api-key") || "")
  const [provider, setProvider] = useState(
    () => localStorage.getItem("openatrading-provider") || "openrouter"
  )
  const [model, setModel] = useState(
    () => localStorage.getItem("openatrading-model") || "claude-sonnet-4-6"
  )
  const [apiUrl, setApiUrl] = useState(
    () => localStorage.getItem("openatrading-api-url") || "http://localhost:8080"
  )
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
  const [socialLoaded, setSocialLoaded] = useState(false)
  const [servers, setServers] = useState<McpServerConfig[]>([])
  const [showAddForm, setShowAddForm] = useState(false)
  const [newServer, setNewServer] = useState({ name: "", url: "" })

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
    loadServers()
    if (isTauri) {
      ;(async () => {
        const { invoke } = await import("@tauri-apps/api/core")
        try {
          const cfg = await invoke("get_config") as any
          if (cfg?.social) {
            setSocial((s) => ({ ...s, ...cfg.social }))
          }
          if (cfg?.llm) {
            if (cfg.llm.provider) setProvider(cfg.llm.provider)
            if (cfg.llm.model) setModel(cfg.llm.model)
          }
        } catch { /* ignore */ }
        setSocialLoaded(true)
      })()
    } else {
      setSocialLoaded(true)
    }
  }, [])

  return (
    <div className="flex h-full flex-1 flex-col gap-5 overflow-y-auto p-6">
      {activeTab === "appearance" && (
        <>
          <section>
            <h2 className="text-ink-muted mb-3 text-xs font-medium tracking-wider uppercase">
              外观
            </h2>
            <div className="border-hairline bg-bg-surface rounded-lg border p-4">
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

          <section>
            <h2 className="text-ink-muted mb-3 text-xs font-medium tracking-wider uppercase">
              Agent 配置
            </h2>
            <div className="border-hairline bg-bg-surface space-y-3 rounded-lg border p-4">
              <div>
                <label className="text-ink-muted mb-1 block text-xs">API Key</label>
                <Input
                  type="password"
                  value={apiKey}
                  onChange={(e) => {
                    setApiKey(e.target.value)
                    localStorage.setItem("openatrading-api-key", e.target.value)
                  }}
                  placeholder="sk-..."
                  className="font-mono text-xs"
                />
              </div>
              <div>
                <label className="text-ink-muted mb-1 block text-xs">Provider</label>
                <select
                  value={provider}
                  onChange={async (e) => {
                    const v = e.target.value
                    setProvider(v)
                    await setConfig("llm", "provider", v)
                  }}
                  className="border-hairline bg-bg-tertiary text-ink w-full rounded-lg border px-3 py-1.5 font-mono text-xs focus:outline-none"
                >
                  <option value="openrouter">OpenRouter</option>
                  <option value="anthropic">Anthropic</option>
                  <option value="openai">OpenAI</option>
                  <option value="deepseek">DeepSeek</option>
                  <option value="siliconflow">硅基流动 (SiliconFlow)</option>
                  <option value="github">GitHub Models</option>
                </select>
              </div>
              <div>
                <label className="text-ink-muted mb-1 block text-xs">默认模型</label>
                <Input
                  value={model}
                  onChange={async (e) => {
                    const v = e.target.value
                    setModel(v)
                    await setConfig("llm", "model", v)
                  }}
                  placeholder="claude-sonnet-4-6"
                  className="font-mono text-xs"
                />
              </div>
            </div>
          </section>

          {isTauri && (
            <section>
              <h2 className="text-ink-muted mb-3 text-xs font-medium tracking-wider uppercase">
                操作
              </h2>
              <div className="border-hairline bg-bg-surface rounded-lg border p-4">
                <div className="flex items-center justify-between">
                  <div>
                    <span className="text-sm text-white">重启客户端</span>
                    <p className="text-ink-muted mt-0.5 text-[11px]">修改 Provider/Model 后重启生效</p>
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

          <section>
            <h2 className="text-ink-muted mb-3 text-xs font-medium tracking-wider uppercase">
              数据源
            </h2>
            <div className="border-hairline bg-bg-surface rounded-lg border p-4">
              <label className="text-ink-muted mb-1 block text-xs">API 地址</label>
              <Input
                value={apiUrl}
                onChange={(e) => {
                  setApiUrl(e.target.value)
                  localStorage.setItem("openatrading-api-url", e.target.value)
                }}
                placeholder="http://localhost:8080"
                className="font-mono text-xs"
              />
            </div>
          </section>
        </>
      )}

      {activeTab === "social" && (
        <div className="space-y-5">
          <section>
            <h2 className="text-ink-muted mb-3 text-xs font-medium tracking-wider uppercase">
              通知渠道
            </h2>
            <div className="border-hairline bg-bg-surface space-y-3 rounded-lg border p-4">
              <div>
                <label className="text-ink-muted mb-1 block text-xs">默认通知渠道</label>
                <select
                  value={social.notify_channel}
                  onChange={async (e) => {
                    const v = e.target.value
                    setSocial((s) => ({ ...s, notify_channel: v }))
                    if (isTauri) await setConfig("social", "notify_channel", v)
                  }}
                  className="border-hairline bg-bg-tertiary text-ink w-full rounded-lg border px-3 py-1.5 font-mono text-xs focus:outline-none"
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
                    {k === "collect" ? "采集" : k === "decision" ? "决策" : k === "trade" ? "交易" : "新闻"}
                  </label>
                ))}
              </div>
            </div>
          </section>

          <section>
            <h2 className="text-ink-muted mb-3 text-xs font-medium tracking-wider uppercase">
              Telegram
            </h2>
            <div className="border-hairline bg-bg-surface space-y-3 rounded-lg border p-4">
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
            <div className="border-hairline bg-bg-surface space-y-3 rounded-lg border p-4">
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
        <section>
          <div className="mb-3 flex items-center justify-between">
            <h2 className="text-ink-muted text-xs font-medium tracking-wider uppercase">
              MCP 服务器 ({servers.length})
            </h2>
            <Button variant="outline" size="sm" onClick={() => setShowAddForm(!showAddForm)}>
              + 添加
            </Button>
          </div>

          {showAddForm && (
            <div className="border-hairline bg-bg-surface mb-3 space-y-3 rounded-lg border p-4">
              <div className="flex gap-2">
                <div className="flex-1">
                  <label className="text-ink-muted mb-1 block text-xs">名称</label>
                  <Input
                    value={newServer.name}
                    onChange={(e) => setNewServer((s) => ({ ...s, name: e.target.value }))}
                    placeholder="例如: 文件系统"
                    className="text-xs"
                  />
                </div>
                <div className="flex-[2]">
                  <label className="text-ink-muted mb-1 block text-xs">URL</label>
                  <Input
                    value={newServer.url}
                    onChange={(e) => setNewServer((s) => ({ ...s, url: e.target.value }))}
                    placeholder="http://localhost:8081/mcp"
                    className="font-mono text-xs"
                  />
                </div>
              </div>
              <div className="flex justify-end gap-2">
                <Button variant="outline" size="sm" onClick={() => setShowAddForm(false)}>
                  取消
                </Button>
                <Button
                  size="sm"
                  disabled={!newServer.name || !newServer.url}
                  onClick={async () => {
                    const config: McpServerConfig = {
                      name: newServer.name,
                      enabled: true,
                      transport: { type: "streamable-http", url: newServer.url },
                    }
                    try {
                      await fetchJson("/api/mcp/servers", {
                        method: "POST",
                        body: JSON.stringify(config),
                      })
                      setNewServer({ name: "", url: "" })
                      setShowAddForm(false)
                      loadServers()
                    } catch {
                      /* silently fail */
                    }
                  }}
                >
                  添加
                </Button>
              </div>
            </div>
          )}

          <div className="space-y-2">
            {servers.length === 0 && !showAddForm && (
              <div className="border-hairline bg-bg-surface rounded-lg border p-8 text-center">
                <p className="text-ink-muted text-xs">暂无 MCP 服务器配置</p>
              </div>
            )}
            {servers.map((s) => (
              <div
                key={s.name}
                className="border-hairline bg-bg-surface flex items-center gap-3 rounded-lg border p-3 transition-colors hover:border-white/10"
              >
                <span
                  className={`size-1.5 flex-shrink-0 rounded-full ${s.enabled ? "bg-up" : "bg-ink-muted"}`}
                />
                <div className="min-w-0 flex-1">
                  <span className="text-sm text-white">{s.name}</span>
                  <p className="text-ink-muted truncate font-mono text-[11px]">
                    {s.transport.type === "stdio"
                      ? `${s.transport.command} ${(s.transport as { args: string[] }).args?.join(" ") ?? ""}`
                      : (s.transport as { url: string }).url}
                  </p>
                </div>
                <div className="flex flex-shrink-0 gap-1">
                  <button
                    className="text-ink-muted hover:bg-bg-hover rounded px-2 py-1 text-[10px] transition-colors hover:text-white"
                    onClick={async () => {
                      try {
                        await fetchJson(`/api/mcp/servers/${encodeURIComponent(s.name)}/toggle`)
                        loadServers()
                      } catch {
                        /* noop */
                      }
                    }}
                  >
                    {s.enabled ? "停用" : "启用"}
                  </button>
                  <button
                    className="text-ink-muted hover:bg-bg-hover hover:text-down rounded px-2 py-1 text-[10px] transition-colors"
                    onClick={async () => {
                      try {
                        await fetchJson(`/api/mcp/servers/${encodeURIComponent(s.name)}`, {
                          method: "DELETE",
                        })
                        loadServers()
                      } catch {
                        /* noop */
                      }
                    }}
                  >
                    删除
                  </button>
                </div>
              </div>
            ))}
          </div>
        </section>
      )}

      {activeTab === "about" && (
        <section>
          <h2 className="text-ink-muted mb-3 text-xs font-medium tracking-wider uppercase">关于</h2>
          <div className="border-hairline bg-bg-surface divide-y rounded-lg border">
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
