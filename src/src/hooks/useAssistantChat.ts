import { useState, useCallback, useRef } from "react";
import { useExternalStoreRuntime } from "@assistant-ui/react";
import { apiChatStream } from "@/stores/data";

export interface CommandSpec {
  name: string;
  description: string;
  command_type: "tool" | "skill" | "pipeline";
}

interface ChatMessage {
  role: "user" | "assistant";
  content: string;
}

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

      const userMsg: ChatMessage = { role: "user", content: text };
      setMessages((prev) => [...prev, userMsg]);
      setIsRunning(true);

      const history = messages.map((m) => ({
        role: m.role,
        content: m.content,
      }));

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
              } else {
                updated.push({ role: "assistant", content: data });
              }
              return updated;
            });
          },
          onDone: () => setIsRunning(false),
          onError: () => setIsRunning(false),
        },
      );
    },
    [messages],
  );

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const runtime = useExternalStoreRuntime<any>({
    messages,
    isRunning,
    onNew,
  });

  const clearChat = useCallback(() => {
    setMessages([]);
    setIsRunning(false);
  }, []);

  return { runtime, cmdRef, clearChat };
}
