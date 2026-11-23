import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import { fetchJson } from "@/stores/data"
import { extensionKeys } from "./queryKeys"

/** 下拉型设置的候选值（第一个是 pi-rs 的默认档）。 */
export interface ExtensionSettingOption {
  value: string
  label: string
}

/**
 * 扩展在 pi-rs `settings.json` 里读取的一个配置项。
 *
 * `key` 是 settings.json 路径（`webSearch.provider`），保存时原样回传；
 * `options` 非空表示下拉框，空表示自由文本。
 */
export interface ExtensionSetting {
  key: string
  label: string
  hint: string | null
  placeholder: string | null
  /** 生效值；空串 = 未配置，pi-rs 回落到环境变量/默认 */
  value: string
  options: ExtensionSettingOption[]
}

/** 一个可配置的 pi-rs 内置扩展。 */
export interface BuiltinExtension {
  id: string
  name: string
  description: string
  enabled: boolean
  tools: string[]
  commands: string[]
  settings: ExtensionSetting[]
  /** 设置面板脚注：凭据来源、生效时机（pi-rs 侧决定，前端不推断）。 */
  settingsNote: string | null
}

/** 后端错误 → 用户可读文案（web 端只带状态码，Tauri 端带 `{ "error": ... }` 文本） */
export function extensionErrorMessage(raw: string): string {
  if (raw === "404" || raw.includes("未知扩展")) return "该扩展不存在，刷新页面后重试"
  if (/^5\d{2}$/.test(raw)) return "保存失败，请查看 gateway 日志后重试"
  if (/^\d{3}$/.test(raw)) return `请求被拒绝（HTTP ${raw}）`
  return raw
}

export function useExtensions() {
  return useQuery({
    queryKey: extensionKeys.all,
    queryFn: () => fetchJson<BuiltinExtension[]>("/api/extensions"),
    staleTime: 30_000,
  })
}

interface ToggleExtensionVars {
  id: string
  enabled: boolean
}

/**
 * 切换内置扩展。后端会持久化到 settings.json 并立即同步所有活跃会话，
 * 因此这里只需要乐观更新列表，不需要重启前端或网关。
 */
export function useToggleExtension() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: ({ id, enabled }: ToggleExtensionVars) =>
      fetchJson<BuiltinExtension[]>(`/api/extensions/${encodeURIComponent(id)}`, {
        method: "PUT",
        body: JSON.stringify({ enabled }),
      }),
    onMutate: async ({ id, enabled }) => {
      await queryClient.cancelQueries({ queryKey: extensionKeys.all })
      const prev = queryClient.getQueryData<BuiltinExtension[]>(extensionKeys.all)
      queryClient.setQueryData<BuiltinExtension[]>(extensionKeys.all, (old) =>
        old?.map((ext) => (ext.id === id ? { ...ext, enabled } : ext))
      )
      return { prev }
    },
    onError: (_err, _vars, ctx) => {
      if (ctx?.prev) queryClient.setQueryData(extensionKeys.all, ctx.prev)
    },
    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: extensionKeys.all })
    },
  })
}

interface UpdateExtensionSettingVars {
  id: string
  /** settings.json 路径，如 `webSearch.provider` */
  key: string
  /** 空串 = 清除该键，回落到 pi-rs 的环境变量/默认值 */
  value: string
}

/**
 * 写入一个扩展设置项（pi-rs `settings.json`）。
 *
 * 后端把解析结果（生效值）整份回传，所以直接替换缓存即可：不乐观更新是
 * 有意的——文本输入框由组件本地状态承载，避免用户打字被服务端值打断。
 */
export function useUpdateExtensionSetting() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: ({ id, key, value }: UpdateExtensionSettingVars) =>
      fetchJson<BuiltinExtension[]>(`/api/extensions/${encodeURIComponent(id)}/settings`, {
        method: "PUT",
        body: JSON.stringify({ key, value }),
      }),
    onSuccess: (items) => {
      queryClient.setQueryData(extensionKeys.all, items)
    },
  })
}
