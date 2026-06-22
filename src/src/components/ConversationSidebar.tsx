import { useState, useMemo } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import { Search, Plus, Settings, Puzzle } from "lucide-react";
import { useAppMode } from "@/contexts/AppModeContext";
import { usePanelWidth } from "@/hooks/usePanelWidth";
import {
  ThreadListPrimitive,
  ThreadListItemPrimitive,
  useAuiState,
} from "@assistant-ui/react";

export default function ConversationSidebar() {
  const { mode, setMode } = useAppMode();
  const { displayWidth, collapsed, toggleCollapse } = usePanelWidth(
    "layout:left-width",
    240,
    180,
    360,
    "layout:left-collapsed",
  );
  const [search, setSearch] = useState("");

  const threadItems = useAuiState((s) => s.threads.threadItems);
  const mainThreadId = useAuiState((s) => s.threads.mainThreadId);

  const filteredThreads = useMemo(() => {
    if (!search.trim()) return threadItems;
    const q = search.trim().toLowerCase();
    return threadItems.filter((t) =>
      (t.title || "").toLowerCase().includes(q)
    );
  }, [threadItems, search]);

  if (collapsed) {
    return (
      <div className="flex flex-col items-center py-3 gap-4 w-12 border-r border-hairline bg-bg-surface flex-shrink-0">
        <ThreadListPrimitive.New asChild>
          <Button variant="ghost" size="icon" className="w-8 h-8" title="新建会话">
            <Plus className="w-4 h-4" />
          </Button>
        </ThreadListPrimitive.New>
        <Button variant="ghost" size="icon" onClick={() => setMode("skill-editor")} className="w-8 h-8">
          <Puzzle className="w-4 h-4" />
        </Button>
        <div className="flex-1" />
        <Button variant="ghost" size="icon" onClick={() => setMode("settings")} className="w-8 h-8">
          <Settings className="w-4 h-4" />
        </Button>
      </div>
    );
  }

  return (
    <div
      className="flex flex-col border-r border-hairline bg-bg-surface flex-shrink-0 overflow-hidden"
      style={{ width: displayWidth || 240, minWidth: 0 }}
    >
      {/* New conversation + collapse toggle */}
      <div className="flex items-center gap-1 px-3 py-2.5 border-b border-hairline">
        <ThreadListPrimitive.New asChild>
          <Button
            variant="default"
            size="sm"
            className="w-full justify-start gap-2 text-sm h-9 flex-1"
          >
            <Plus className="w-4 h-4" />
            新建会话
          </Button>
        </ThreadListPrimitive.New>
        <Button variant="ghost" size="icon" onClick={toggleCollapse} className="w-8 h-8 flex-shrink-0">
          <span className="block w-1 h-1 rounded-full bg-ink-muted shadow-[0_4px_0_0_var(--color-muted),0_8px_0_0_var(--color-muted)]" />
        </Button>
      </div>

      {/* Skills entry */}
      <button
        onClick={() => setMode(mode === "skill-editor" ? "chat" : "skill-editor")}
        className="flex items-center gap-2 px-4 py-2 text-sm text-ink-muted hover:text-white hover:bg-bg-hover transition-colors border-b border-hairline"
      >
        <Puzzle className="w-4 h-4" />
        技能
      </button>

      {/* Search */}
      <div className="px-3 py-2">
        <div className="relative">
          <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-ink-muted" />
          <Input
            placeholder="搜索会话..."
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className="pl-8 h-8 text-xs"
          />
        </div>
      </div>

      <Separator />

      {/* Session List */}
      <ScrollArea className="flex-1 px-2 py-1">
        <ThreadListPrimitive.Items>
          {() =>
            filteredThreads.length === 0 ? (
              <div className="py-8 text-center text-xs text-ink-muted">
                {search.trim() ? "未找到匹配会话" : "暂无会话"}
              </div>
            ) : (
              filteredThreads.map((t) => (
                <ThreadListItemPrimitive.Root
                  key={t.id}
                  className="group relative flex h-8 items-center rounded-lg transition-colors hover:bg-bg-hover data-[active=true]:bg-ai/10"
                >
                  <ThreadListItemPrimitive.Trigger className="flex h-full min-w-0 flex-1 items-center px-2.5 text-start text-sm gap-2">
                    <span className={`w-1.5 h-1.5 rounded-full flex-shrink-0 ${mainThreadId === t.id ? "bg-up" : "bg-ink-muted"}`} />
                    <span className="min-w-0 flex-1 truncate text-xs">
                      <ThreadListItemPrimitive.Title fallback="新对话" />
                    </span>
                  </ThreadListItemPrimitive.Trigger>
                </ThreadListItemPrimitive.Root>
              ))
            )
          }
        </ThreadListPrimitive.Items>
      </ScrollArea>

      {/* Settings at bottom */}
      <Separator />
      <button
        onClick={() => setMode(mode === "settings" ? "chat" : "settings")}
        className="flex items-center gap-2 px-4 py-2.5 text-sm text-ink-muted hover:text-white hover:bg-bg-hover transition-colors"
      >
        <Settings className="w-4 h-4" />
        设置
      </button>
    </div>
  );
}
