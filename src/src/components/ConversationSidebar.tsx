import { useMemo, useState } from "react";
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
import { Search, Plus, Settings, Puzzle, Trash2 } from "lucide-react";
import { useAppMode } from "@/contexts/AppModeContext";
import type { ChatThread } from "@/hooks/useChatThreads";

interface ConversationSidebarProps {
  threads: ChatThread[];
  activeThreadId: number | null;
  onSelectThread: (id: number) => void;
  onNewThread: () => void;
  onDeleteThread: (id: number) => void;
}

export default function ConversationSidebar({
  threads,
  activeThreadId,
  onSelectThread,
  onNewThread,
  onDeleteThread,
}: ConversationSidebarProps) {
  const { mode, setMode } = useAppMode();
  const [search, setSearch] = useState("");

  const filteredThreads = useMemo(() => {
    if (!search.trim()) return threads;
    const q = search.trim().toLowerCase();
    return threads.filter((t) => t.title.toLowerCase().includes(q));
  }, [threads, search]);

  return (
    <Sidebar collapsible="icon">
      <SidebarHeader>
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton onClick={onNewThread} tooltip="新建会话">
              <Plus />
              <span>新建会话</span>
            </SidebarMenuButton>
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
            {filteredThreads.length === 0 ? (
              <div className="py-8 text-center text-xs text-muted-foreground group-data-[collapsible=icon]:hidden">
                {search.trim() ? "未找到匹配会话" : "暂无会话"}
              </div>
            ) : (
              filteredThreads.map((t) => (
                <SidebarMenuItem key={t.id} className="group/item">
                  <SidebarMenuButton
                    isActive={activeThreadId === t.id}
                    onClick={() => onSelectThread(t.id)}
                    tooltip={t.title}
                  >
                    <span className={`w-1.5 h-1.5 rounded-full flex-shrink-0 ${activeThreadId === t.id ? "bg-up" : "bg-ink-muted"}`} />
                    <span className="flex-1 truncate">{t.title}</span>
                  </SidebarMenuButton>
                  <button
                    className="absolute right-1.5 top-1/2 -translate-y-1/2 opacity-0 group-hover/item:opacity-100 text-ink-muted hover:text-down transition-opacity z-10"
                    onClick={(e) => { e.stopPropagation(); onDeleteThread(t.id); }}
                    title="删除"
                  >
                    <Trash2 className="w-3 h-3" />
                  </button>
                </SidebarMenuItem>
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
