import { useCallback, useMemo, useEffect, useRef, useState } from "react";
import { Search } from "lucide-react";
import {
  AssistantRuntimeProvider,
  useRemoteThreadListRuntime,
  useAuiState,
} from "@assistant-ui/react";
import { useAssistantChat } from "@/hooks/useAssistantChat";
import { createThreadListAdapter, getMessages, saveMessage } from "@/hooks/useChatThreads";
import { fetchJson } from "@/stores/data";
import { Thread } from "@/components/assistant-ui/thread";
import ChatQuickButtons from "@/components/ChatQuickButtons";
import KlineDrawer from "@/components/KlineDrawer";

function AiCopilotContent() {
  const mainThreadId = useAuiState((s) => s.threads.mainThreadId);
  const threadItems = useAuiState((s) => s.threads.threadItems);

  const remoteId = useMemo(() => {
    if (!mainThreadId) return null;
    const item = threadItems.find((t) => t.id === mainThreadId);
    return item?.remoteId ?? null;
  }, [mainThreadId, threadItems]);

  const remoteIdRef = useRef<string | null>(null);
  remoteIdRef.current = remoteId;

  const onSaveMessage = useCallback((role: string, content: string) => {
    const rid = remoteIdRef.current;
    if (rid && /^\d+$/.test(rid)) {
      saveMessage(Number(rid), role, content);
    }
  }, []);

  const { slashAdapter, slashAction, loadMessages } = useAssistantChat(
    "",
    remoteId && /^\d+$/.test(remoteId) ? Number(remoteId) : null,
    onSaveMessage,
  );

  useEffect(() => {
    if (remoteId && /^\d+$/.test(remoteId)) {
      getMessages(Number(remoteId)).then((msgs) => {
        loadMessages(msgs);
      });
    }
  }, [remoteId, loadMessages]);

  // Stock search state
  const [searchQuery, setSearchQuery] = useState("");
  const [showDropdown, setShowDropdown] = useState(false);
  const [selectedStock, setSelectedStock] = useState<{symbol: string; name: string} | null>(null);
  const [searchResults, setSearchResults] = useState<{code: string; name: string}[]>([]);
  const [searchLoading, setSearchLoading] = useState(false);
  const searchRef = useRef<HTMLInputElement>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout>>();

  // Debounced API search
  useEffect(() => {
    if (debounceRef.current) clearTimeout(debounceRef.current);
    if (!searchQuery.trim()) {
      setSearchResults([]);
      return;
    }
    debounceRef.current = setTimeout(async () => {
      setSearchLoading(true);
      try {
        const data = await fetchJson<{code: string; name: string}[]>(
          `/api/search-stocks?q=${encodeURIComponent(searchQuery.trim())}`
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

  // Ctrl+P / Cmd+P shortcut to focus search
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

  // Close dropdown on outside click
  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (searchRef.current && !searchRef.current.contains(e.target as Node)) {
        setShowDropdown(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, []);

  return (
    <div className="flex flex-col h-full w-full relative">
      {/* Center header: breadcrumb + stock search */}
      <div className="flex items-center gap-3 px-4 py-1.5 border-b border-hairline bg-bg-surface flex-shrink-0">
        <div className="flex items-center gap-1.5 text-xs text-ink-muted whitespace-nowrap flex-shrink-0">
          <span className="text-white font-medium">市场分析</span>
          <span>·</span>
          <span>分析助手</span>
        </div>
        <div className="flex-1 flex justify-center">
          <div className="relative w-full max-w-[200px]">
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
                {searchLoading && (
                  <div className="px-3 py-2 text-xs text-ink-muted">搜索中...</div>
                )}
                {!searchLoading && searchResults.length === 0 && (
                  <div className="px-3 py-2 text-xs text-ink-muted">无匹配结果</div>
                )}
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

      {/* Chat area */}
      <div className="flex flex-row flex-1 min-h-0">
        <div className="flex-1 flex flex-col min-w-0">
          <Thread slashAdapter={slashAdapter} slashAction={slashAction} />
          <ChatQuickButtons />
        </div>
      </div>

      {/* Stock detail overlay */}
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

export default function AiCopilot() {
  const adapter = useMemo(() => createThreadListAdapter(), []);

  const { runtime } = useAssistantChat("");

  const runtimeHook = useCallback(() => runtime, [runtime]);

  const threadListRuntime = useRemoteThreadListRuntime({
    adapter,
    runtimeHook,
  });

  return (
    <AssistantRuntimeProvider runtime={threadListRuntime}>
      <AiCopilotContent />
    </AssistantRuntimeProvider>
  );
}
