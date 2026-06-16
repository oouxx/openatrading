import { useState, useCallback, useRef } from "react";
import { apiChatStream } from "@/stores/data";

export interface CommandSpec {
  name: string;
  description: string;
  command_type: "tool" | "skill" | "pipeline";
}

export interface ThreadMessage {
  id: string;
  role: "user" | "assistant";
  text: string;
}

export type AppendMessage = { text: string };

let msgId = 0;
function nextId() {
  return `msg-${++msgId}`;
}

export function useAssistantChat() {
  const [messages, setMessages] = useState<ThreadMessage[]>([]);
  const [streaming, setStreaming] = useState(false);
  const cmdRef = useRef<{ name: string; type: string } | null>(null);

  const appendToLastMessage = useCallback((text: string) => {
    setMessages((prev) => {
      const updated = [...prev];
      const last = updated[updated.length - 1];
      if (last && last.role === "assistant") {
        updated[updated.length - 1] = { ...last, text: last.text + text };
      }
      return updated;
    });
  }, []);

  const handleNew = useCallback(
    (content: string, commandContext?: { name: string; type: string } | null) => {
      const cmd = commandContext ?? cmdRef.current;
      cmdRef.current = null;

      setMessages((prev) => [
        ...prev,
        { id: nextId(), role: "user", text: content },
        { id: nextId(), role: "assistant", text: "" },
      ]);
      setStreaming(true);

      const history = messages.map((m) => ({
        role: m.role === "assistant" ? "assistant" : "user",
        content: m.text,
      }));

      apiChatStream(
        {
          message: content,
          role: "zarili",
          history,
          command_name: cmd?.name,
          command_type: cmd?.type,
        },
        {
          onData: (data) => appendToLastMessage(data),
          onDone: () => setStreaming(false),
          onError: (err) =>
            setMessages((prev) => {
              const updated = [...prev];
              const last = updated[updated.length - 1];
              if (last && last.role === "assistant") {
                updated[updated.length - 1] = {
                  ...last,
                  text: last.text + `\n\n[错误] ${err}`,
                };
              }
              return updated;
            }),
        }
      );
    },
    [messages, appendToLastMessage]
  );

  const sendCommand = useCallback(
    (cmd: CommandSpec) => {
      cmdRef.current = { name: cmd.name, type: cmd.command_type };
      if (cmd.command_type === "tool") {
        // Execute immediately
        handleNew(`/${cmd.name}`, { name: cmd.name, type: "tool" });
      }
      // Skill/pipeline: fill composer text — handled by caller via the returned cmdRef
    },
    [handleNew]
  );

  const clearChat = useCallback(() => {
    setMessages([]);
    setStreaming(false);
  }, []);

  return {
    messages,
    streaming,
    handleNew,
    sendCommand,
    cmdRef,
    clearChat,
  };
}
