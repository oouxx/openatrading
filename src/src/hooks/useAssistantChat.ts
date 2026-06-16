import { useState, useCallback, useRef } from "react";
import { useExternalStoreRuntime } from "@assistant-ui/react";
import { apiChatStream } from "@/stores/data";

export interface CommandSpec {
  name: string;
  description: string;
  command_type: "tool" | "skill" | "pipeline";
}

interface ChatMessage {
  id: string;
  role: "user" | "assistant";
  content: string;
}

let nextMsgId = 0;
const newId = () => `msg-${++nextMsgId}`;

export function useAssistantChat() {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [isRunning, setIsRunning] = useState(false);
  const cmdRef = useRef<{ name: string; type: string } | null>(null);

  const onNew = useCallback(
    async (msg: { content?: ReadonlyArray<{ type?: string; text?: string }> }) => {
      const text = msg.content?.[0]?.text ?? "";
      if (!text) return;

      const cmd = cmdRef.current;
      cmdRef.current = null;

      const history = messages.map((m) => ({ role: m.role, content: m.content }));

      const userMsg: ChatMessage = { id: newId(), role: "user", content: text };
      const assistantMsg: ChatMessage = { id: newId(), role: "assistant", content: "" };
      setMessages((prev) => [...prev, userMsg, assistantMsg]);
      setIsRunning(true);

      void apiChatStream(
        {
          message: text,
          role: "zarili",
          history,
          command_name: cmd?.name,
          command_type: cmd?.type,
        },
        {
          onData: (data) => {
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
    [messages],
  );

  const runtime = useExternalStoreRuntime<ChatMessage>({
    messages,
    isRunning,
    onNew,
    convertMessage: (m) => ({
      id: m.id,
      role: m.role,
      content: [{ type: "text", text: m.content }],
    }),
  });

  const clearChat = useCallback(() => {
    setMessages([]);
    setIsRunning(false);
  }, []);

  return { runtime, cmdRef, clearChat };
}
