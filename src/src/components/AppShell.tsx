import { useState, useCallback } from "react"
import { useAppMode } from "@/contexts/AppModeContext"
import AiCopilot from "@/AiCopilot"
import ConversationSidebar from "./ConversationSidebar"
import { useThreads } from "@/hooks/useChatThreads"
import { useStockTabStore } from "@/stores/stockTabStore"
import { useThreadStore } from "@/stores/threadStore"
import RightPanel from "./RightPanel"
import SkillEditor from "./SkillEditor"
import RoleView from "@/RoleView"
import WatchlistView from "./WatchlistView"
import SettingsView from "./SettingsView"
import SettingsSidebar from "./SettingsSidebar"
import type { SettingsTab } from "./SettingsSidebar"
import { SidebarInset, SidebarProvider, SidebarTrigger } from "@/components/ui/sidebar"
import { TooltipProvider } from "@/components/ui/tooltip"

export default function AppShell() {
  const { mode } = useAppMode()
  const [settingsTab, setSettingsTab] = useState<SettingsTab>("appearance")
  const activeThreadId = useThreadStore((s) => s.activeThreadId)
  const setActiveThreadId = useThreadStore((s) => s.setActiveThreadId)
  const { threads, createThread, deleteThread, reload } = useThreads()

  const handleNewThread = async () => {
    const thread = await createThread("新对话")
    setActiveThreadId(thread.id)
  }

  const handleDeleteThread = useCallback(
    async (id: number) => {
      await deleteThread(id)
      if (activeThreadId === id) setActiveThreadId(null)
      reload()
    },
    [activeThreadId, deleteThread, reload]
  )

  const handleActiveChange = useCallback(
    (id: number | null) => {
      setActiveThreadId(id)
      useStockTabStore.getState().setActiveStockTab(null)
      reload()
    },
    [reload]
  )

  return (
    <TooltipProvider>
      <SidebarProvider defaultOpen>
        {mode === "settings" ? (
          <SettingsSidebar activeTab={settingsTab} onTabChange={setSettingsTab} />
        ) : (
          <ConversationSidebar
            threads={threads}
            activeThreadId={activeThreadId}
            onSelectThread={handleActiveChange}
            onNewThread={handleNewThread}
            onDeleteThread={handleDeleteThread}
          />
        )}
        <SidebarInset className="overflow-hidden">
          {mode === "chat" ? (
            <div className="flex h-full max-h-screen min-w-0 flex-1 flex-row">
              <div className="flex min-w-0 flex-1 flex-col">
                <AiCopilot
                  activeThreadId={activeThreadId}
                  onActiveThreadChange={handleActiveChange}
                  onReload={reload}
                />
              </div>
              <RightPanel />
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
