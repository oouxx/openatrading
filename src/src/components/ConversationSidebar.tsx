import { useState, useMemo, type FC } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
} from "@/components/ui/sidebar";
import { Search, Plus, Settings, Puzzle } from "lucide-react";
import { useAppMode } from "@/contexts/AppModeContext";
import { usePanelWidth } from "@/hooks/usePanelWidth";
import {
  ThreadListItemPrimitive,
  ThreadListPrimitive,
  useAuiState,
} from "@assistant-ui/react";

const ThreadListItem: FC = () => (
  <ThreadListItemPrimitive.Root className="group relative flex h-8 items-center rounded-lg transition-colors hover:bg-bg-hover data-[active=true]:bg-ai/10">
    <ThreadListItemPrimitive.Trigger className="flex h-full min-w-0 flex-1 items-center px-2.5 text-start text-sm gap-2">
      <span className="w-1.5 h-1.5 rounded-full flex-shrink-0 bg-ink-muted data-[active=true]:bg-up" />
      <span className="min-w-0 flex-1 truncate text-xs">
        <ThreadListItemPrimitive.Title fallback="新对话" />
      </span>
    </ThreadListItemPrimitive.Trigger>
  </ThreadListItemPrimitive.Root>
);

export default function ConversationSidebar() {
  const { mode, setMode } = useAppMode();
  const { displayWidth, collapsed, toggleCollapse } = usePanelWidth(
    "layout:left-width", 240, 180, 360, "layout:left-collapsed",
  );
  const [search, setSearch] = useState("");

  const threadIds = useAuiState((s) => s.threads.threadIds);
  const threadItems = useAuiState((s) => s.threads.threadItems);

  const filteredIndices = useMemo(() => {
    if (!search.trim()) return threadIds.map((_, i) => i);
    const q = search.trim().toLowerCase();
    const itemsById = new Map(threadItems.map((t) => [t.id, t]));
    return threadIds
      .map((id, i) => { const t = itemsById.get(id); return t && (t.title || "").toLowerCase().includes(q) ? i : -1; })
      .filter((i) => i !== -1);
  }, [threadIds, threadItems, search]);

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
    <Sidebar className="border-r border-hairline" style={{ width: displayWidth || 240 }}>
      <SidebarHeader>
        <SidebarMenu>
          <SidebarMenuItem>
            <ThreadListPrimitive.New asChild>
              <SidebarMenuButton>
                <Plus />
                新建会话
              </SidebarMenuButton>
            </ThreadListPrimitive.New>
          </SidebarMenuItem>
          <SidebarMenuItem>
            <Button variant="ghost" size="icon" onClick={toggleCollapse} className="ml-auto">
              <span className="block w-1 h-1 rounded-full bg-ink-muted shadow-[0_4px_0_0_var(--color-muted),0_8px_0_0_var(--color-muted)]" />
            </Button>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarHeader>

      <SidebarContent>
        <SidebarGroup>
          <SidebarMenu>
            <SidebarMenuItem>
              <SidebarMenuButton
                onClick={() => setMode(mode === "skill-editor" ? "chat" : "skill-editor")}
                isActive={mode === "skill-editor"}
              >
                <Puzzle />
                技能
              </SidebarMenuButton>
            </SidebarMenuItem>
          </SidebarMenu>
        </SidebarGroup>

        <SidebarGroup>
          <div className="px-3 py-2">
            <div className="relative">
              <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-muted-foreground" />
              <Input
                placeholder="搜索会话..."
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                className="pl-8 h-8 text-xs"
              />
            </div>
          </div>
        </SidebarGroup>

        <SidebarGroup className="flex-1 min-h-0">
          <SidebarMenu>
            {filteredIndices.length === 0 ? (
              <div className="py-8 text-center text-xs text-muted-foreground">
                {search.trim() ? "未找到匹配会话" : "暂无会话"}
              </div>
            ) : (
              filteredIndices.map((index) => (
                <ThreadListPrimitive.ItemByIndex
                  key={threadIds[index]}
                  index={index}
                  components={{ ThreadListItem }}
                />
              ))
            )}
          </SidebarMenu>
        </SidebarGroup>
      </SidebarContent>

      <SidebarFooter>
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton
              onClick={() => setMode(mode === "settings" ? "chat" : "settings")}
              isActive={mode === "settings"}
            >
              <Settings />
              设置
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarFooter>
    </Sidebar>
  );
}
