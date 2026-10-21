import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import { fetchJson } from "@/stores/data"
import type { SkillEntry, SkillPayload } from "@/types/skill"
import { skillKeys } from "./queryKeys"

/** 把后端错误转换成用户可读文案。 */
export function skillErrorMessage(raw: string): string {
  if (raw === "404" || raw.includes("not found")) return "技能不存在或已被删除"
  if (raw === "500" || /^5\d{2}$/.test(raw)) return "保存失败，请检查服务端日志后重试"
  if (raw.includes("name must not be empty")) return "技能名称不能为空"
  if (raw.includes("path separators")) return "技能名称不能包含路径分隔符或 `..`"
  if (/failed to save skill/i.test(raw)) return "保存失败，请检查服务端日志后重试"
  if (/^\d{3}$/.test(raw)) return `请求失败（HTTP ${raw}）`
  return raw || "未知错误"
}

export function useSkills() {
  return useQuery({
    queryKey: skillKeys.all,
    queryFn: () => fetchJson<SkillEntry[]>("/api/skills"),
    staleTime: 30_000,
  })
}

/** 新建与更新共用同一个接口，成功后把返回的技能写回列表缓存。 */
export function useSaveSkill() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (payload: SkillPayload) =>
      fetchJson<SkillEntry | null>("/api/skills", {
        method: "POST",
        body: JSON.stringify({
          name: payload.name,
          description: payload.description,
          content: payload.content,
          "disable-model-invocation": payload.disableModelInvocation,
        }),
      }),
    onSuccess: (saved) => {
      if (!saved) return
      queryClient.setQueryData<SkillEntry[]>(skillKeys.all, (old) => {
        if (!old) return [saved]
        const exists = old.some((skill) => skill.name === saved.name)
        return exists
          ? old.map((skill) => (skill.name === saved.name ? saved : skill))
          : [...old, saved]
      })
    },
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: skillKeys.all })
    },
  })
}

export function useDeleteSkill() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (name: string) =>
      fetchJson(`/api/skills/${encodeURIComponent(name)}`, { method: "DELETE" }),
    onSuccess: (_data, name) => {
      queryClient.setQueryData<SkillEntry[]>(skillKeys.all, (old) =>
        old?.filter((skill) => skill.name !== name)
      )
    },
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: skillKeys.all })
    },
  })
}
