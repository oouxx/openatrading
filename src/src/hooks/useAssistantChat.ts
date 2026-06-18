import { useState, useCallback, useRef, useEffect, useMemo } from "react";
import { useExternalStoreRuntime, unstable_useSlashCommandAdapter } from "@assistant-ui/react";
import { apiChatStream, fetchJson } from "@/stores/data";

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

export function useAssistantChat(roleName: string = "") {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [isRunning, setIsRunning] = useState(false);
  const [cmdSpecs, setCmdSpecs] = useState<CommandSpec[]>([]);
  const cmdRef = useRef<{ name: string; type: string } | null>(null);

  // Fetch available commands once
  useEffect(() => {
    fetchJson<CommandSpec[]>("/api/ai/commands")
      .then(setCmdSpecs)
      .catch(() => {});
  }, []);

  // Slash command adapter — correctly typed for Unstable_TriggerPopover
  const slashCommands = useMemo(() => {
    return cmdSpecs.map((c) => ({
      id: c.name,
      label: c.name,
      description: c.description,
      commandType: c.command_type,
      execute: () => {
        cmdRef.current = { name: c.name, type: c.command_type };
      },
    }));
  }, [cmdSpecs]);

  const { adapter: slashAdapter, action: slashAction } = unstable_useSlashCommandAdapter({
    commands: slashCommands,
    removeOnExecute: false,
  });

  const onNew = useCallback(
    async (msg: { content?: ReadonlyArray<{ type?: string; text?: string }> }) => {
      const text = msg.content?.[0]?.text ?? "";
      if (!text) return;

      const cmd = cmdRef.current;
      cmdRef.current = null;

      const history = messages.map((m) => ({
        role: m.role,
        content: m.reasoning ? `${m.reasoning}\n\n${m.content}` : m.content,
      }));

      const userMsg: ChatMessage = { id: newId(), role: "user", content: text };
      const assistantMsg: ChatMessage = { id: newId(), role: "assistant", content: "" };
      setMessages((prev) => [...prev, userMsg, assistantMsg]);
      setIsRunning(true);

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
          onDone: () => setIsRunning(false),
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
    [messages, roleName],
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

  const clearChat = useCallback(() => {
    setMessages([]);
    setIsRunning(false);
  }, []);

  return {
    runtime,
    slashAdapter,
    slashAction,
    clearChat,
  };
}
