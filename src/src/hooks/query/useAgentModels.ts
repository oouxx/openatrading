import { useQuery } from "@tanstack/react-query"
import { fetchJson } from "@/stores/data"
import { agentModelsKeys } from "./queryKeys"

export interface ModelOption {
  id: string
  name: string
  reasoning: boolean
  contextWindow: number
  maxTokens: number
  /** 该模型支持的思考级别（pi-rs `get_supported_thinking_levels`），非推理模型只有 off */
  thinkingLevels: string[]
  available: boolean
}

export interface ProviderOption {
  name: string
  models: ModelOption[]
}

/**
 * pi-rs 可选模型列表（`/api/agent/models`）。
 *
 * 设置页的模型选择器与思考级别下拉共用同一份缓存：key 取自 queryKeys，
 * 保存 API Key 后按同一 key 失效即可让两处同时刷新。
 */
export function useAgentModels() {
  return useQuery({
    queryKey: agentModelsKeys.all,
    queryFn: () => fetchJson<{ providers: ProviderOption[] }>("/api/agent/models"),
    staleTime: 5 * 60 * 1000,
    retry: 1,
  })
}
