import { lazy, Suspense, useEffect, useCallback, useState } from "react"
import { createPortal } from "react-dom"
import {
  ChevronLeft,
  ChevronRight,
  CirclePlay,
  Clock,
  Gauge,
  Layers,
  ListOrdered,
  Maximize2,
  Rss,
  TrendingUp,
  Trophy,
  X,
} from "lucide-react"
import { motion, AnimatePresence, useDragControls } from "motion/react"
import { Panel, Separator, usePanelRef, type PanelSize } from "react-resizable-panels"
import { Button } from "@/components/ui/button"
import ErrorBoundary from "@/components/ErrorBoundary"
import { Skeleton } from "@/components/ui/skeleton"
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { useRightPanelStore, type RightPanelView } from "@/stores/rightPanelStore"
import { motionTransition } from "@/lib/motion"

const KlineDrawer = lazy(() => import("./KlineDrawer"))
const DragonTigerView = lazy(() => import("@/components/DragonTigerView"))
const ReplayView = lazy(() => import("@/components/ReplayView"))
const StrongStocksTab = lazy(() => import("@/components/StrongStocksTab"))
const PromotionView = lazy(() => import("@/components/PromotionView"))
const PlateRankingTab = lazy(() => import("@/components/PlateRankingTab"))
const StockRankingTab = lazy(() => import("@/components/StockRankingTab"))
const TimelineTab = lazy(() => import("@/components/TimelineTab"))
const SentimentChipTab = lazy(() => import("@/components/SentimentChipTab"))
const NewsFeed = lazy(() => import("@/components/NewsFeed"))

const TABS: { id: RightPanelView; label: string; icon: typeof TrendingUp }[] = [
  { id: "strong-stocks", label: "强势股", icon: TrendingUp },
  { id: "promotion", label: "晋级分析", icon: Layers },
  { id: "plate-ranking", label: "板块排行", icon: ListOrdered },
  { id: "stock-ranking", label: "个股排行", icon: TrendingUp },
  { id: "dragon-tiger", label: "龙虎榜", icon: Trophy },
  { id: "timeline", label: "时间线", icon: Clock },
  { id: "sentiment-chip", label: "情绪筹码", icon: Gauge },
  { id: "news", label: "资讯", icon: Rss },
  { id: "replay", label: "行情回放", icon: CirclePlay },
]

const PANEL_DEFAULT_WIDTH = 400
const PANEL_MIN_WIDTH = 300
const PANEL_MAX_WIDTH = 700
const PANEL_COLLAPSED_WIDTH = 48
const PANEL_COLLAPSED_STORAGE_KEY = "right-panel-collapsed"

function loadCollapsed(): boolean {
  try {
    return localStorage.getItem(PANEL_COLLAPSED_STORAGE_KEY) === "true"
  } catch {
    // localStorage 不可用或被禁用时回退到展开状态
    return false
  }
}

function ViewFallback() {
  return (
    <div className="p-6">
      <Skeleton className="h-24 w-full" />
    </div>
  )
}

function ViewRenderer({ view }: { view: RightPanelView }) {
  switch (view) {
    case "dragon-tiger":
      return <DragonTigerView />
    case "replay":
      return <ReplayView />
    case "strong-stocks":
      return <StrongStocksTab />
    case "promotion":
      return <PromotionView />
    case "plate-ranking":
      return <PlateRankingTab />
    case "stock-ranking":
      return <StockRankingTab />
    case "timeline":
      return <TimelineTab />
    case "sentiment-chip":
      return <SentimentChipTab />
    case "news":
      return <NewsFeed />
  }
}

