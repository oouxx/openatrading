import { useEffect, useState } from "react";
import { useIndexQuotes } from "@/hooks/query/useIndexQuotes";
import { useAssistantChat, type CommandSpec } from "@/hooks/useAssistantChat";
import { fetchJson } from "@/stores/data";
import { ExternalThread } from "@assistant-ui/react";

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

// ── Main Component ────────────────────────────────────

export default function AiCopilot() {
  const { messages, streaming, handleNew, clearChat } = useAssistantChat();

  // Load commands on mount
  useEffect(() => {
    fetchJson<CommandSpec[]>("/api/ai/commands").then((cmds) => {
      // Commands available for slash completion
    });
  }, []);

  const messagesForThread = messages.map((m) => ({
    id: m.id,
    role: m.role,
    content: [{ type: "text" as const, text: m.text }],
  }));

  return (
    <div
      className="min-h-screen bg-bg pt-11 flex flex-col"
      style={{ height: "calc(100vh - 44px)" }}
    >
      <TickerStrip />

      <div className="flex-1 overflow-hidden flex flex-col">
        <div className="flex-1 overflow-hidden">
          <ExternalThread
            messages={messagesForThread}
            onNew={(appendMsg: { text: string }) => handleNew(appendMsg.text)}
            isRunning={streaming}
          />
        </div>

        {messages.length > 0 && (
          <div className="px-4 pb-3 flex justify-center">
            <button
              onClick={clearChat}
              className="px-3 py-1.5 rounded-lg border border-hairline text-xs text-ink-muted hover:text-white transition-colors cursor-pointer"
            >
              清空对话
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
