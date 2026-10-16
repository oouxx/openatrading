import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import { fetchJson } from "@/stores/data"
import { agentAuthKeys, agentModelsKeys } from "./queryKeys"

/** 单个 provider 的凭据状态（后端从不回传密钥原文）。 */
export interface ProviderAuth {
  provider: string
  displayName: string
  configured: boolean
  /** `"stored"`（auth.json）/ `"environment"` / `"runtime"` / `"fallback"`；未配置为 null */
  source: string | null
  /** 来源标识（环境变量名等），不是密钥值 */
  label: string | null
}

export function useAgentAuth() {
  return useQuery({
    queryKey: agentAuthKeys.all,
    queryFn: () =>
      fetchJson<{ providers: ProviderAuth[] }>("/api/agent/auth").then((d) => d.providers),
    staleTime: 30_000,
  })
}

/**
 * 密钥状态一变，可选模型也跟着变（`available` 由 provider 是否已配置决定），
 * 所以两个 mutation 都要连带失效模型列表，否则 ModelPicker 仍显示旧的置灰状态。
 */
function useInvalidateAuth() {
  const queryClient = useQueryClient()
  return () => {
    queryClient.invalidateQueries({ queryKey: agentAuthKeys.all })
    queryClient.invalidateQueries({ queryKey: agentModelsKeys.all })
  }
}

export interface SetApiKeyVars {
  provider: string
  key: string
}

/** 写入 provider 的 API Key（后端落到 pi-rs `auth.json`）。 */
export function useSetApiKey() {
  const invalidate = useInvalidateAuth()
  return useMutation({
    mutationFn: ({ provider, key }: SetApiKeyVars) =>
      fetchJson("/api/agent/auth", {
        method: "POST",
        body: JSON.stringify({ provider, key }),
      }),
    onSettled: invalidate,
  })
}

/** 删除 provider 已保存的 API Key。 */
export function useDeleteApiKey() {
  const invalidate = useInvalidateAuth()
  return useMutation({
    mutationFn: (provider: string) =>
      fetchJson(`/api/agent/auth/${encodeURIComponent(provider)}`, { method: "DELETE" }),
    onSettled: invalidate,
  })
}
