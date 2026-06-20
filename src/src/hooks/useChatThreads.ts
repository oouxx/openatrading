import { useState, useEffect, useCallback } from "react";
import { fetchJson } from "@/stores/data";

export interface ChatThread {
  id: number;
  title: string;
  created_at: string;
  updated_at: string;
}

export interface ChatMessage {
  id: number;
  thread_id: number;
  role: string;
  content: string;
  created_at: string;
}

export function useChatThreads() {
  const [threads, setThreads] = useState<ChatThread[]>([]);
  const [loading, setLoading] = useState(false);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const data = await fetchJson<ChatThread[]>("/api/ai/threads");
      setThreads(data);
    } catch {
      // silently fail
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const createThread = useCallback(async (title?: string): Promise<ChatThread | null> => {
    try {
      const thread = await fetchJson<ChatThread>("/api/ai/threads", {
        method: "POST",
        body: JSON.stringify({ title: title || "新对话" }),
      });
      setThreads((prev) => [thread, ...prev]);
      return thread;
    } catch {
      return null;
    }
  }, []);

  const renameThread = useCallback(async (id: number, title: string) => {
    try {
      const updated = await fetchJson<ChatThread>(`/api/ai/threads/${id}`, {
        method: "PUT",
        body: JSON.stringify({ title }),
      });
      setThreads((prev) => prev.map((t) => (t.id === id ? updated : t)));
    } catch {
      // silently fail
    }
  }, []);

  const deleteThread = useCallback(async (id: number) => {
    try {
      await fetchJson(`/api/ai/threads/${id}`, { method: "DELETE" });
      setThreads((prev) => prev.filter((t) => t.id !== id));
    } catch {
      // silently fail
    }
  }, []);

  const getMessages = useCallback(async (threadId: number): Promise<ChatMessage[]> => {
    try {
      return await fetchJson<ChatMessage[]>(`/api/ai/threads/${threadId}/messages`);
    } catch {
      return [];
    }
  }, []);

  const saveMessage = useCallback(async (
    threadId: number,
    role: string,
    content: string,
  ): Promise<ChatMessage | null> => {
    try {
      return await fetchJson<ChatMessage>(`/api/ai/threads/${threadId}/messages`, {
        method: "POST",
        body: JSON.stringify({ role, content }),
      });
    } catch {
      return null;
    }
  }, []);

  return {
    threads,
    loading,
    refresh,
    createThread,
    renameThread,
    deleteThread,
    getMessages,
    saveMessage,
  };
}
