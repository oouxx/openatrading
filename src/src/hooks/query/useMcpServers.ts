import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import { fetchJson } from "@/stores/data"
import type { McpServerListResponse, McpServerPayload, McpServerTestResult } from "@/types/mcp"
import { mcpKeys } from "./queryKeys"

/** 后端错误 → 用户可读文案（web 端只带状态码，Tauri 端带原始字符串） */
export function mcpErrorMessage(raw: string): string {
  if (raw === "409" || raw === "MCP server already exists") return "已存在同名服务器"
  if (raw === "404" || raw === "MCP server not found") return "服务器不存在或已被删除"
  if (raw === "403" || raw.includes("project config"))
    return "该服务器由项目配置定义或覆盖，请直接编辑该文件"
  if (raw === "400" || raw.includes("invalid MCP server config"))
    return "配置不合法：名称仅支持字母、数字、下划线、连字符，并检查命令/URL"
  if (/^5\d{2}$/.test(raw)) return "服务端处理失败，请查看 gateway 日志后重试"
  if (/^\d{3}$/.test(raw)) return `请求被拒绝（HTTP ${raw}）`
  return raw
}

export function useMcpServers() {
  return useQuery({
    queryKey: mcpKeys.all,
    queryFn: () => fetchJson<McpServerListResponse>("/api/mcp/servers"),
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
      const prev = queryClient.getQueryData<McpServerListResponse>(mcpKeys.all)
      queryClient.setQueryData<McpServerListResponse>(mcpKeys.all, (old) =>
        old
          ? {
              ...old,
              servers: old.servers.map((s) =>
                s.name === name ? { ...s, enabled: !s.enabled } : s
              ),
            }
          : old
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
      queryClient.setQueryData<McpServerListResponse>(mcpKeys.all, (old) =>
        old ? { ...old, servers: old.servers.filter((s) => s.name !== name) } : old
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

/** 对某个已保存条目做一次连接测试（不修改配置）。 */
export function useTestMcpServer() {
  return useMutation({
    mutationFn: (name: string) =>
      fetchJson<McpServerTestResult>(`/api/mcp/servers/${encodeURIComponent(name)}/test`, {
        method: "POST",
      }),
  })
}
