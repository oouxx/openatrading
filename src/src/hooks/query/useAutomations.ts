import { keepPreviousData, useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import { fetchJson } from "@/stores/data"
import type {
  Automation,
  AutomationPatchBody,
  AutomationRun,
  NewAutomationBody,
  NextRuns,
  RunQuery,
} from "@/types/automation"

export const automationKeys = {
  all: ["automations"] as const,
  runs: (params?: RunQuery) => ["automations", "runs", params ?? {}] as const,
}

function toSearchParams(query: RunQuery): string {
  const params = new URLSearchParams()
  if (query.automation_id) params.set("automation_id", query.automation_id)
  if (query.unread) params.set("unread", "true")
  if (query.active) params.set("active", "true")
  if (query.limit) params.set("limit", String(query.limit))
  const s = params.toString()
  return s ? `?${s}` : ""
}

/** 全部自动化任务 */
export interface AutomationsResponse {
  automations: Automation[]
  next_runs: NextRuns
}

export function useAutomations() {
  return useQuery({
    queryKey: automationKeys.all,
    queryFn: () => fetchJson<AutomationsResponse>("/api/automations"),
    staleTime: 10_000,
  })
}

/** 运行记录(收件箱) */
export function useAutomationRuns(query: RunQuery = { limit: 100 }) {
  return useQuery({
    queryKey: automationKeys.runs(query),
    queryFn: () =>
      fetchJson<{ runs: AutomationRun[] }>(`/api/automations/runs${toSearchParams(query)}`),
    select: (d) => d.runs,
    staleTime: 5_000,
    // 切换「未读 / 未归档」筛选时保留上一份列表，否则每次都会闪一遍骨架屏
    placeholderData: keepPreviousData,
  })
}

function useInvalidateAll() {
  const queryClient = useQueryClient()
  return () => {
    void queryClient.invalidateQueries({ queryKey: automationKeys.all })
  }
}

export function useCreateAutomation() {
  const invalidate = useInvalidateAll()
  return useMutation({
    mutationFn: (body: NewAutomationBody) =>
      fetchJson<Automation>("/api/automations", {
        method: "POST",
        body: JSON.stringify(body),
      }),
    onSettled: invalidate,
  })
}

export function useUpdateAutomation() {
  const invalidate = useInvalidateAll()
  return useMutation({
    mutationFn: ({ id, patch }: { id: string; patch: AutomationPatchBody }) =>
      fetchJson<Automation>(`/api/automations/${encodeURIComponent(id)}`, {
        method: "PUT",
        body: JSON.stringify(patch),
      }),
    onSettled: invalidate,
  })
}

export function useDeleteAutomation() {
  const invalidate = useInvalidateAll()
  return useMutation({
    mutationFn: (id: string) =>
      fetchJson(`/api/automations/${encodeURIComponent(id)}`, { method: "DELETE" }),
    onSettled: invalidate,
  })
}

export function useTriggerAutomation() {
  const invalidate = useInvalidateAll()
  return useMutation({
    mutationFn: (id: string) =>
      fetchJson(`/api/automations/${encodeURIComponent(id)}/trigger`, { method: "POST" }),
    onSettled: invalidate,
  })
}

export function useMarkRunRead() {
  const invalidate = useInvalidateAll()
  return useMutation({
    mutationFn: (runId: number) =>
      fetchJson(`/api/automations/runs/${runId}/read`, { method: "POST" }),
    onSettled: invalidate,
  })
}

export function useArchiveRun() {
  const invalidate = useInvalidateAll()
  return useMutation({
    mutationFn: ({ runId, archived }: { runId: number; archived: boolean }) =>
      fetchJson(`/api/automations/runs/${runId}/archive`, {
        method: "POST",
        body: JSON.stringify({ archived }),
      }),
    onSettled: invalidate,
  })
}
