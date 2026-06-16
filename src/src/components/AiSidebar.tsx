import { useEffect } from "react";
import { ExternalThread } from "@assistant-ui/react";
import { useAssistantChat, type CommandSpec } from "@/hooks/useAssistantChat";
import { fetchJson } from "@/stores/data";

interface AiSidebarProps {
  overview?: unknown;
}

export default function AiSidebar(_props: AiSidebarProps) {
  const { messages, streaming, handleNew, clearChat } = useAssistantChat();

  useEffect(() => {
    fetchJson<CommandSpec[]>("/api/ai/commands").then((cmds) => {
      // Slash commands registered for assistant-ui
    });
  }, []);

  const messagesForThread = messages.map((m) => ({
    id: m.id,
    role: m.role,
    content: [{ type: "text" as const, text: m.text }],
  }));

  return (
    <div className="h-full flex flex-col">
      <div className="flex-1 overflow-y-auto">
        <ExternalThread
          messages={messagesForThread}
          onNew={(appendMsg: { text: string }) => handleNew(appendMsg.text)}
          isRunning={streaming}
        />
      </div>

      {messages.length > 0 && (
        <div className="px-3 py-2 border-t border-hairline flex justify-center flex-shrink-0">
          <button
            onClick={clearChat}
            className="px-3 py-1.5 rounded-lg border border-hairline text-xs text-ink-muted hover:text-white transition-colors cursor-pointer"
          >
            清空对话
          </button>
        </div>
      )}
    </div>
  );
}
