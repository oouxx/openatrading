import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import { fetchJson } from "@/stores/data"
import { extensionKeys } from "./queryKeys"

/** 一个可配置的 pi-rs 内置扩展。 */
export interface BuiltinExtension {
  id: string
  name: string
  description: string
  enabled: boolean
  tools: string[]
  commands: string[]
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
