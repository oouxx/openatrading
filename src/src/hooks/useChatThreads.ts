import { useCallback } from "react";
import type {
  RemoteThreadListAdapter,
  RemoteThreadListResponse,
  RemoteThreadMetadata,
  RemoteThreadInitializeResponse,
} from "@assistant-ui/react";
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

function toMetadata(t: ChatThread): RemoteThreadMetadata {
  return {
    status: "regular",
    remoteId: String(t.id),
    title: t.title,
    lastMessageAt: new Date(t.updated_at),
  };
}

export function createThreadListAdapter(): RemoteThreadListAdapter {
  return {
    async list(): Promise<RemoteThreadListResponse> {
      const threads = await fetchJson<ChatThread[]>("/api/ai/threads");
      return { threads: threads.map(toMetadata) };
    },

    async rename(remoteId: string, newTitle: string): Promise<void> {
      await fetchJson(`/api/ai/threads/${remoteId}`, {
        method: "PUT",
        body: JSON.stringify({ title: newTitle }),
      });
    },

    async archive(_remoteId: string): Promise<void> {
      // not implemented
    },

    async unarchive(_remoteId: string): Promise<void> {
      // not implemented
    },

    async delete(remoteId: string): Promise<void> {
      await fetchJson(`/api/ai/threads/${remoteId}`, { method: "DELETE" });
    },

    async initialize(_threadId: string): Promise<RemoteThreadInitializeResponse> {
      const thread = await fetchJson<ChatThread>("/api/ai/threads", {
        method: "POST",
        body: JSON.stringify({ title: "新对话" }),
      });
      return { remoteId: String(thread.id), externalId: String(thread.id) };
    },

    async generateTitle(): Promise<never> {
      throw new Error("not implemented");
    },

    async fetch(remoteId: string): Promise<RemoteThreadMetadata> {
      const threads = await fetchJson<ChatThread[]>("/api/ai/threads");
      const t = threads.find((t) => String(t.id) === remoteId);
      if (!t) throw new Error("thread not found");
      return toMetadata(t);
    },
  };
}

// Standalone helpers used by AiCopilot outside the hook

export async function getMessages(threadId: number): Promise<ChatMessage[]> {
  try {
    return await fetchJson<ChatMessage[]>(`/api/ai/threads/${threadId}/messages`);
  } catch {
    return [];
  }
}

export async function saveMessage(
  threadId: number,
  role: string,
  content: string,
): Promise<void> {
  try {
    await fetchJson(`/api/ai/threads/${threadId}/messages`, {
      method: "POST",
      body: JSON.stringify({ role, content }),
    });
  } catch {
    // silently fail
  }
}
