import { useCallback, useEffect, useState, startTransition } from "react"
import { fetchJson } from "@/stores/data"

export interface ChatSession {
  id: string
  name: string
  created: string
  modified: string
  message_count: number
  status: string
  path: string
  cwd: string
  first_message: string | null
  parent_session_path: string | null
}

/**
 * 会话列表。`enabled: false` 时不拉取（如自动化弹窗只在选了「复用会话」后才需要）。
 */
export function useThreads(options: { enabled?: boolean } = {}) {
  const { enabled = true } = options
  const [sessions, setSessions] = useState<ChatSession[]>([])
  const [loading, setLoading] = useState(true)

  const reload = useCallback(async () => {
    if (!enabled) return
    try {
      const data = await fetchJson<ChatSession[]>("/api/ai/sessions")
      setSessions(data || [])
    } catch {
      setSessions([])
    } finally {
      setLoading(false)
    }
  }, [enabled])

  useEffect(() => {
    if (!enabled) return
    startTransition(() => reload())
  }, [reload, enabled])

  const deleteSession = useCallback(async (id: string) => {
    await fetchJson(`/api/ai/sessions/${id}`, { method: "DELETE" })
    setSessions((prev) => prev.filter((s) => s.id !== id))
  }, [])

  return { sessions, loading, reload, deleteSession }
}

export async function fetchCommands() {
  try {
    return await fetchJson<{ name: string; description: string; command_type: string }[]>(
      "/api/ai/commands"
    )
  } catch {
    return []
  }
}