export default function RightPanel() {
  const activeView = useRightPanelStore((s) => s.activeView)
  const setActiveView = useRightPanelStore((s) => s.setActiveView)
  const klineTarget = useRightPanelStore((s) => s.klineTarget)
  const closeKline = useRightPanelStore((s) => s.closeKline)
  const [popoutView, setPopoutView] = useState<RightPanelView | null>(null)
  const dragControls = useDragControls()
  const panelRef = usePanelRef()
  const [collapsed, setCollapsed] = useState(loadCollapsed)

  // The Panel's imperative API is only wired up after the Group registers
  // itself in the Group's own layout effect. Layout effects run child-first, so
  // reading getSize()/isCollapsed() here during mount throws "Group not found".
  // Seed the UI from storage instead and keep it in sync via onResize, which
  // the library also fires for the initial size.
  useEffect(() => {
    try {
      localStorage.setItem(PANEL_COLLAPSED_STORAGE_KEY, String(collapsed))
    } catch {
      // localStorage 写入失败时仍更新内存状态，保持交互可用
    }
  }, [collapsed])

  const handleResize = useCallback((size: PanelSize) => {
    if (size.inPixels <= 0) return
    setCollapsed(size.inPixels <= PANEL_COLLAPSED_WIDTH + 1)
  }, [])

  useEffect(() => {
    if (!popoutView) return
    const handler = (event: KeyboardEvent) => {
      if (event.key === "Escape") setPopoutView(null)
    }
    document.addEventListener("keydown", handler)
    return () => document.removeEventListener("keydown", handler)
  }, [popoutView])

  useEffect(() => {
    const handler = (e: Event) => {
      const detail = (e as CustomEvent).detail as RightPanelView
      panelRef.current?.expand()
      setActiveView(detail)
    }
    window.addEventListener("open-right-panel", handler)
    return () => window.removeEventListener("open-right-panel", handler)
  }, [panelRef, setActiveView])

  const toggleCollapse = useCallback(() => {
    const panel = panelRef.current
    if (!panel) return
    if (panel.isCollapsed()) panel.expand()
    else panel.collapse()
  }, [panelRef])

  return (
    <>
      <Separator className="bg-hairline data-[separator=active]:bg-brand/60 data-[separator=hover]:bg-brand/40 w-1 shrink-0 cursor-col-resize transition-colors" />
      <Panel
        id="right"
        panelRef={panelRef}
        defaultSize={PANEL_DEFAULT_WIDTH}
        minSize={PANEL_MIN_WIDTH}
        maxSize={PANEL_MAX_WIDTH}
        collapsible
        collapsedSize={PANEL_COLLAPSED_WIDTH}
        collapsedThreshold={PANEL_COLLAPSED_WIDTH + 32}
        onResize={handleResize}
        className="min-w-0"
      >
        <div className="bg-surface border-hairline flex h-full flex-col overflow-hidden border-l">
          <div
            className={`border-hairline flex shrink-0 items-center gap-1 border-b p-2 ${
              collapsed ? "justify-center" : ""
            }`}
          >
            <Button
              variant="ghost"
              size="icon-xs"
              onClick={toggleCollapse}
              title={collapsed ? "展开面板" : "折叠面板"}
              aria-label={collapsed ? "展开面板" : "折叠面板"}
            >
              {collapsed ? <ChevronLeft /> : <ChevronRight />}
            </Button>
            {/* 当前视图名由下方 TabsList 的选中态表达，头部不再重复一遍 */}
            {!collapsed && (
              <Button
                variant="ghost"
                size="icon-xs"
                className="ml-auto"
                onClick={() => setPopoutView(activeView)}
                title="弹出到独立窗口"
                aria-label="弹出到独立窗口"
              >
                <Maximize2 />
              </Button>
            )}
          </div>

          {collapsed ? (
            <div className="border-hairline flex flex-1 flex-col items-center gap-1 overflow-y-auto border-t py-2">
              {TABS.map(({ id, label, icon: Icon }) => (
                <Button
                  key={id}
                  variant="ghost"
                  size="icon"
                  className="data-[active=true]:text-brand hover:bg-hover size-10 rounded-lg"
                  data-active={activeView === id}
                  title={label}
                  aria-label={label}
                  onClick={() => {
                    setActiveView(id)
                    toggleCollapse()
                  }}
                >
                  <Icon className="size-4" />
                </Button>
              ))}
            </div>
          ) : (
            <>
              <Tabs
                value={activeView}
                onValueChange={(value) => setActiveView(value as RightPanelView)}
                className="shrink-0 gap-0"
              >
                <TabsList
                  variant="line"
                  className="border-hairline no-scrollbar h-auto w-full shrink-0 justify-start gap-0 overflow-x-auto rounded-none border-b bg-transparent px-3 py-0"
                >
                  {TABS.map((tab) => (
                    <TabsTrigger
                      key={tab.id}
                      value={tab.id}
                      className="text-caption data-active:border-b-brand data-active:text-brand flex-none rounded-none border-b-2 border-transparent px-3 py-2 whitespace-nowrap after:hidden"
                    >
                      {tab.label}
                    </TabsTrigger>
                  ))}
                </TabsList>
              </Tabs>

              <div className="relative flex flex-1 overflow-hidden">
                <div className="flex-1 overflow-y-auto">
                  <ErrorBoundary name="RightPanel">
                    <Suspense fallback={<ViewFallback />}>
                      <ViewRenderer view={activeView} />
                    </Suspense>
                  </ErrorBoundary>
                </div>

                <AnimatePresence>
                  {klineTarget && (
                    <div className="bg-surface absolute inset-0 z-10 overflow-y-auto">
                      <Suspense fallback={<ViewFallback />}>
                        <KlineDrawer
                          symbol={klineTarget.symbol}
                          name={klineTarget.name}
                          onClose={closeKline}
                        />
                      </Suspense>
                    </div>
                  )}
                </AnimatePresence>
              </div>
            </>
          )}
        </div>
      </Panel>

      {/* Custom draggable popout (not a Dialog): DialogContent is centered and
          fixed, while this window can be dragged aside. Content remains
          standard shadcn controls. */}
      {createPortal(
        <AnimatePresence>
          {popoutView && (
            <motion.div
              className="bg-scrim fixed inset-0 z-50 flex items-center justify-center"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              onClick={() => setPopoutView(null)}
            >
              <motion.div
                role="dialog"
                aria-modal="true"
                aria-label={TABS.find((t) => t.id === popoutView)?.label}
                className="bg-surface border-hairline shadow-modal flex h-[min(600px,90vh)] w-[min(896px,90vw)] flex-col rounded-lg border"
                drag
                dragMomentum={false}
                dragListener={false}
                dragControls={dragControls}
                initial={{ opacity: 0, scale: 0.95 }}
                animate={{ opacity: 1, scale: 1 }}
                exit={{ opacity: 0, scale: 0.95 }}
                transition={motionTransition.fast}
                onClick={(e) => e.stopPropagation()}
              >
                <div
                  className="border-hairline flex cursor-grab items-center justify-between border-b px-4 py-3 active:cursor-grabbing"
                  onPointerDown={(e) => {
                    if (e.target instanceof HTMLElement && e.target.closest("button")) return
                    dragControls.start(e)
                  }}
                >
                  <span className="text-body text-primary font-medium">
                    {TABS.find((t) => t.id === popoutView)?.label}
                  </span>
                  <Button
                    autoFocus
                    variant="ghost"
                    size="icon-xs"
                    aria-label="关闭弹窗"
                    onClick={() => setPopoutView(null)}
                    title="关闭"
                  >
                    <X className="size-4" />
                  </Button>
                </div>
                <div className="flex-1 overflow-y-auto p-6">
                  <ErrorBoundary name="RightPanelPopout">
                    <Suspense fallback={<ViewFallback />}>
                      <ViewRenderer view={popoutView} />
                    </Suspense>
                  </ErrorBoundary>
                </div>
              </motion.div>
            </motion.div>
          )}
        </AnimatePresence>,
        document.body
      )}
    </>
  )
}
