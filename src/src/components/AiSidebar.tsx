import {
  AssistantRuntimeProvider,
  ThreadPrimitive,
  ComposerPrimitive,
  MessagePrimitive,
} from "@assistant-ui/react";
import { useAssistantChat } from "@/hooks/useAssistantChat";

interface AiSidebarProps {
  overview?: unknown;
}

export default function AiSidebar(_props: AiSidebarProps) {
  const { runtime, clearChat } = useAssistantChat();

  return (
    <AssistantRuntimeProvider runtime={runtime}>
      <div className="h-full flex flex-col">
        <ThreadPrimitive.Root className="flex-1 flex flex-col overflow-hidden">
          <ThreadPrimitive.Viewport className="flex-1 overflow-y-auto px-3 py-3">
            <ThreadPrimitive.Messages>
              {() => (
                <MessagePrimitive.Root>
                  <MessagePrimitive.Content />
                </MessagePrimitive.Root>
              )}
            </ThreadPrimitive.Messages>
          </ThreadPrimitive.Viewport>

          <div className="border-t border-hairline px-3 py-2 flex-shrink-0">
            <ComposerPrimitive.Root className="flex gap-2 items-end">
              <ComposerPrimitive.Input className="flex-1 bg-bg border border-hairline rounded-input px-3 py-2 text-caption text-white placeholder-ink-muted outline-none focus:border-ai/50 transition-colors font-mono" />
              <div className="flex gap-1 items-center">
                <ComposerPrimitive.Send className="px-3 py-2 bg-ai/20 text-ai rounded-button text-caption font-mono hover:bg-ai/30 transition-colors cursor-pointer disabled:opacity-30 disabled:cursor-not-allowed">
                  发送
                </ComposerPrimitive.Send>
                <button
                  onClick={clearChat}
                  className="px-2 py-2 bg-bg-hover text-ink-muted rounded-button text-caption hover:text-white transition-colors cursor-pointer flex-shrink-0"
                  title="清空对话"
                >
                  <svg
                    xmlns="http://www.w3.org/2000/svg"
                    className="w-4 h-4"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    strokeWidth="2"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                  >
                    <path d="M3 6h18" />
                    <path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6" />
                    <path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2" />
                  </svg>
                </button>
              </div>
            </ComposerPrimitive.Root>
          </div>
        </ThreadPrimitive.Root>
      </div>
    </AssistantRuntimeProvider>
  );
}
