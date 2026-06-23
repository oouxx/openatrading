import { useCallback, useEffect, useRef, useState } from "react";
import { Search, Send } from "lucide-react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { SidebarTrigger } from "@/components/ui/sidebar";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { apiChatStream, fetchJson } from "@/stores/data";
import {
  getMessages,
  saveMessage,
  fetchCommands,
  type ChatMessage,
} from "@/hooks/useChatThreads";
import ChatQuickButtons from "@/components/ChatQuickButtons";
import KlineDrawer from "@/components/KlineDrawer";

interface DisplayMessage {
  id: string;
  role: "user" | "assistant";
  content: string;
  reasoning?: string;
}

interface SlashCommand {
  name: string;
  description: string;
  command_type: string;
}

interface AiCopilotProps {
  activeThreadId: number | null;
  onActiveThreadChange: (id: number | null) => void;
  onReload?: () => void;
}

let msgSeq = 0;
const newMsgId = () => `msg-${++msgSeq}`;

export default function AiCopilot({ activeThreadId, onActiveThreadChange, onReload }: AiCopilotProps) {
  const [messages, setMessages] = useState<DisplayMessage[]>([]);
  const [input, setInput] = useState("");
  const [streaming, setStreaming] = useState(false);
  const [commands, setCommands] = useState<SlashCommand[]>([]);
  const [showSlash, setShowSlash] = useState(false);
  const [slashQuery, setSlashQuery] = useState("");
  const [slashIndex, setSlashIndex] = useState(0);
  const pendingCmdRef = useRef<{ name: string; type: string } | null>(null);
  const messagesRef = useRef<DisplayMessage[]>([]);
  messagesRef.current = messages;
  const activeThreadRef = useRef<number | null>(activeThreadId);
  activeThreadRef.current = activeThreadId;
  const newThreadRef = useRef<number | null>(null);

  // Stock search
  const [searchQuery, setSearchQuery] = useState("");
  const [showDropdown, setShowDropdown] = useState(false);
  const [selectedStock, setSelectedStock] = useState<{ symbol: string; name: string } | null>(null);
  const [searchResults, setSearchResults] = useState<{ code: string; name: string }[]>([]);
  const [searchLoading, setSearchLoading] = useState(false);
  const searchRef = useRef<HTMLInputElement>(null);
  const searchContainerRef = useRef<HTMLDivElement>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout>>();
  const msgContainerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    fetchCommands().then(setCommands).catch(() => {});
  }, []);

  // Load messages when active thread changes
  useEffect(() => {
    if (activeThreadId === null) {
      setMessages([]);
      return;
    }
    // Don't reload if this thread was just created by send() — it already set messages
    if (newThreadRef.current === activeThreadId) {
      newThreadRef.current = null;
      return;
    }
    getMessages(activeThreadId).then((msgs) => {
      setMessages(
        msgs.map((m: ChatMessage) => ({
          id: newMsgId(),
          role: m.role === "user" ? "user" : "assistant",
          content: m.content,
        })),
      );
    });
  }, [activeThreadId]);

  // Stock search debounce
  useEffect(() => {
    if (debounceRef.current) clearTimeout(debounceRef.current);
    if (!searchQuery.trim()) {
      setSearchResults([]);
      return;
    }
    debounceRef.current = setTimeout(async () => {
      setSearchLoading(true);
      try {
        const data = await fetchJson<{ code: string; name: string }[]>(
          `/api/search-stocks?q=${encodeURIComponent(searchQuery.trim())}`,
        );
        setSearchResults(data);
      } catch {
        setSearchResults([]);
      } finally {
        setSearchLoading(false);
      }
    }, 200);
    return () => { if (debounceRef.current) clearTimeout(debounceRef.current); };
  }, [searchQuery]);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key === "p") {
        e.preventDefault();
        searchRef.current?.focus();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (searchContainerRef.current && !searchContainerRef.current.contains(e.target as Node)) {
        setShowDropdown(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, []);

  const scrollDown = useCallback(() => {
    requestAnimationFrame(() => {
      if (msgContainerRef.current) {
        msgContainerRef.current.scrollTop = msgContainerRef.current.scrollHeight;
      }
    });
  }, []);

  const filteredCommands = slashQuery
    ? commands.filter((c) => c.name.toLowerCase().includes(slashQuery.toLowerCase())).slice(0, 8)
    : commands.slice(0, 8);

  const send = useCallback(async (question?: string) => {
    const text = (question || input).trim();
    if (!text || streaming) return;

    let threadId = activeThreadRef.current;
    if (threadId === null) {
      const thread = await fetchJson<{ id: number; title: string }>("/api/ai/threads", {
        method: "POST",
        body: JSON.stringify({ title: "新对话" }),
      });
      threadId = thread.id;
      onActiveThreadChange(thread.id);
    }

    const cmd = pendingCmdRef.current;
    pendingCmdRef.current = null;
    setShowSlash(false);
    setSlashQuery("");
    setInput("");

    const history = messagesRef.current.map((m) => ({
      role: m.role === "assistant" ? "assistant" : "user",
      content: m.reasoning ? `${m.reasoning}\n\n${m.content}` : m.content,
    }));

    const userMsg: DisplayMessage = { id: newMsgId(), role: "user", content: text };
    const aiMsg: DisplayMessage = { id: newMsgId(), role: "assistant", content: "" };
    setMessages((prev) => [...prev, userMsg, aiMsg]);
    setStreaming(true);
    scrollDown();

    saveMessage(threadId, "user", text);

    try {
      await apiChatStream(
        {
          message: text,
          role: "",
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
                updated[updated.length - 1] = { ...last, content: last.content + data };
              }
              return updated;
            });
            scrollDown();
          },
          onReasoning: (data) => {
            setMessages((prev) => {
              const updated = [...prev];
              const last = updated[updated.length - 1];
              if (last && last.role === "assistant") {
                updated[updated.length - 1] = { ...last, reasoning: (last.reasoning ?? "") + data };
              }
              return updated;
            });
          },
          onDone: () => {
            setStreaming(false);
            const lastMsg = messagesRef.current[messagesRef.current.length - 1];
            if (lastMsg && lastMsg.role === "assistant" && lastMsg.content) {
              saveMessage(threadId!, "assistant", lastMsg.content);
              if (text.length > 0) {
                fetchJson(`/api/ai/threads/${threadId}`, {
                  method: "PUT",
                  body: JSON.stringify({ title: text.slice(0, 30) }),
                }).then(() => onReload?.()).catch(() => {});
              }
            }
          },
          onError: (err) => {
            setMessages((prev) => {
              const updated = [...prev];
              const last = updated[updated.length - 1];
              if (last && last.role === "assistant") {
                updated[updated.length - 1] = { ...last, content: last.content + `\n\n[错误] ${err}` };
              }
              return updated;
            });
            setStreaming(false);
          },
        },
      );
    } catch (e: unknown) {
      setMessages((prev) => {
        const updated = [...prev];
        const last = updated[updated.length - 1];
        if (last && last.role === "assistant") {
          updated[updated.length - 1] = { ...last, content: `[错误] ${(e as Error).message || "网络请求失败"}` };
        }
        return updated;
      });
      setStreaming(false);
    }
  }, [input, streaming, scrollDown, onActiveThreadChange]);

  const onInputKeyDown = useCallback((e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (showSlash) {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setSlashIndex((i) => Math.min(i + 1, filteredCommands.length - 1));
        return;
      }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        setSlashIndex((i) => Math.max(i - 1, 0));
        return;
      }
      if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault();
        const cmd = filteredCommands[slashIndex];
        if (cmd) {
          pendingCmdRef.current = { name: cmd.name, type: cmd.command_type };
          setShowSlash(false);
          setSlashQuery("");
          setInput("");
        }
        return;
      }
      if (e.key === "Escape") {
        e.preventDefault();
        setShowSlash(false);
        setSlashQuery("");
        return;
      }
    }
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      send();
    }
  }, [showSlash, filteredCommands, slashIndex, send]);

  const onInputChange = useCallback((e: React.ChangeEvent<HTMLTextAreaElement>) => {
    const val = e.target.value;
    setInput(val);
    if (val.startsWith("/")) {
      setShowSlash(true);
      setSlashQuery(val.slice(1));
      setSlashIndex(0);
    } else {
      setShowSlash(false);
      setSlashQuery("");
    }
  }, []);

  const isEmpty = messages.length === 0;

  return (
    <div className="flex flex-col h-full w-full">
      <div className="flex items-center gap-3 px-4 py-1.5 border-b border-hairline bg-bg-surface flex-shrink-0">
        <SidebarTrigger className="flex-shrink-0" />
        <div className="flex items-center gap-1.5 text-xs text-ink-muted whitespace-nowrap flex-shrink-0">
          <span className="text-white font-medium">市场分析</span>
          <span>·</span>
          <span>分析助手</span>
        </div>
        <div className="flex-1 flex justify-center">
          <div className="relative w-full max-w-[200px]" ref={searchContainerRef}>
            <Search className="absolute left-2 top-1/2 -translate-y-1/2 w-3 h-3 text-ink-muted pointer-events-none" />
            <input
              ref={searchRef}
              className="w-full h-7 pl-7 pr-12 text-xs bg-bg border border-hairline rounded-lg text-white placeholder:text-ink-muted outline-none transition-colors focus:border-ai"
              placeholder="搜索股票..."
              value={searchQuery}
              onChange={(e) => { setSearchQuery(e.target.value); setShowDropdown(true); }}
              onFocus={() => setShowDropdown(true)}
              onKeyDown={(e) => {
                if (e.key === "Escape") { setShowDropdown(false); searchRef.current?.blur(); }
              }}
            />
            <span className="absolute right-1.5 top-1/2 -translate-y-1/2 text-[9px] text-ink-muted bg-bg px-1 py-0.5 rounded-badge pointer-events-none leading-none">
              ⌘P
            </span>
            {showDropdown && searchQuery.trim() && (
              <div className="absolute top-full left-0 right-0 mt-0.5 max-h-72 overflow-y-auto bg-bg-surface border border-hairline rounded-card shadow-lg z-50">
                {searchLoading && <div className="px-3 py-2 text-xs text-ink-muted">搜索中...</div>}
                {!searchLoading && searchResults.length === 0 && <div className="px-3 py-2 text-xs text-ink-muted">无匹配结果</div>}
                {searchResults.map((s) => (
                  <button
                    key={s.code}
                    className="flex items-center gap-2 w-full px-3 py-1.5 text-xs text-left hover:bg-bg-hover transition-colors"
                    onClick={() => {
                      setSelectedStock({ symbol: s.code, name: s.name });
                      setSearchQuery("");
                      setShowDropdown(false);
                      searchRef.current?.blur();
                    }}
                  >
                    <span className="font-medium text-white">{s.name}</span>
                    <span className="text-ink-muted font-mono">{s.code}</span>
                  </button>
                ))}
              </div>
            )}
          </div>
        </div>
        <div className="flex-shrink-0">
          <button className="w-6 h-6 flex items-center justify-center rounded-button text-sm text-ink-muted hover:text-white hover:bg-bg-hover transition-colors" title="更多">
            ⋯
          </button>
        </div>
      </div>

      <div className="flex-1 flex flex-col min-h-0">
        <div ref={msgContainerRef} className="flex-1 overflow-y-auto px-6 py-5 flex flex-col gap-5">
          {isEmpty ? (
            <div className="flex-1 flex items-center justify-center">
              <div className="text-center max-w-[500px]">
                <h2 className="text-2xl font-medium text-white mb-2">AI 行情助手 · A 股专版</h2>
                <p className="text-sm text-ink-muted leading-relaxed mb-6">
                  覆盖上证、深证、北交所全市场数据。支持大盘研判、板块轮动、个股分析、资金面跟踪及策略问答。
                </p>
              </div>
            </div>
          ) : (
            messages.map((msg, idx) => {
              const isLastAi = msg.role === "assistant" && idx === messages.length - 1;
              return (
                <div
                  key={msg.id}
                  className={`flex gap-3 max-w-[820px] ${msg.role === "user" ? "self-end flex-row-reverse" : ""}`}
                >
                  <span
                    className={`w-8 h-8 rounded-full flex items-center justify-center text-[11px] font-semibold flex-shrink-0 ${
                      msg.role === "user" ? "bg-ai text-white" : "bg-bg-hover text-white"
                    }`}
                  >
                    {msg.role === "user" ? "U" : "AI"}
                  </span>
                  <div
                    className={`px-4 py-3 rounded-xl text-sm leading-relaxed ${
                      msg.role === "user"
                        ? "rounded-tr-sm max-w-[70%] bg-ink-dim text-white"
                        : "rounded-tl-sm bg-bg-surface border border-hairline text-white/80"
                    }`}
                  >
                    {msg.reasoning && (
                      <details
                        open={isLastAi && streaming}
                        className="mb-2 text-xs text-ink-muted border-l-2 border-ink-muted/30 pl-3"
                      >
                        <summary className="cursor-pointer select-none hover:text-white">
                          思考过程 {streaming && isLastAi && <span className="ml-1 text-ai">●</span>}
                        </summary>
                        <div className="mt-2 whitespace-pre-wrap leading-relaxed opacity-80">
                          {msg.reasoning}
                        </div>
                      </details>
                    )}
                    {msg.role === "assistant" ? (
                      msg.content ? (
                        <ReactMarkdown remarkPlugins={[remarkGfm]}>{msg.content}</ReactMarkdown>
                      ) : streaming && isLastAi && !msg.reasoning ? (
                        <div className="flex gap-1 py-1">
                          <span className="w-1.5 h-1.5 rounded-full bg-ink-muted animate-pulse" />
                          <span className="w-1.5 h-1.5 rounded-full bg-ink-muted animate-pulse" style={{ animationDelay: "0.2s" }} />
                          <span className="w-1.5 h-1.5 rounded-full bg-ink-muted animate-pulse" style={{ animationDelay: "0.4s" }} />
                        </div>
                      ) : null
                    ) : (
                      msg.content
                    )}
                  </div>
                </div>
              );
            })
          )}
        </div>

        {showSlash && filteredCommands.length > 0 && (
          <div className="absolute bottom-[140px] left-1/2 -translate-x-1/2 w-[420px] max-w-[90vw] bg-bg-surface border border-hairline rounded-card shadow-lg z-50 p-1.5">
            <div className="text-[10px] text-ink-muted uppercase tracking-wider px-2 py-1 font-medium">Commands</div>
            {filteredCommands.map((cmd, i) => (
              <button
                key={cmd.name}
                className={`flex items-center gap-2 w-full px-2.5 py-2 text-sm rounded-md text-left transition-colors ${
                  i === slashIndex ? "bg-bg-hover text-white" : "text-ink-muted hover:text-white"
                }`}
                onClick={() => {
                  pendingCmdRef.current = { name: cmd.name, type: cmd.command_type };
                  setShowSlash(false);
                  setSlashQuery("");
                  setInput("");
                }}
              >
                <span className="flex-1 truncate font-medium">/{cmd.name}</span>
                <span className="text-[10px] text-ink-muted truncate">{cmd.description}</span>
                <span className="text-[9px] px-1.5 py-0.5 rounded-badge bg-bg-hover text-ink-muted uppercase">{cmd.command_type}</span>
              </button>
            ))}
          </div>
        )}

        <div className="px-4 py-2 border-t border-hairline bg-bg-surface flex-shrink-0">
          <div className="relative max-w-[820px] mx-auto">
            <Textarea
              value={input}
              onChange={onInputChange}
              onKeyDown={onInputKeyDown}
              placeholder="输入问题，/ 触发命令..."
              disabled={streaming}
              className="min-h-[44px] max-h-[120px] resize-none text-sm pr-12"
              rows={1}
            />
            <Button
              size="icon"
              className="absolute right-1.5 bottom-1.5 w-8 h-8"
              onClick={() => send()}
              disabled={streaming || !input.trim()}
            >
              <Send className="w-4 h-4" />
            </Button>
          </div>
          <ChatQuickButtons />
        </div>
      </div>

      {selectedStock && (
        <KlineDrawer
          symbol={selectedStock.symbol}
          name={selectedStock.name}
          onClose={() => setSelectedStock(null)}
        />
      )}
    </div>
  );
}
