import { useState, useEffect, useCallback } from "react"
import { Input } from "@/components/ui/input"
import { Button } from "@/components/ui/button"
import { Skeleton } from "@/components/ui/skeleton"
import { Switch } from "@/components/ui/switch"
import { useUiStore } from "@/stores/ui"
import { useSettingsStore } from "@/stores/settingsStore"
import { fetchJson } from "@/stores/data"
import {
  useMcpServers,
  useToggleMcpServer,
  useDeleteMcpServer,
  useSaveMcpServer,
} from "@/hooks/query/useMcpServers"
import { McpServerForm } from "./McpServerForm"
import { McpServerCard } from "./McpServerCard"
import ModelPicker from "./ModelPicker"
import { SettingRow, SettingsSection, SelectField } from "./settings-ui"
import { CheckIcon } from "lucide-react"
import type { McpServerPayload } from "../types/mcp"
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

const TAB_TITLES: Record<SettingsTab, string> = {
  appearance: "外观",
  agent: "Agent",
  social: "通知",
  mcp: "MCP 服务器",
  about: "关于",
}

const THINKING_LEVELS: { value: string; label: string }[] = [
  { value: "off", label: "关闭" },
  { value: "minimal", label: "极简" },
  { value: "low", label: "低" },
  { value: "medium", label: "中" },
  { value: "high", label: "高" },
]

type NotifyKey = "collect" | "decision" | "trade" | "news"

const NOTIFY_ITEMS: { key: NotifyKey; label: string; description: string }[] = [
  { key: "collect", label: "采集", description: "数据采集完成时发送通知" },
  { key: "decision", label: "决策", description: "AI 完成交易决策时发送通知" },
  { key: "trade", label: "交易", description: "触发交易信号时发送通知" },
  { key: "news", label: "新闻", description: "收到重要新闻时发送通知" },
]

/* ── 主题选择卡片（Codex 外观页：带预览的明/暗卡片） ── */

/** 固定明/暗预览图（不随当前主题变化，与 Codex 的 base theme 卡片一致） */
function ThemePreview({ dark }: { dark: boolean }) {
  return (
    <div className="border-hairline w-full overflow-hidden rounded-md border">
      {/* 窗口标题栏 + 红绿灯 */}
      <div
        className={`flex items-center gap-1 px-1.5 py-[5px] ${dark ? "bg-[#1f1f1d]" : "bg-[#f0f0ee]"}`}
      >
        <span className="size-[6px] rounded-full bg-[#ff5f57]" />
        <span className="size-[6px] rounded-full bg-[#febc2e]" />
        <span className="size-[6px] rounded-full bg-[#28c840]" />
        <span
          className={`ml-1.5 h-[5px] w-10 rounded-full ${dark ? "bg-white/25" : "bg-black/15"}`}
        />
      </div>
      {/* 窗口内容：左侧导航条 + 内容骨架 */}
      <div className={`flex h-11 ${dark ? "bg-[#262624]" : "bg-white"}`}>
        <div
          className={`w-5 border-r ${dark ? "border-white/10 bg-[#1c1c1a]" : "border-black/5 bg-[#faf9f5]"}`}
        />
        <div className="flex flex-1 flex-col justify-center gap-[5px] px-1.5">
          <div className={`h-[5px] w-2/3 rounded-full ${dark ? "bg-white/30" : "bg-black/15"}`} />
          <div className={`h-[5px] w-full rounded-full ${dark ? "bg-white/12" : "bg-black/8"}`} />
          <div className={`h-[5px] w-1/2 rounded-full ${dark ? "bg-white/12" : "bg-black/8"}`} />
        </div>
      </div>
    </div>
  )
}

function ThemeCard({
  label,
  dark,
  active,
  onSelect,
}: {
  label: string
  dark: boolean
  active: boolean
  onSelect: () => void
}) {
  return (
    <button
      type="button"
      onClick={onSelect}
      aria-pressed={active}
      className={`relative w-[140px] rounded-lg border p-1.5 text-left transition-colors ${
        active ? "border-brand ring-brand/25 ring-2" : "border-hairline hover:border-secondary/50"
      }`}
    >
      <ThemePreview dark={dark} />
      <span
        className={`mt-1.5 block px-0.5 text-xs ${
          active ? "text-primary font-medium" : "text-secondary"
        }`}
      >
        {label}
      </span>
      {active && (
        <span className="bg-brand absolute top-2.5 right-2.5 grid size-[18px] place-items-center rounded-full shadow-sm">
          <CheckIcon className="text-brand-foreground size-3" />
        </span>
      )}
    </button>
  )
}

