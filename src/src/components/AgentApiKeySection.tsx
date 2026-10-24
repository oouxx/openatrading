import { useState } from "react"
import { Input } from "@/components/ui/input"
import { Button } from "@/components/ui/button"
import { Skeleton } from "@/components/ui/skeleton"
import {
  useAgentAuth,
  useDeleteApiKey,
  useSetApiKey,
  type ProviderAuth,
} from "@/hooks/query/useAgentAuth"
import { SettingRow, SettingsSection, SelectField } from "./settings-ui"

/** 与设置页其它输入框一致的样式 */
const inputCls = "w-64 bg-surface font-number text-caption"

/** 凭据来源 → 说明文案。`label` 是来源标识（环境变量名），不是密钥值。 */
function sourceHint(auth: ProviderAuth | undefined): string {
  switch (auth?.source) {
    case "stored":
      return "已保存到 auth.json"
    case "environment":
      return auth.label ? `来自环境变量 ${auth.label}` : "来自环境变量"
    case "runtime":
      return "来自启动参数"
    case "fallback":
      return "来自扩展配置"
    default:
      return "未配置密钥"
  }
}

/**
 * 设置页「API Key」区块：选 provider、填 key、保存即写入 pi-rs `auth.json`。
 *
 * 密钥是只写不读的——后端从不回传原文，输入框始终为空，placeholder 只提示
 * 「已配置」。保存成功后清空输入框，避免密钥留在 DOM 里。
 */
export function AgentApiKeySection({ defaultProvider }: { defaultProvider: string }) {
  const authQuery = useAgentAuth()
  const setApiKey = useSetApiKey()
  const deleteApiKey = useDeleteApiKey()

  // 用户手动选过的 provider；为空时跟随「默认模型」的 provider
  const [picked, setPicked] = useState("")
  const [key, setKey] = useState("")
  const [error, setError] = useState<string | null>(null)

  const providers = authQuery.data ?? []
  const fallbackProvider = providers.some((p) => p.provider === defaultProvider)
    ? defaultProvider
    : (providers[0]?.provider ?? "")
  const active = picked || fallbackProvider
  const current = providers.find((p) => p.provider === active)
  // 只有存在 auth.json 里的凭据能删；环境变量/扩展提供的配置不归这里管
  const canDelete = current?.source === "stored"

  const selectProvider = (provider: string) => {
    setPicked(provider)
    setKey("")
    setError(null)
  }

  const handleSave = async () => {
    setError(null)
    try {
      await setApiKey.mutateAsync({ provider: active, key })
      setKey("")
    } catch (e) {
      setError(e instanceof Error ? e.message : "保存失败")
    }
  }

  const handleDelete = async () => {
    setError(null)
    try {
      await deleteApiKey.mutateAsync(active)
    } catch (e) {
      setError(e instanceof Error ? e.message : "删除失败")
    }
  }

  if (authQuery.isPending) {
    return (
      <SettingsSection title="API Key">
        <SettingRow label="Provider" control={<Skeleton className="h-8 w-[200px] rounded-lg" />} />
      </SettingsSection>
    )
  }

  if (authQuery.isError) {
    return (
      <SettingsSection title="API Key">
        <SettingRow
          label="Provider"
          hint="无法读取凭据状态"
          control={
            <Button size="default" variant="outline" onClick={() => authQuery.refetch()}>
              重试
            </Button>
          }
        />
      </SettingsSection>
    )
  }

  return (
    <SettingsSection title="API Key">
      <SettingRow
        label="Provider"
        hint={sourceHint(current)}
        control={
          <SelectField
            value={active}
            onChange={selectProvider}
            options={providers.map((p) => ({ value: p.provider, label: p.displayName }))}
            className="w-[200px]"
          />
        }
      />
      <SettingRow
        label="API Key"
        hint="保存到 pi-rs auth.json，立即对模型列表生效"
        control={
          <div className="flex flex-col items-end gap-1">
            <div className="flex items-center gap-2">
              <Input
                type="password"
                value={key}
                autoComplete="off"
                spellCheck={false}
                onChange={(e) => setKey(e.target.value)}
                placeholder={current?.configured ? "已配置，输入新值可覆盖" : "粘贴 API Key"}
                className={inputCls}
              />
              <Button
                size="default"
                disabled={!active || !key.trim() || setApiKey.isPending}
                onClick={handleSave}
              >
                保存
              </Button>
              {canDelete && (
                <Button
                  size="default"
                  variant="ghost"
                  disabled={deleteApiKey.isPending}
                  onClick={handleDelete}
                >
                  删除
                </Button>
              )}
            </div>
            {error && <p className="text-danger text-caption">{error}</p>}
          </div>
        }
      />
    </SettingsSection>
  )
}
