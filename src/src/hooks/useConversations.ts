import { useState, useCallback, useEffect } from "react";
import { fetchJson } from "@/stores/data";

export interface Conversation {
  id: string;
  title: string;
  skillName?: string;
  createdAt: string;
  updatedAt: string;
}

interface ThreadResponse {
  id: number;
  title: string;
  created_at: string;
  updated_at: string;
}

export function useConversations() {
  const [conversations, setConversations] = useState<Conversation[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [search, setSearch] = useState("");

  const load = useCallback(async () => {
    try {
      const threads = await fetchJson<ThreadResponse[]>("/api/ai/threads");
      const mapped: Conversation[] = threads.map((t) => ({
        id: String(t.id),
        title: t.title,
        createdAt: t.created_at,
        updatedAt: t.updated_at,
      }));
      setConversations(mapped);
    } catch {
      // silently fail
    }
  }, []);

  useEffect(() => { load(); }, [load]);

  const selectConversation = useCallback((id: string) => {
    setSelectedId(id);
  }, []);

  const createConversation = useCallback(async (title?: string) => {
    try {
      const thread = await fetchJson<ThreadResponse>("/api/ai/threads", {
        method: "POST",
        body: JSON.stringify({ title: title || "新对话" }),
      });
      const conv: Conversation = {
        id: String(thread.id),
        title: thread.title,
        createdAt: thread.created_at,
        updatedAt: thread.updated_at,
      };
      setConversations((prev) => [conv, ...prev]);
      setSelectedId(conv.id);
      // Notify assistant-ui to switch to the new thread
      window.dispatchEvent(
        new CustomEvent("select-thread", { detail: conv.id })
      );
      return conv;
    } catch {
      return null;
    }
  }, []);

  return {
    conversations,
    selectedId,
    selectConversation,
    createConversation,
    search,
    setSearch,
  };
}
