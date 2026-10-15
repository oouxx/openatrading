import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import { fetchJson } from "@/stores/data"
import type { McpServerConfig, McpServerPayload } from "@/types/mcp"
import { mcpKeys } from "./queryKeys"

/** 后端错误 → 用户可读文案（web 端只带状态码，Tauri 端带原始字符串） */
export function mcpErrorMessage(raw: string): string {
  if (raw === "409" || raw === "MCP server already exists") return "已存在同名服务器"
  if (raw === "404" || raw === "MCP server not found") return "服务器不存在或已被删除"
  if (raw === "403" || raw.includes("project config")) return "该服务器定义在项目配置文件中，请直接编辑该文件"
  if (raw === "400" || raw.includes("invalid MCP server config")) return "配置不完整：请检查名称与命令/URL"
  if (/^\d{3}$/.test(raw)) return `操作失败 (${raw})`
  return raw
}

export function useMcpServers() {
  return useQuery({
    queryKey: mcpKeys.all,
    queryFn: () => fetchJson<McpServerConfig[]>("/api/mcp/servers"),
    staleTime: 30_000,
  })
}

/** 启用/停用切换：乐观更新，失败回滚 */
export function useToggleMcpServer() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (name: string) => fetchJson(`/api/mcp/servers/${encodeURIComponent(name)}/toggle`),
    onMutate: async (name) => {
      await queryClient.cancelQueries({ queryKey: mcpKeys.all })
      const prev = queryClient.getQueryData<McpServerConfig[]>(mcpKeys.all)
      queryClient.setQueryData<McpServerConfig[]>(mcpKeys.all, (old) =>
        old?.map((s) => (s.name === name ? { ...s, enabled: !s.enabled } : s))
      )
      return { prev }
    },
    onError: (_err, _name, ctx) => {
      if (ctx?.prev) queryClient.setQueryData(mcpKeys.all, ctx.prev)
    },
    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: mcpKeys.all })
    },
  })
}

export function useDeleteMcpServer() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (name: string) =>
      fetchJson(`/api/mcp/servers/${encodeURIComponent(name)}`, { method: "DELETE" }),
    onSuccess: (_data, name) => {
      queryClient.setQueryData<McpServerConfig[]>(mcpKeys.all, (old) =>
        old?.filter((s) => s.name !== name)
      )
    },
    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: mcpKeys.all })
    },
  })
}

export interface SaveMcpServerVars {
  config: McpServerPayload
  /** 非空表示编辑（PUT 到原名称路径），为空表示新建（POST） */
  originalName: string | null
}

/** 新建 / 编辑统一走一个 mutation，成功后在 onSettled 里刷新列表 */
export function useSaveMcpServer() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: ({ config, originalName }: SaveMcpServerVars) => {
      if (originalName !== null) {
        return fetchJson(`/api/mcp/servers/${encodeURIComponent(originalName)}`, {
          method: "PUT",
          body: JSON.stringify(config),
        })
      }
      return fetchJson("/api/mcp/servers", { method: "POST", body: JSON.stringify(config) })
    },
    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: mcpKeys.all })
    },
  })
}