/* ── 右侧控件宽度常量 ── */
const inputCls = "w-64 bg-surface font-number text-caption"

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
  const [editingServer, setEditingServer] = useState<McpServerPayload | null>(null)
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

  // MCP 服务器列表 + 增删改切换（react-query 缓存，乐观更新）
  const mcpQuery = useMcpServers()
  const toggleMcp = useToggleMcpServer()
  const deleteMcp = useDeleteMcpServer()
  const saveMcp = useSaveMcpServer()
  const servers = mcpQuery.data ?? []

  const newServerTemplate = (): McpServerPayload => ({
    name: "",
    enabled: true,
    transport: { type: "http", url: "", headers: {} },
  })

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
  }, [loadAbout, loadAgentSettings])

  const aboutRows: [string, string][] = aboutInfo
    ? [
        ["应用名称", aboutInfo.app_name],
        ["版本", aboutInfo.version],
        ["Git 版本", aboutInfo.git_version],
        ["引擎", aboutInfo.engine],
      ]
    : [
        ["应用名称", "OpenATrading"],
        ["版本", "加载中…"],
      ]

  return (
    <div className="mx-auto w-full max-w-2xl px-8 pt-8 pb-16">
      {activeTab === "mcp" ? (
        <div className="mb-8 flex items-center justify-between">
          <h1 className="text-h1 text-primary">MCP 服务器 ({servers.length})</h1>
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
      ) : (
        <h1 className="text-h1 text-primary mb-8">{TAB_TITLES[activeTab]}</h1>
      )}
      <div className="space-y-8">
        {activeTab === "appearance" && (
          <section>
            <h2 className="text-primary mb-3 text-[13px] font-semibold">基础主题</h2>
            <div className="flex items-start gap-3">
              <ThemeCard
                label={THEME_LABELS.light}
                dark={false}
                active={theme === "light"}
                onSelect={() => setTheme("light")}
              />
              <ThemeCard
                label={THEME_LABELS.dark}
                dark
                active={theme === "dark"}
                onSelect={() => setTheme("dark")}
              />
            </div>
          </section>
        )}

        {activeTab === "agent" && (
          <>
            <SettingsSection title="默认模型">
              <ModelPicker
                provider={agentSettings.defaultProvider}
                model={agentSettings.defaultModel}
                onChange={(provider, model) =>
                  saveAgentSettings({ defaultProvider: provider, defaultModel: model })
                }
              />
              <SettingRow
                label="思考级别"
                description="控制模型推理深度，级别越高耗时越长"
                control={
                  <SelectField
                    value={agentSettings.thinkingLevel}
                    onChange={(v) => saveAgentSettings({ thinkingLevel: v })}
                    options={THINKING_LEVELS}
                    className="w-[140px]"
                  />
                }
              />
            </SettingsSection>

            <SettingsSection title="数据源">
              <SettingRow
                label="API 地址"
                description="数据服务与 Agent 网关地址"
                control={
                  <Input
                    value={apiUrl}
                    onChange={(e) => {
                      const v = e.target.value
                      setApiUrl(v)
                      setConfig("agent", "api-url", v)
                    }}
                    placeholder="http://localhost:8081"
                    className={inputCls}
                  />
                }
              />
            </SettingsSection>

            {isTauri && (
              <SettingsSection title="操作">
                <SettingRow
                  label="重启客户端"
                  description="修改 Provider / Model 后重启生效"
                  control={
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
                  }
                />
              </SettingsSection>
            )}
          </>
        )}

        {activeTab === "social" && (
          <>
            <SettingsSection title="通知渠道">
              <SettingRow
                label="默认通知渠道"
                description="选择推送消息使用的平台"
                control={
                  <SelectField
                    value={social.notify_channel}
                    onChange={async (v) => {
                      setSocial((s) => ({ ...s, notify_channel: v }))
                      try {
                        await setConfig("social", "notify_channel", v)
                      } catch (err) {
                        console.error("保存配置失败", err)
                      }
                    }}
                    options={[
                      { value: "telegram", label: "Telegram" },
                      { value: "discord", label: "Discord" },
                    ]}
                    className="w-[160px]"
                  />
                }
              />
            </SettingsSection>

            <SettingsSection title="通知事件">
              {NOTIFY_ITEMS.map((item) => (
                <SettingRow
                  key={item.key}
                  label={item.label}
                  description={item.description}
                  control={
                    <Switch
                      checked={social[`notify_on_${item.key}`] as boolean}
                      onCheckedChange={async (v) => {
                        setSocial((s) => ({ ...s, [`notify_on_${item.key}`]: v }))
                        try {
                          await setConfig("social", `notify_on_${item.key}`, String(v))
                        } catch (err) {
                          console.error("保存配置失败", err)
                        }
                      }}
                    />
                  }
                />
              ))}
            </SettingsSection>

            <SettingsSection title="Telegram">
              <SettingRow
                label="Bot Token"
                description="机器人令牌，用于发送消息"
                control={
                  <Input
                    type="password"
                    value={social.telegram_bot_token}
                    onChange={async (e) => {
                      const v = e.target.value
                      setSocial((s) => ({ ...s, telegram_bot_token: v }))
                      try {
                        await setConfig("social.telegram", "bot_token", v)
                      } catch (err) {
                        console.error("保存配置失败", err)
                      }
                    }}
                    placeholder="123456:ABC-DEF1234gh..."
                    className={inputCls}
                  />
                }
              />
              <SettingRow
                label="Chat ID"
                description="接收通知的会话 ID"
                control={
                  <Input
                    value={social.telegram_chat_id}
                    onChange={async (e) => {
                      const v = e.target.value
                      setSocial((s) => ({ ...s, telegram_chat_id: v }))
                      try {
                        await setConfig("social.telegram", "chat_id", v)
                      } catch (err) {
                        console.error("保存配置失败", err)
                      }
                    }}
                    placeholder="-1001234567890"
                    className={inputCls}
                  />
                }
              />
            </SettingsSection>

            <SettingsSection title="Discord">
              <SettingRow
                label="Bot Token"
                description="机器人令牌，用于发送消息"
                control={
                  <Input
                    type="password"
                    value={social.discord_bot_token}
                    onChange={async (e) => {
                      const v = e.target.value
                      setSocial((s) => ({ ...s, discord_bot_token: v }))
                      try {
                        await setConfig("social.discord", "bot_token", v)
                      } catch (err) {
                        console.error("保存配置失败", err)
                      }
                    }}
                    placeholder="MTE4..."
                    className={inputCls}
                  />
                }
              />
              <SettingRow
                label="Channel ID"
                description="接收通知的频道 ID"
                control={
                  <Input
                    value={social.discord_channel_id}
                    onChange={async (e) => {
                      const v = e.target.value
                      setSocial((s) => ({ ...s, discord_channel_id: v }))
                      try {
                        await setConfig("social.discord", "channel_id", v)
                      } catch (err) {
                        console.error("保存配置失败", err)
                      }
                    }}
                    placeholder="123456789012345678"
                    className={inputCls}
                  />
                }
              />
            </SettingsSection>
          </>
        )}

        {activeTab === "mcp" && (
          <>
            {/* ── 编辑/新建表单 ── */}
            {editingServer && showNewForm && (
              <div className="mt-3">
                <McpServerForm
                  key={editingOriginalName ?? "new"}
                  server={editingServer}
                  onChange={setEditingServer}
                  onCancel={() => {
                    setShowNewForm(false)
                    setEditingServer(null)
                  }}
                  onSave={async () => {
                    await saveMcp.mutateAsync({
                      config: { ...editingServer, name: editingServer.name.trim() },
                      originalName: editingOriginalName,
                    })
                    setShowNewForm(false)
                    setEditingServer(null)
                    setEditingOriginalName(null)
                  }}
                  existingNames={servers
                    .filter((s) => s.name !== editingOriginalName)
                    .map((s) => s.name)}
                />
              </div>
            )}

            {/* ── 已配置列表 ── */}
            <div className="divide-hairline mt-3 divide-y">
              {mcpQuery.isPending ? (
                Array.from({ length: 3 }).map((_, i) => (
                  <Skeleton key={i} className="h-[52px] w-full rounded-none" />
                ))
              ) : mcpQuery.isError ? (
                <div className="flex flex-col items-center gap-2 py-10 text-center">
                  <p className="text-secondary text-caption">无法加载 MCP 服务器列表</p>
                  <Button size="xs" variant="outline" onClick={() => mcpQuery.refetch()}>
                    重试
                  </Button>
                </div>
              ) : servers.length === 0 && !showNewForm ? (
                <div className="py-10 text-center">
                  <p className="text-secondary text-caption">
                    暂无 MCP 服务器配置，点击右上角「+ 添加」开始
                  </p>
                </div>
              ) : (
                servers.map((s) => (
                  <McpServerCard
                    key={s.name}
                    server={s}
                    onToggle={() => toggleMcp.mutateAsync(s.name).then(() => undefined)}
                    onDelete={() => deleteMcp.mutateAsync(s.name).then(() => undefined)}
                    onEdit={() => {
                      setEditingServer({
                        name: s.name,
                        enabled: s.enabled,
                        transport: s.transport,
                      })
                      setEditingOriginalName(s.name)
                      setShowNewForm(true)
                    }}
                  />
                ))
              )}
            </div>

            <p className="text-secondary text-caption mt-4">
              新增与修改写入 pi-rs 的全局 <code>$PI_RS_HOME/agent/mcp.json</code>，
              对之后新建的会话生效；项目级 <code>{"{cwd}"}/.pi-rs/mcp.json</code>{" "}
              的同名条目会覆盖全局，需直接编辑该文件。
            </p>
          </>
        )}

        {activeTab === "about" && (
          <SettingsSection title="关于">
            {aboutRows.map(([label, value]) => (
              <div key={label} className="flex items-center justify-between py-3">
                <span className="text-secondary text-sm">{label}</span>
                <span className="text-primary font-number text-caption">{value}</span>
              </div>
            ))}
          </SettingsSection>
        )}
      </div>
    </div>
  )
}
