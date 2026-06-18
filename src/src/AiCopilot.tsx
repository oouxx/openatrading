import { useState, useEffect } from "react";
import {
  AssistantRuntimeProvider,
  ThreadPrimitive,
  ComposerPrimitive,
  MessagePrimitive,
  groupPartByType,
} from "@assistant-ui/react";
import { useIndexQuotes } from "@/hooks/query/useIndexQuotes";
import { useAssistantChat } from "@/hooks/useAssistantChat";
import { MarkdownTextPrimitive } from "@assistant-ui/react-markdown";
import remarkGfm from "remark-gfm";
import {
  ReasoningRoot,
  ReasoningTrigger,
  ReasoningContent,
  ReasoningText,
} from "@/components/ui/reasoning";

// ── Ticker ───────────────────────────────────────────

function TickerStrip() {
  const { data: quotes = [] } = useIndexQuotes();
  const [data, setData] = useState<typeof quotes>([]);
  const [prices, setPrices] = useState<
    Record<string, { price: number; change: number }>
  >({});

  useEffect(() => {
    if (quotes.length && !data.length) {
      setData(quotes);
      const p: Record<string, { price: number; change: number }> = {};
      quotes.forEach((q) => {
        p[q.symbol] = { price: q.last_px, change: q.px_change_rate };
      });
      setPrices(p);
    }
  }, [quotes, data.length]);

  if (!data.length) return null;

  const iconInfo: Record<string, { cls: string; label: string }> = {
    "000001.SS": { cls: "sh", label: "沪" },
    "399001.SZ": { cls: "sz", label: "深" },
    "000300.SS": { cls: "hs300", label: "300" },
    "399006.SZ": { cls: "cyb", label: "创" },
    "000688.SS": { cls: "kc50", label: "科" },
  };

  const tickerData = data.map((d) => ({
    name: d.name.length > 4 ? d.name.replace("指数", "") : d.name,
    icon: iconInfo[d.symbol] || { cls: "kc50", label: d.name[0] },
    price: prices[d.symbol]?.price ?? d.last_px,
    change: prices[d.symbol]?.change ?? d.px_change_rate,
    up: (prices[d.symbol]?.change ?? d.px_change_rate) >= 0,
  }));

  return (
    <div
      className="bg-bg-surface border-b border-hairline overflow-x-auto whitespace-nowrap"
      style={{ WebkitOverflowScrolling: "touch" }}
    >
      <div className="inline-flex gap-0 px-4">
        {tickerData.map((m) => {
          const iconBg: Record<string, string> = {
            sh: "bg-up",
            sz: "bg-[oklch(45%_0.14_240)]",
            hs300: "bg-ai",
            cyb: "bg-down",
            kc50: "bg-[oklch(42%_0.16_280)]",
          };
          return (
            <div
              key={m.name}
              className="inline-flex items-center gap-2.5 py-2.5 px-[18px] border-r border-hairline last:border-r-0 min-w-[178px] cursor-pointer hover:opacity-80 transition-opacity"
            >
              <span
                className={`w-8 h-8 rounded-lg flex items-center justify-center text-[10px] font-bold text-white flex-shrink-0 ${iconBg[m.icon.cls] || "bg-ai"}`}
              >
                {m.icon.label}
              </span>
              <div className="leading-tight">
                <div className="font-semibold text-xs text-white tracking-wide">
                  {m.name}
                </div>
                <div className="font-mono text-base font-semibold text-white">
                  {m.price.toLocaleString(undefined, {
                    minimumFractionDigits: 2,
                    maximumFractionDigits: 2,
                  })}
                </div>
                <div
                  className={`font-mono text-[11px] font-medium ${m.up ? "text-up" : "text-down"}`}
                >
                  {m.up ? "+" : ""}
                  {m.change.toFixed(2)}%
                </div>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}

// ── Thread Content ───────────────────────────────────

function ThreadContent({
  onClear,
  slashAdapter,
  slashAction,
}: {
  onClear: () => void;
  slashAdapter: import("@assistant-ui/core").Unstable_TriggerAdapter;
  slashAction: { onExecute: (item: import("@assistant-ui/core").Unstable_TriggerItem) => void; removeOnExecute?: boolean };
}) {
  return (
    <ThreadPrimitive.Root className="flex-1 flex flex-col overflow-hidden">
      <ThreadPrimitive.Viewport className="flex-1 overflow-y-auto px-4 py-4">
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

      <div className="border-t border-hairline px-4 py-3">
        <ComposerPrimitive.Unstable_TriggerPopoverRoot>
          <ComposerPrimitive.Unstable_TriggerPopover char="/" adapter={slashAdapter}>
            <ComposerPrimitive.Unstable_TriggerPopover.Action
              onExecute={slashAction.onExecute}
              removeOnExecute={slashAction.removeOnExecute}
            />
            <ComposerPrimitive.Unstable_TriggerPopoverItems>
              {(items) => (
                <div className="max-h-48 overflow-y-auto rounded-xl border border-hairline bg-bg-surface shadow-lg p-1 mb-2">
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

          <ComposerPrimitive.Root className="flex gap-2 items-end">
            <ComposerPrimitive.Input
              className="flex-1 px-3.5 py-2.5 rounded-xl border border-hairline text-sm leading-relaxed resize-none outline-none bg-bg text-white placeholder-ink-muted focus:border-ai transition-colors font-sans"
            />
            <div className="flex gap-2 items-center">
              <ComposerPrimitive.Send className="w-11 h-11 rounded-xl border-none bg-ai text-white cursor-pointer flex items-center justify-center flex-shrink-0 hover:opacity-90 transition-opacity disabled:opacity-30 disabled:cursor-not-allowed">
                <svg
                  xmlns="http://www.w3.org/2000/svg"
                  className="w-5 h-5"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="2.5"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                >
                  <line x1="12" y1="19" x2="12" y2="5" />
                  <polyline points="5 12 12 5 19 12" />
                </svg>
              </ComposerPrimitive.Send>
              <button
                onClick={onClear}
                className="px-3 py-2 rounded-lg border border-hairline text-xs text-ink-muted hover:text-white transition-colors cursor-pointer"
              >
                清空
              </button>
            </div>
          </ComposerPrimitive.Root>
        </ComposerPrimitive.Unstable_TriggerPopoverRoot>
      </div>
    </ThreadPrimitive.Root>
  );
}

// ── Main Component ──────────────────────────────────

export default function AiCopilot() {
  const { runtime, slashAdapter, slashAction, clearChat } = useAssistantChat("");

  return (
    <AssistantRuntimeProvider runtime={runtime}>
      <div
        className="min-h-screen bg-bg pt-11 flex flex-col"
        style={{ height: "calc(100vh - 44px)" }}
      >
        <TickerStrip />
        <div className="flex-1 overflow-hidden flex flex-col">
          <ThreadContent
            onClear={clearChat}
            slashAdapter={slashAdapter}
            slashAction={slashAction}
          />
        </div>
      </div>
    </AssistantRuntimeProvider>
  );
}
