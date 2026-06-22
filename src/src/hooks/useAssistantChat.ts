import { useState, useCallback, useRef, useEffect, useMemo } from "react";
import { useExternalStoreRuntime, unstable_useSlashCommandAdapter } from "@assistant-ui/react";
import { apiChatStream, fetchJson } from "@/stores/data";
import type { ChatMessage as StoredMessage } from "./useChatThreads";

export interface CommandSpec {
  name: string;
  description: string;
  command_type: "tool" | "skill" | "pipeline";
}

interface ChatMessage {
  id: string;
  role: "user" | "assistant";
  content: string;
  reasoning?: string;
}

let nextMsgId = 0;
const newId = () => `msg-${++nextMsgId}-${Date.now()}`;

// Shared ref so slash command selection works across useAssistantChat instances
const sharedCmdRef = { current: null as { name: string; type: string } | null };

export function useAssistantChat(
  roleName: string = "",
  threadId?: number | null,
  onSaveMessage?: (role: string, content: string) => void,
) {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [isRunning, setIsRunning] = useState(false);
  const [cmdSpecs, setCmdSpecs] = useState<CommandSpec[]>([]);

  const threadIdRef = useRef(threadId);
  threadIdRef.current = threadId;
  const messagesRef = useRef(messages);
  messagesRef.current = messages;

  // Fetch available commands once
  useEffect(() => {
    fetchJson<CommandSpec[]>("/api/ai/commands")
      .then(setCmdSpecs)
      .catch(() => {});
  }, []);

  // Slash command adapter — Directive mode enables Tab auto-complete
  const slashCommands = useMemo(() => {
    return cmdSpecs.map((c) => ({
      id: c.name,
      label: c.name,
      description: c.description,
      icon: c.command_type === "tool" ? "Wrench" : c.command_type === "skill" ? "Brain" : "Workflow",
      metadata: { commandType: c.command_type },
      execute: () => {
        sharedCmdRef.current = { name: c.name, type: c.command_type };
      },
    }));
  }, [cmdSpecs]);

  const { adapter: slashAdapter, action: slashAction } = unstable_useSlashCommandAdapter({
    commands: slashCommands,
    removeOnExecute: true,
  });

  const onNew = useCallback(
    async (msg: { content?: ReadonlyArray<{ type?: string; text?: string }> }) => {
      let text = msg.content?.[0]?.text ?? "";
      if (!text) return;

      // Parse directive from message text (e.g. ":tool[command_name] → command_name")
      let cmd = sharedCmdRef.current;
      sharedCmdRef.current = null;
      if (!cmd) {
        const directiveMatch = text.match(/:(\w+)\[([^\]]+)\]/);
        if (directiveMatch) {
          const cmdName = directiveMatch[2];
          const found = cmdSpecs.find((c) => c.name === cmdName);
          if (found) {
            cmd = { name: found.name, type: found.command_type };
            text = text.replace(directiveMatch[0], "").trim();
          }
        }
      }

      const currentMessages = messagesRef.current;
      const history = currentMessages.map((m) => ({
        role: m.role,
        content: m.reasoning ? `${m.reasoning}\n\n${m.content}` : m.content,
      }));

      const userMsg: ChatMessage = { id: newId(), role: "user", content: text };
      const assistantMsg: ChatMessage = { id: newId(), role: "assistant", content: "" };
      setMessages((prev) => [...prev, userMsg, assistantMsg]);
      setIsRunning(true);

      // Save user message to backend
      if (threadIdRef.current && onSaveMessage) {
        onSaveMessage("user", text);
      }

      void apiChatStream(
        {
          message: text,
          role: roleName,
          history,
          command_name: cmd?.name,
          command_type: cmd?.type,
        },
        {
          onText: (data) => {
            setMessages((prev) => {
              const updated = [...prev];
              const last = updated[updated.length - 1];
              if (last && last.role === "assistant") {
                updated[updated.length - 1] = {
                  ...last,
                  content: last.content + data,
                };
              }
              return updated;
            });
          },
          onReasoning: (data) => {
            setMessages((prev) => {
              const updated = [...prev];
              const last = updated[updated.length - 1];
              if (last && last.role === "assistant") {
                updated[updated.length - 1] = {
                  ...last,
                  reasoning: (last.reasoning ?? "") + data,
                };
              }
              return updated;
            });
          },
          onDone: () => {
            setIsRunning(false);
            // Save assistant message to backend — read latest from ref
            if (threadIdRef.current && onSaveMessage) {
              const lastMsg = messagesRef.current[messagesRef.current.length - 1];
              if (lastMsg && lastMsg.role === "assistant" && lastMsg.content) {
                onSaveMessage("assistant", lastMsg.content);
              }
            }
          },
          onError: (err) => {
            setMessages((prev) => {
              const updated = [...prev];
              const last = updated[updated.length - 1];
              if (last && last.role === "assistant") {
                updated[updated.length - 1] = {
                  ...last,
                  content: last.content + `\n\n[错误] ${err}`,
                };
              }
              return updated;
            });
            setIsRunning(false);
          },
        },
      );
    },
    [roleName, onSaveMessage],
  );

  const runtime = useExternalStoreRuntime<ChatMessage>({
    messages,
    isRunning,
    onNew,
    convertMessage: (m) => {
      const content: Array<{ type: "text" | "reasoning"; text: string }> = [];
      if (m.reasoning) content.push({ type: "reasoning", text: m.reasoning });
      if (m.content) content.push({ type: "text", text: m.content });
      return { id: m.id, role: m.role, content };
    },
  });

  const loadMessages = useCallback((storedMessages: StoredMessage[]) => {
    const converted: ChatMessage[] = storedMessages.map((m) => ({
      id: newId(),
      role: m.role as "user" | "assistant",
      content: m.content,
    }));
    setMessages(converted);
    setIsRunning(false);
  }, []);

  const clearChat = useCallback(() => {
    setMessages([]);
    setIsRunning(false);
  }, []);

  return {
    runtime,
    slashAdapter,
    slashAction,
    clearChat,
    loadMessages,
    messages,
  };
}
