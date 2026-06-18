import {
  AssistantRuntimeProvider,
  ThreadPrimitive,
  ComposerPrimitive,
  MessagePrimitive,
  groupPartByType,
} from "@assistant-ui/react";
import { useAssistantChat } from "@/hooks/useAssistantChat";
import { MarkdownTextPrimitive } from "@assistant-ui/react-markdown";
import remarkGfm from "remark-gfm";
import {
  ReasoningRoot,
  ReasoningTrigger,
  ReasoningContent,
  ReasoningText,
} from "@/components/ui/reasoning";

interface AiSidebarProps {
  overview?: unknown;
}

export default function AiSidebar(_props: AiSidebarProps) {
  const { runtime, slashAdapter, slashAction, clearChat } = useAssistantChat("");

  return (
    <AssistantRuntimeProvider runtime={runtime}>
      <div className="h-full flex flex-col">
        <ThreadPrimitive.Root className="flex-1 flex flex-col overflow-hidden">
          <ThreadPrimitive.Viewport className="flex-1 overflow-y-auto px-3 py-3">
            <ThreadPrimitive.Messages>
              {() => (
                <MessagePrimitive.Root>
                  <MessagePrimitive.GroupedParts
                    groupBy={groupPartByType({
                      reasoning: ["group-reasoning"],
                    })}
                  >
                    {({ part, children }) => {
                      switch (part.type) {
                        case "group-reasoning": {
                          const running = part.status?.type === "running";
                          return (
                            <ReasoningRoot defaultOpen={running}>
                              <ReasoningTrigger active={running} />
                              <ReasoningContent aria-busy={running}>
                                <ReasoningText>{children}</ReasoningText>
                              </ReasoningContent>
                            </ReasoningRoot>
                          );
                        }
                        case "text":
                          return (
                            <div className="px-1 py-0.5 aui-md">
                              <MarkdownTextPrimitive remarkPlugins={[remarkGfm]} />
                            </div>
                          );
                        case "reasoning":
                          return (
                            <div className="px-1 py-0.5 text-caption text-ink-muted aui-md">
                              <MarkdownTextPrimitive remarkPlugins={[remarkGfm]} />
                            </div>
                          );
                        default:
                          return null;
                      }
                    }}
                  </MessagePrimitive.GroupedParts>
                </MessagePrimitive.Root>
              )}
            </ThreadPrimitive.Messages>
          </ThreadPrimitive.Viewport>

          <div className="border-t border-hairline px-3 py-2 flex-shrink-0">
            <ComposerPrimitive.Unstable_TriggerPopoverRoot>
              <ComposerPrimitive.Root className="flex gap-2 items-end relative">
                <ComposerPrimitive.Input
                  className="flex-1 bg-bg border border-hairline rounded-input px-3 py-2 text-caption text-white placeholder-ink-muted outline-none focus:border-ai/50 transition-colors font-mono"
                />
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
                <ComposerPrimitive.Unstable_TriggerPopover char="/" adapter={slashAdapter}>
                  <ComposerPrimitive.Unstable_TriggerPopover.Action
                    onExecute={slashAction.onExecute}
                    removeOnExecute={slashAction.removeOnExecute}
                  />
                  <ComposerPrimitive.Unstable_TriggerPopoverItems>
                    {(items) => (
                      <div className="absolute bottom-full left-0 right-0 mb-2 max-h-48 overflow-y-auto rounded-xl border border-hairline bg-bg-surface shadow-lg p-1">
                        {items.map((item) => (
                          <ComposerPrimitive.Unstable_TriggerPopoverItem
                            key={item.id}
                            item={item}
                            className="w-full text-left px-3 py-2 rounded-lg text-xs text-white hover:bg-ai/20 cursor-pointer transition-colors data-[highlighted]:bg-ai/30"
                          >
                            <div className="font-medium text-xs text-white">/{item.label}</div>
                            <div className="text-[11px] text-ink-muted mt-0.5">{item.description}</div>
                          </ComposerPrimitive.Unstable_TriggerPopoverItem>
                        ))}
                      </div>
                    )}
                  </ComposerPrimitive.Unstable_TriggerPopoverItems>
                </ComposerPrimitive.Unstable_TriggerPopover>
              </ComposerPrimitive.Root>
            </ComposerPrimitive.Unstable_TriggerPopoverRoot>
          </div>
        </ThreadPrimitive.Root>
      </div>
    </AssistantRuntimeProvider>
  );
}
