import { useState, useEffect, useCallback } from "react"
import { Input } from "@/components/ui/input"
import { Button } from "@/components/ui/button"
import { Skeleton } from "@/components/ui/skeleton"
import { Switch } from "@/components/ui/switch"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { InputGroup, InputGroupAddon, InputGroupInput } from "@/components/ui/input-group"
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
import { McpTemplateGrid } from "./McpTemplateGrid"
import ModelPicker from "./ModelPicker"
import { AgentApiKeySection } from "./AgentApiKeySection"
import { SettingRow, SettingsSection, SelectField } from "./settings-ui"
import { CheckIcon, Info, Plus, RefreshCw, Search, Server } from "lucide-react"
import {
  emptyTransport,
  mcpTransportTarget,
  type McpServerPayload,
  type McpTransportType,
} from "../types/mcp"
import { SETTINGS_TABS } from "@/lib/settings-tabs"
import type { SettingsTab } from "@/stores/ui"

import { isTauri } from "@/lib/env"

/**
 * 凭据类字段永远不写 localStorage。
 *
 * localStorage 是持久化的明文存储：同机其他用户、浏览器扩展、任何 XSS
 * 都能读到，而且退出登录后依然留在磁盘上。bot token / chat id / 密码这类
 * 东西只留在服务端配置（`~/.atrading/config.toml` + `.env`），前端只负责
 * 提交，不负责持久化。
 *
 * 读回来的路径同样跳过（见下方 loadConfig），因此这些输入框在刷新后是空的：
 * 拿不到值就不显示——留空即不修改，服务端现值保留。
 */
const SENSITIVE_KEY_RE = /(token|secret|password|passwd|api[-_]?key|chat_id|channel_id|phone)/i

async function setConfig(section: string, key: string, value: string) {
  if (isTauri) {
    const { invoke } = await import("@tauri-apps/api/core")
    await invoke("update_config", { section, key, value })
  }
  if (SENSITIVE_KEY_RE.test(key)) return
  // localStorage key 包含 section 信息避免冲突（social.telegram + bot_token → telegram_bot_token）
  const lsKey = section.includes(".") ? `${section.split(".").pop()}_${key}` : key
  localStorage.setItem(`openatrading-${lsKey}`, value)
}

