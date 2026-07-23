import { useCallback, useEffect, useState, startTransition } from "react"
import { fetchJson } from "@/stores/data"

export interface ChatSession {
  id: string
  name: string
  created: string
  modified: string
  message_count: number
  path: string
  cwd: string
  first_message: string | null
  parent_session_path: string | null
}

export function useThreads() {
  const [sessions, setSessions] = useState<ChatSession[]>([])
  const [loading, setLoading] = useState(true)

  const reload = useCallback(async () => {
    try {
      const data = await fetchJson<ChatSession[]>("/api/ai/sessions")
      setSessions(data || [])
    } catch {
      setSessions([])
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    startTransition(() => reload())
  }, [reload])

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
