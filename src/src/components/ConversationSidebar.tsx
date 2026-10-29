import { useMemo, useState } from "react"
import { Input } from "@/components/ui/input"
import { Button } from "@/components/ui/button"
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
} from "@/components/ui/sidebar"
import { Search, Plus, Settings, Puzzle, Users, Star, Trash2, Timer } from "lucide-react"
import { useAppMode } from "@/contexts/useAppMode"
import type { ChatSession } from "@/hooks/useChatThreads"
import { cn } from "@/lib/utils"

interface ConversationSidebarProps {
  threads: ChatSession[]
  activeThreadId: string | null
  onSelectThread: (id: string) => void
  onNewThread: () => void
  onDeleteThread: (id: string) => void
}

export default function ConversationSidebar({
  threads,
  activeThreadId,
  onSelectThread,
  onNewThread,
  onDeleteThread,
}: ConversationSidebarProps) {
  const { mode, setMode } = useAppMode()
  const [search, setSearch] = useState("")

  const filteredThreads = useMemo(() => {
    if (!search.trim()) return threads
    const q = search.trim().toLowerCase()
    return threads.filter((t) => (t.name ?? "").toLowerCase().includes(q))
  }, [threads, search])

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
            <SidebarMenuItem>
              <SidebarMenuButton
                onClick={() => setMode(mode === "roles" ? "chat" : "roles")}
                isActive={mode === "roles"}
                tooltip="模拟盘"
              >
                <Users />
                <span>模拟盘</span>
              </SidebarMenuButton>
            </SidebarMenuItem>
            <SidebarMenuItem>
              <SidebarMenuButton
                onClick={() => setMode(mode === "watchlist" ? "chat" : "watchlist")}
                isActive={mode === "watchlist"}
                tooltip="自选"
              >
                <Star />
                <span>自选</span>
              </SidebarMenuButton>
            </SidebarMenuItem>
            <SidebarMenuItem>
              <SidebarMenuButton
                onClick={() => setMode(mode === "automations" ? "chat" : "automations")}
                isActive={mode === "automations"}
                tooltip="自动化"
              >
                <Timer />
                <span>自动化</span>
              </SidebarMenuButton>
            </SidebarMenuItem>
          </SidebarMenu>
        </SidebarGroup>

        <SidebarGroup>
          <div className="px-3 py-2 group-data-[collapsible=icon]:hidden">
            <div className="relative">
              <Search className="text-secondary absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2" />
              <Input
                placeholder="搜索会话..."
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                className="text-caption h-8 pl-8"
              />
            </div>
          </div>
        </SidebarGroup>

        <SidebarGroup className="min-h-0 flex-1">
          <SidebarMenu>
            {filteredThreads.length === 0 ? (
              <div className="text-secondary text-caption py-8 text-center group-data-[collapsible=icon]:hidden">
                {search.trim() ? "未找到匹配会话" : "暂无会话"}
              </div>
            ) : (
              filteredThreads.map((t) => (
                <SidebarMenuItem key={t.id} className="group/item">
                  <SidebarMenuButton
                    isActive={activeThreadId === t.id}
                    onClick={() => {
                      setMode("chat")
                      onSelectThread(t.id)
                    }}
                    tooltip={t.name || t.first_message || t.id}
                    className={cn(
                      "relative transition-colors",
                      "data-[active=true]:before:bg-brand data-[active=true]:before:absolute data-[active=true]:before:inset-y-1.5 data-[active=true]:before:left-0 data-[active=true]:before:w-0.5 data-[active=true]:before:rounded-full"
                    )}
                  >
                    <span
                      className={`size-1.5 shrink-0 rounded-full ${activeThreadId === t.id ? "bg-up" : "bg-ink-muted"}`}
                    />
                    <span className="flex-1 truncate">{t.name || t.first_message || t.id}</span>
                  </SidebarMenuButton>
                  <Button
                    variant="ghost"
                    size="icon-xs"
                    className="hover:text-down text-secondary focus-visible:opacity-100 absolute top-1/2 right-1.5 z-10 -translate-y-1/2 opacity-0 group-focus-within/item:opacity-100 group-hover/item:opacity-100"
                    onClick={(e) => {
                      e.stopPropagation()
                      onDeleteThread(t.id)
                    }}
                    title="删除"
                    aria-label={`删除会话 ${t.name || t.id}`}
                  >
                    <Trash2 />
                  </Button>
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
  )
}
