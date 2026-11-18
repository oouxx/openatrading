import { useMemo, useState } from "react"
import { Input } from "@/components/ui/input"
import { Button } from "@/components/ui/button"
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
} from "@/components/ui/sidebar"
import { Search, Plus, Settings, Puzzle, Users, Star, Trash2 } from "lucide-react"
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

const WORKSPACE_NAV = [
  { mode: "skill-editor", label: "技能", icon: Puzzle },
  { mode: "roles", label: "模拟盘", icon: Users },
  { mode: "watchlist", label: "自选", icon: Star },
] as const

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
      <SidebarHeader className="gap-2 px-3 pt-3 pb-1">
        <div className="text-primary flex h-8 items-center gap-2.5 px-1 group-data-[collapsible=icon]:justify-center">
          <img src="/app-icon.png" alt="" className="size-5 rounded-sm" />
          <span className="text-body font-semibold tracking-tight group-data-[collapsible=icon]:hidden">
            openatrading
          </span>
        </div>

        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton
              onClick={onNewThread}
              tooltip="新建会话"
              className="border-hairline bg-surface hover:bg-hover hover:border-secondary/25 h-9 rounded-md border transition-colors"
            >
              <Plus className="text-brand" />
              <span className="text-primary font-medium">新建会话</span>
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarHeader>

      <SidebarContent>
        <SidebarGroup className="pt-3 pb-1">
          <SidebarGroupLabel className="text-tertiary text-micro mb-1 px-2 font-medium">
            工作台
          </SidebarGroupLabel>
          <SidebarMenu>
            {WORKSPACE_NAV.map((item) => {
              const Icon = item.icon
              const active = mode === item.mode
              return (
                <SidebarMenuItem key={item.mode}>
                  <SidebarMenuButton
                    onClick={() => setMode(active ? "chat" : item.mode)}
                    isActive={active}
                    tooltip={item.label}
                    className={cn(
                      "text-secondary hover:text-primary h-9",
                      "data-[active=true]:text-primary data-[active=true]:font-medium"
                    )}
                  >
                    <Icon className={active ? "text-brand" : undefined} />
                    <span>{item.label}</span>
                  </SidebarMenuButton>
                </SidebarMenuItem>
              )
            })}
          </SidebarMenu>
        </SidebarGroup>

        <SidebarGroup className="min-h-0 flex-1 pt-4">
          <SidebarGroupLabel className="text-tertiary text-micro mb-1 px-2 font-medium">
            会话
          </SidebarGroupLabel>

          <div className="px-0 pb-2 group-data-[collapsible=icon]:hidden">
            <div className="relative">
              <Search className="text-tertiary pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2" />
              <Input
                placeholder="搜索会话…"
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                className="text-caption h-8 rounded-md pl-8"
              />
            </div>
          </div>

          <SidebarMenu>
            {filteredThreads.length === 0 ? (
              <p className="text-tertiary text-caption px-2 py-6 text-center leading-relaxed group-data-[collapsible=icon]:hidden">
                {search.trim() ? "没有匹配的会话" : "还没有会话"}
              </p>
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
                      "text-secondary hover:text-primary h-9",
                      "data-[active=true]:bg-hover data-[active=true]:text-primary data-[active=true]:font-medium"
                    )}
                  >
                    <span className="truncate">{t.name || t.first_message || t.id}</span>
                  </SidebarMenuButton>
                  <Button
                    variant="ghost"
                    size="icon-xs"
                    className="text-tertiary hover:text-danger absolute top-1/2 right-1.5 z-10 -translate-y-1/2 opacity-0 transition-opacity group-focus-within/item:opacity-100 group-hover/item:opacity-100 focus-visible:opacity-100"
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

      <SidebarFooter className="border-hairline border-t px-3 py-2">
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton
              onClick={() => setMode(mode === "settings" ? "chat" : "settings")}
              isActive={mode === "settings"}
              tooltip="设置"
              className="text-secondary hover:text-primary data-[active=true]:text-primary h-9"
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
