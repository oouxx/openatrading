import { lazy, Suspense, useCallback, useMemo, type ReactNode } from "react"
import { motion } from "motion/react"
import { useAppMode } from "@/contexts/useAppMode"
import AiCopilot from "@/components/ChatView"
import ErrorBoundary from "@/components/ErrorBoundary"
import ConversationSidebar from "./ConversationSidebar"
import LayoutNodeView from "./LayoutNodeView"
import LayoutToast from "./LayoutToast"
import { useThreads } from "@/hooks/useChatThreads"
import { useLayoutHydration } from "@/hooks/useLayoutHydration"
import { useWorkspaceShortcuts } from "@/hooks/useWorkspaceShortcuts"
import { useThreadStore } from "@/stores/threadStore"
import { useUiStore } from "@/stores/ui"
import {
  useActiveLayout,
  useLayoutStore,
  type PaneLeaf,
  type WorkspaceLayout,
} from "@/stores/layoutStore"
import { renderWorkspacePane } from "@/lib/paneRegistry"
import SettingsSidebar from "./SettingsSidebar"
import { SidebarInset, SidebarProvider, SidebarTrigger } from "@/components/ui/sidebar"
import { Skeleton } from "@/components/ui/skeleton"
import { TooltipProvider } from "@/components/ui/tooltip"
import { useIsMobile } from "@/hooks/use-mobile"
import { motionTransition } from "@/lib/motion"

const RoleView = lazy(() => import("@/components/RoleView"))
const WatchlistView = lazy(() => import("./WatchlistView"))
const SettingsView = lazy(() => import("./SettingsView"))

/** 窄屏只能单栏时的兜底布局：只有对话，行情仍由对话头部的按钮唤出。 */
const COMPACT_LAYOUT: WorkspaceLayout = {
  id: "compact",
  name: "紧凑",
  builtin: true,
  storageId: "chat-layout",
  root: { id: "chat", type: "pane", kind: "chat" },
}

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
      {/* 只有侧栏开关，不再铺一条带分割线的空工具条——页面自己的标题承担层级 */}
      <div className="flex h-10 shrink-0 items-center gap-2 px-3">
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
  const activeLayout = useActiveLayout()
  const presetKinds = useLayoutStore((s) => s.presetKinds)
  useWorkspaceShortcuts()
  useLayoutHydration()

  const layout = useMemo(() => (isMobile ? COMPACT_LAYOUT : activeLayout), [isMobile, activeLayout])

  const { sessions, deleteSession, reload } = useThreads()

  const handleNewThread = () => {
    setActiveThreadId(null)
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

  // 切会话只改选中项：会话列表本身没变，原来这里的 reload() 会每点一次都拉一遍
  // /api/ai/sessions 并 setState，把整棵树（含消息列表）再渲染一遍。
  const handleActiveChange = useCallback(
    (id: string | null) => {
      setActiveThreadId(id)
    },
    [setActiveThreadId]
  )

  // 面板类型 → 内容由注册表决定（lib/paneRegistry）。加一种新面板不用改这里，
  // AppShell 只负责提供对话节点这份"外部句柄"。
  const paneContext = {
    chat: (
      <ErrorBoundary name="AiCopilot">
        <AiCopilot
          activeThreadId={activeThreadId}
          onActiveThreadChange={handleActiveChange}
          onReload={reload}
        />
      </ErrorBoundary>
    ),
    layout,
    presetKinds,
  }

  const renderPane = (leaf: PaneLeaf) => renderWorkspacePane(leaf, paneContext)

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
            // 换布局就换一棵面板树：面板 id 变了，必须整体重挂载才能拿到新的默认尺寸。
            // 挂载时给一次 200ms 淡入——两套网格直接硬切会"啪"地跳一下，淡入把
            // 这次重排收成一个动作（只动 opacity，走合成层，不引起重排）。
            <motion.div
              key={layout.id}
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              transition={motionTransition.fast}
              className="h-svh min-w-0 flex-1"
            >
              <LayoutNodeView node={layout.root} layout={layout} renderPane={renderPane} />
            </motion.div>
          ) : mode === "roles" ? (
            <LazyView>
              <RoleView />
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
        <LayoutToast />
      </SidebarProvider>
    </TooltipProvider>
  )
}