interface SocialConfig {
  notify_channel: string
  notify_on_collect: boolean
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

type NotifyKey = "collect" | "news"

const NOTIFY_ITEMS: { key: NotifyKey; label: string }[] = [
  { key: "collect", label: "采集" },
  { key: "news", label: "新闻" },
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
          className={`ml-1.5 h-[5px] w-10 rounded-full ${dark ? "bg-pure-white/25" : "bg-pure-black/15"}`}
        />
      </div>
      {/* 窗口内容：左侧导航条 + 内容骨架 */}
      <div className={`flex h-11 ${dark ? "bg-[#262624]" : "bg-pure-white"}`}>
        <div
          className={`w-5 border-r ${dark ? "border-pure-white/10 bg-[#1c1c1a]" : "border-pure-black/5 bg-[#faf9f5]"}`}
        />
        <div className="flex flex-1 flex-col justify-center gap-[5px] px-1.5">
          <div
            className={`h-[5px] w-2/3 rounded-full ${dark ? "bg-pure-white/30" : "bg-pure-black/15"}`}
          />
          <div
            className={`h-[5px] w-full rounded-full ${dark ? "bg-pure-white/12" : "bg-pure-black/8"}`}
          />
          <div
            className={`h-[5px] w-1/2 rounded-full ${dark ? "bg-pure-white/12" : "bg-pure-black/8"}`}
          />
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
    <Button
      type="button"
      variant="outline"
      onClick={onSelect}
      aria-pressed={active}
      className={`relative h-auto w-[140px] flex-col items-stretch justify-start gap-0 rounded-lg p-1.5 text-left font-normal whitespace-normal hover:bg-transparent ${
        active
          ? "border-brand ring-brand/25 hover:border-brand ring-2"
          : "border-hairline hover:border-secondary/50"
      }`}
    >
      <ThemePreview dark={dark} />
      <span
        className={`text-caption mt-1.5 block px-0.5 ${
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
    </Button>
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
    notify_on_news: true,
    telegram_bot_token: "",
    telegram_chat_id: "",
    discord_bot_token: "",
    discord_channel_id: "",
  })
  const [editingServer, setEditingServer] = useState<McpServerPayload | null>(null)
  const [editingOriginalName, setEditingOriginalName] = useState<string | null>(null)
  const [showNewForm, setShowNewForm] = useState(false)
  const [mcpSearch, setMcpSearch] = useState("")
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

  const newServerTemplate = (type: McpTransportType = "http"): McpServerPayload => ({
    name: "",
    enabled: true,
    transport: emptyTransport(type),
  })

  const mcpNeedle = mcpSearch.trim().toLowerCase()
  const visibleServers = mcpNeedle
    ? servers.filter((s) =>
        `${s.name} ${s.transport.type} ${mcpTransportTarget(s.transport)}`
          .toLowerCase()
          .includes(mcpNeedle)
      )
    : servers

  const closeServerForm = () => {
    setShowNewForm(false)
    setEditingServer(null)
    setEditingOriginalName(null)
  }

  const openNewServer = () => {
    setEditingServer(newServerTemplate())
    setEditingOriginalName(null)
    setShowNewForm(true)
    setMcpSearch("")
  }

  const openTemplate = (type: McpTransportType) => {
    setEditingServer(newServerTemplate(type))
    setEditingOriginalName(null)
    setShowNewForm(true)
    setMcpSearch("")
  }

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
      // ponytail: 非敏感项从 localStorage 读回（由 setConfig 写入）。
      // 凭据类（bot_token / chat_id）不读——它们不该有本地副本。
      const keys = ["notify_channel", "notify_on_collect", "notify_on_news"] as const
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

  // 一次性清除旧版本在浏览器里留下的凭据副本（旧版 setConfig 什么都往
  // localStorage 写）。发现即删，不需要用户操作。
  useEffect(() => {
    if (isTauri) return
    try {
      const prefix = "openatrading-"
      Object.keys(localStorage)
        .filter((k) => k.startsWith(prefix) && SENSITIVE_KEY_RE.test(k.slice(prefix.length)))
        .forEach((k) => localStorage.removeItem(k))
    } catch {
      /* localStorage 被禁用时无需清理 */
    }
  }, [])

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

  const { label: pageTitle } = SETTINGS_TABS[activeTab]
  const isMcp = activeTab === "mcp"

  return (
    <div className="min-h-full">
      <div
        className={`mx-auto w-full px-4 pt-8 pb-16 sm:px-8 ${isMcp ? "max-w-5xl" : "max-w-3xl"}`}
      >
        <header className="mb-8 flex flex-wrap items-start justify-between gap-4">
          <div>
            <p className="text-tertiary text-caption font-medium tracking-wide">设置</p>
            <h1 className="text-h1 text-primary mt-1">{pageTitle}</h1>
          </div>
          {isMcp && (
            <div className="flex items-center gap-1.5">
              <Button
                variant="ghost"
                size="icon-sm"
                onClick={() => void mcpQuery.refetch()}
                title="刷新列表"
                aria-label="刷新列表"
              >
                <RefreshCw className={mcpQuery.isFetching ? "animate-spin" : undefined} />
              </Button>
              <Button size="sm" onClick={openNewServer}>
                <Plus className="size-3.5" />
                添加服务器
              </Button>
            </div>
          )}
        </header>

        <Dialog
          open={showNewForm}
          onOpenChange={(open) => {
            if (!open) closeServerForm()
          }}
        >
          <DialogContent className="max-h-[85vh] overflow-y-auto sm:max-w-lg">
            <DialogHeader className="pr-8">
              <DialogTitle>
                {editingOriginalName ? "编辑 MCP 服务器" : "添加 MCP 服务器"}
              </DialogTitle>
              <DialogDescription>
                {editingOriginalName
                  ? "修改配置后，对之后新建的会话生效。"
                  : "配置一个外部工具服务器，保存后对之后新建的会话生效。"}
              </DialogDescription>
            </DialogHeader>
            {editingServer && (
              <McpServerForm
                key={editingOriginalName ?? "new"}
                server={editingServer}
                onChange={setEditingServer}
                onCancel={closeServerForm}
                onSave={async () => {
                  await saveMcp.mutateAsync({
                    config: { ...editingServer, name: editingServer.name.trim() },
                    originalName: editingOriginalName,
                  })
                  closeServerForm()
                }}
                existingNames={servers
                  .filter((s) => s.name !== editingOriginalName)
                  .map((s) => s.name)}
              />
            )}
          </DialogContent>
        </Dialog>

        <div className="space-y-6">
          {activeTab === "appearance" && (
            <SettingsSection title="基础主题">
              <div className="flex flex-wrap items-start gap-3 py-3">
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
            </SettingsSection>
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

              <AgentApiKeySection defaultProvider={agentSettings.defaultProvider} />

              <SettingsSection title="数据源">
                <SettingRow
                  label="API 地址"
                  hint="数据服务与 Agent 网关地址"
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
                    hint="修改 Provider / Model 后重启生效"
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
            <div className="space-y-8">
              <section>
                <div className="mb-3 flex flex-wrap items-end justify-between gap-3">
                  <div className="flex items-baseline gap-2">
                    <h2 className="text-primary text-body font-semibold">已安装</h2>
                    <span className="text-tertiary font-number text-caption">
                      {mcpNeedle ? `${visibleServers.length} / ${servers.length}` : servers.length}
                    </span>
                  </div>
                  {servers.length > 0 && (
                    <InputGroup className="bg-surface h-8 w-full sm:w-64">
                      <InputGroupAddon>
                        <Search className="size-3.5" />
                      </InputGroupAddon>
                      <InputGroupInput
                        value={mcpSearch}
                        onChange={(e) => setMcpSearch(e.target.value)}
                        placeholder="搜索名称、地址或传输方式"
                        aria-label="搜索 MCP 服务器"
                      />
                    </InputGroup>
                  )}
                </div>

                {mcpQuery.isPending ? (
                  <div className="space-y-2.5">
                    {Array.from({ length: 2 }).map((_, i) => (
                      <Skeleton key={i} className="h-[76px] w-full rounded-xl" />
                    ))}
                  </div>
                ) : mcpQuery.isError ? (
                  <div className="bg-surface border-hairline flex flex-col items-center gap-2 rounded-lg border px-6 py-10 text-center">
                    <p className="text-secondary text-caption">无法加载 MCP 服务器列表</p>
                    <Button size="xs" variant="outline" onClick={() => mcpQuery.refetch()}>
                      重试
                    </Button>
                  </div>
                ) : servers.length === 0 ? (
                  <div className="bg-surface border-hairline flex flex-col items-center rounded-lg border px-6 py-12 text-center">
                    <span className="bg-hover text-tertiary grid size-10 place-items-center rounded-full">
                      <Server className="size-5" />
                    </span>
                    <h3 className="text-primary text-body mt-3 font-medium">还没有 MCP 服务器</h3>
                    <p className="text-secondary text-caption mt-1 max-w-sm">
                      连接外部工具后，Agent 可以在会话中调用它们。
                    </p>
                    <Button size="sm" className="mt-4" onClick={openNewServer}>
                      <Plus className="size-3.5" />
                      添加服务器
                    </Button>
                  </div>
                ) : visibleServers.length === 0 ? (
                  <div className="bg-surface border-hairline flex flex-col items-center gap-2 rounded-lg border px-6 py-10 text-center">
                    <p className="text-secondary text-caption">没有匹配的服务器</p>
                    <Button size="xs" variant="ghost" onClick={() => setMcpSearch("")}>
                      清除搜索
                    </Button>
                  </div>
                ) : (
                  <div className="space-y-2.5">
                    {visibleServers.map((s) => (
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
                    ))}
                  </div>
                )}
              </section>

              {(servers.length === 0 || mcpSearch.trim() === "") && (
                <section>
                  <div className="mb-3 flex flex-wrap items-end justify-between gap-3">
                    <h2 className="text-primary text-body font-semibold">快速开始</h2>
                    <p className="text-tertiary text-caption">选择一种传输方式，填入命令或 URL</p>
                  </div>
                  <McpTemplateGrid onUse={openTemplate} />
                </section>
              )}

              <div className="bg-surface border-hairline text-tertiary text-caption flex items-start gap-2 rounded-lg border px-3.5 py-3">
                <Info className="mt-0.5 size-3.5 shrink-0" />
                <p className="leading-5">
                  新增与修改写入 pi-rs 的全局{" "}
                  <code className="text-secondary font-number">$PI_RS_HOME/agent/mcp.json</code>
                  ，对之后新建的会话生效；项目级{" "}
                  <code className="text-secondary font-number">{"{cwd}"}/.pi-rs/mcp.json</code>{" "}
                  的同名条目会覆盖全局，需直接编辑该文件。
                </p>
              </div>
            </div>
          )}

          {activeTab === "about" && (
            <SettingsSection title="关于">
              {aboutRows.map(([label, value]) => (
                <div key={label} className="flex items-center justify-between py-3">
                  <span className="text-secondary text-body">{label}</span>
                  <span className="text-primary font-number text-caption">{value}</span>
                </div>
              ))}
            </SettingsSection>
          )}
        </div>
      </div>
    </div>
  )
}
