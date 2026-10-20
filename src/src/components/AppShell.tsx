import { lazy, Suspense, useCallback, useMemo, type ReactNode } from "react"
import { Group, Panel, useDefaultLayout } from "react-resizable-panels"
import { useAppMode } from "@/contexts/useAppMode"
import AiCopilot from "@/components/ChatView"
import ErrorBoundary from "@/components/ErrorBoundary"
import ConversationSidebar from "./ConversationSidebar"
import { useThreads } from "@/hooks/useChatThreads"
import { useStockTabStore } from "@/stores/stockTabStore"
import { useThreadStore } from "@/stores/threadStore"
import { useUiStore } from "@/stores/ui"
import RightPanel from "./RightPanel"
import SettingsSidebar from "./SettingsSidebar"
import { SidebarInset, SidebarProvider, SidebarTrigger } from "@/components/ui/sidebar"
import { Skeleton } from "@/components/ui/skeleton"
import { TooltipProvider } from "@/components/ui/tooltip"
import { useIsMobile } from "@/hooks/use-mobile"

const SkillEditor = lazy(() => import("./SkillEditor"))
const RoleView = lazy(() => import("@/components/RoleView"))
const WatchlistView = lazy(() => import("./WatchlistView"))
const AutomationsView = lazy(() => import("./AutomationsView"))
const SettingsView = lazy(() => import("./SettingsView"))

function ViewFallback() {
  return (
    <div className="p-4">
      <Skeleton className="h-24 w-full" />
    </div>
  )
}

function LazyView({ children }: { children: ReactNode }) {
  return (
    <div className="flex h-full min-w-0 flex-1 flex-col">
      <div className="border-hairline flex items-center gap-2 border-b px-3 py-2">
        <SidebarTrigger />
      </div>
      <div className="flex-1 overflow-y-auto">
        <ErrorBoundary name="View">
          <Suspense fallback={<ViewFallback />}>{children}</Suspense>
        </ErrorBoundary>
      </div>
    </div>
  )
}

export default function AppShell() {
  const { mode, setMode } = useAppMode()
  const settingsTab = useUiStore((s) => s.settingsTab)
  const setSettingsTab = useUiStore((s) => s.setSettingsTab)
  const activeThreadId = useThreadStore((s) => s.activeThreadId)
  const setActiveThreadId = useThreadStore((s) => s.setActiveThreadId)
  const isMobile = useIsMobile()
  // Panel ids change when the right panel is hidden on mobile; persist each
  // combination separately so desktop widths survive a narrow-window visit.
  const panelIds = useMemo(() => (isMobile ? ["chat"] : ["chat", "right"]), [isMobile])
  const { defaultLayout, onLayoutChanged } = useDefaultLayout({
    id: "chat-layout",
    panelIds,
  })
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
            <Group
              key={panelIds.join("-")}
              id="chat-layout"
              orientation="horizontal"
              defaultLayout={defaultLayout}
              onLayoutChanged={onLayoutChanged}
              className="h-svh min-w-0 flex-1"
            >
              <Panel id="chat" minSize="320px" className="min-w-0">
                <div className="flex h-full min-w-0 flex-col">
                  <ErrorBoundary name="AiCopilot">
                    <AiCopilot
                      activeThreadId={activeThreadId}
                      onActiveThreadChange={handleActiveChange}
                      onReload={reload}
                    />
                  </ErrorBoundary>
                </div>
              </Panel>
              {!isMobile && <RightPanel />}
            </Group>
          ) : mode === "skill-editor" ? (
            <LazyView>
              <SkillEditor />
            </LazyView>
          ) : mode === "roles" ? (
            <LazyView>
              <RoleView />
            </LazyView>
          ) : mode === "automations" ? (
            <LazyView>
              <AutomationsView />
            </LazyView>
          ) : mode === "watchlist" ? (
            <LazyView>
              <WatchlistView />
            </LazyView>
          ) : (
            <LazyView>
              <SettingsView activeTab={settingsTab} />
            </LazyView>
          )}
        </SidebarInset>
      </SidebarProvider>
    </TooltipProvider>
  )
}
