// src/hooks/useConversations.ts
import { useState, useCallback } from "react";

export interface Conversation {
  id: string;
  title: string;
  skillName: string;
  createdAt: number;
  updatedAt: number;
}

export function useConversations() {
  const [conversations] = useState<Conversation[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [search, setSearch] = useState("");

  const selectConversation = useCallback((id: string) => {
    setSelectedId(id);
  }, []);

  return { conversations, selectedId, selectConversation, search, setSearch };
}
