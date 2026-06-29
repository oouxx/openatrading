import { useCallback, useEffect, useState, startTransition } from "react"
import { fetchJson } from "@/stores/data"

export interface ChatThread {
  id: number
  title: string
  created_at: string
  updated_at: string
}

export interface ChatMessage {
  id: number
  thread_id: number
  role: string
  content: string
  created_at: string
}

export function useThreads() {
  const [threads, setThreads] = useState<ChatThread[]>([])
  const [loading, setLoading] = useState(true)

  const reload = useCallback(async () => {
    try {
      const data = await fetchJson<ChatThread[]>("/api/ai/threads")
      setThreads(data)
    } catch {
      setThreads([])
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    startTransition(() => reload())
  }, [reload])

  const createThread = useCallback(async (title = "新对话") => {
    const thread = await fetchJson<ChatThread>("/api/ai/threads", {
      method: "POST",
      body: JSON.stringify({ title }),
    })
    setThreads((prev) => [thread, ...prev])
    return thread
  }, [])

  const renameThread = useCallback(async (id: number, title: string) => {
    await fetchJson(`/api/ai/threads/${id}`, {
      method: "PUT",
      body: JSON.stringify({ title }),
    })
    setThreads((prev) => prev.map((t) => (t.id === id ? { ...t, title } : t)))
  }, [])

  const deleteThread = useCallback(async (id: number) => {
    await fetchJson(`/api/ai/threads/${id}`, { method: "DELETE" })
    setThreads((prev) => prev.filter((t) => t.id !== id))
  }, [])

  return { threads, loading, reload, createThread, renameThread, deleteThread }
}

export async function getMessages(threadId: number): Promise<ChatMessage[]> {
  try {
    return await fetchJson<ChatMessage[]>(`/api/ai/threads/${threadId}/messages`)
  } catch {
    return []
  }
}

export async function saveMessage(threadId: number, role: string, content: string): Promise<void> {
  try {
    await fetchJson(`/api/ai/threads/${threadId}/messages`, {
      method: "POST",
      body: JSON.stringify({ role, content }),
    })
  } catch {
    // silently fail
  }
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
