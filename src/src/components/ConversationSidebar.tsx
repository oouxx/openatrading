import { useState, useMemo, useCallback, type FC } from "react";
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
  const [search, setSearch] = useState("");

  const threadIds = useAuiState((s) => s.threads.threadIds);
  const threadItems = useAuiState((s) => s.threads.threadItems);

  const filteredIndices = useMemo(() => {
    if (!search.trim()) return threadIds.map((_, i) => i);
    const q = search.trim().toLowerCase();
    const itemsById = new Map(threadItems.map((t) => [t.id, t]));
    return threadIds
      .map((id, i) => {
        const t = itemsById.get(id);
        return t && (t.title || "").toLowerCase().includes(q) ? i : -1;
      })
      .filter((i) => i !== -1);
  }, [threadIds, threadItems, search]);

  return (
    <Sidebar collapsible="icon">
      <SidebarHeader>
        <SidebarMenu>
          <SidebarMenuItem>
            <ThreadListPrimitive.New asChild>
              <SidebarMenuButton tooltip="新建会话">
                <Plus />
                <span>新建会话</span>
              </SidebarMenuButton>
            </ThreadListPrimitive.New>
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
                tooltip="技能"
              >
                <Puzzle />
                <span>技能</span>
              </SidebarMenuButton>
            </SidebarMenuItem>
          </SidebarMenu>
        </SidebarGroup>

        <SidebarGroup>
          <div className="px-3 py-2 group-data-[collapsible=icon]:hidden">
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
              <div className="py-8 text-center text-xs text-muted-foreground group-data-[collapsible=icon]:hidden">
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
              tooltip="设置"
            >
              <Settings />
              <span>设置</span>
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarFooter>
    </Sidebar>
  );
}
