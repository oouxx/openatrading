import { useCallback } from "react"
import { useAppMode } from "@/contexts/AppModeContext"
import AiCopilot from "@/components/ChatView"
import ErrorBoundary from "@/components/ErrorBoundary"
import ConversationSidebar from "./ConversationSidebar"
import { useThreads } from "@/hooks/useChatThreads"
import { useStockTabStore } from "@/stores/stockTabStore"
import { useThreadStore } from "@/stores/threadStore"
import { useUiStore } from "@/stores/ui"
import RightPanel from "./RightPanel"
import SkillEditor from "./SkillEditor"
import RoleView from "@/components/RoleView"
import WatchlistView from "./WatchlistView"
import AutomationsView from "./AutomationsView"
import SettingsView from "./SettingsView"
import SettingsSidebar from "./SettingsSidebar"
import { SidebarInset, SidebarProvider, SidebarTrigger } from "@/components/ui/sidebar"
import { TooltipProvider } from "@/components/ui/tooltip"

export default function AppShell() {
  const { mode, setMode } = useAppMode()
  const settingsTab = useUiStore((s) => s.settingsTab)
  const setSettingsTab = useUiStore((s) => s.setSettingsTab)
  const activeThreadId = useThreadStore((s) => s.activeThreadId)
  const setActiveThreadId = useThreadStore((s) => s.setActiveThreadId)
  const { sessions, deleteSession, reload } = useThreads()

  const handleNewThread = () => {
    setActiveThreadId(null)
    useStockTabStore.getState().setActiveStockTab(null)
    setMode("chat")
  }

  const handleDeleteThread = useCallback(
    async (id: string) => {
      await deleteSession(id)
      if (activeThreadId === id) setActiveThreadId(null)
      reload()
    },
    [activeThreadId, deleteSession, reload, setActiveThreadId]
  )

  const handleActiveChange = useCallback(
    (id: string | null) => {
      setActiveThreadId(id)
      useStockTabStore.getState().setActiveStockTab(null)
      reload()
    },
    [reload, setActiveThreadId]
  )

  return (
    <TooltipProvider>
      <SidebarProvider defaultOpen className="h-svh">
        {mode === "settings" ? (
          <SettingsSidebar activeTab={settingsTab} onTabChange={setSettingsTab} />
        ) : (
          <ConversationSidebar
            threads={sessions}
            activeThreadId={activeThreadId}
            onSelectThread={handleActiveChange}
            onNewThread={handleNewThread}
            onDeleteThread={handleDeleteThread}
          />
        )}
        <SidebarInset className="overflow-hidden">
          {mode === "chat" ? (
            <div className="flex h-svh min-w-0 flex-1 flex-row">
              <div className="flex min-w-0 flex-1 flex-col">
                <ErrorBoundary name="AiCopilot">
                  <AiCopilot
                    activeThreadId={activeThreadId}
                    onActiveThreadChange={handleActiveChange}
                    onReload={reload}
                  />
                </ErrorBoundary>
              </div>
              <div className="hidden md:block"><RightPanel /></div>
            </div>
          ) : mode === "skill-editor" ? (
            <div className="flex h-full min-w-0 flex-1 flex-col">
              <div className="border-hairline flex items-center gap-2 border-b px-3 py-2">
                <SidebarTrigger />
              </div>
              <div className="flex-1 overflow-y-auto">
                <SkillEditor />
              </div>
            </div>
          ) : mode === "roles" ? (
            <div className="flex h-full min-w-0 flex-1 flex-col">
              <div className="border-hairline flex items-center gap-2 border-b px-3 py-2">
                <SidebarTrigger />
              </div>
              <div className="flex-1 overflow-y-auto">
                <RoleView />
              </div>
            </div>
          ) : mode === "automations" ? (
            <div className="flex h-full min-w-0 flex-1 flex-col">
              <div className="border-hairline flex items-center gap-2 border-b px-3 py-2">
                <SidebarTrigger />
              </div>
              <div className="flex-1 overflow-y-auto">
                <AutomationsView />
              </div>
            </div>
          ) : mode === "watchlist" ? (
            <div className="flex h-full min-w-0 flex-1 flex-col">
              <div className="border-hairline flex items-center gap-2 border-b px-3 py-2">
                <SidebarTrigger />
              </div>
              <div className="flex-1 overflow-y-auto">
                <WatchlistView />
              </div>
            </div>
          ) : (
            <div className="flex h-full min-w-0 flex-1 flex-col">
              <div className="border-hairline flex items-center gap-2 border-b px-3 py-2">
                <SidebarTrigger />
              </div>
              <div className="flex-1 overflow-y-auto">
                <SettingsView activeTab={settingsTab} />
              </div>
            </div>
          )}
        </SidebarInset>
      </SidebarProvider>
    </TooltipProvider>
  )
}
