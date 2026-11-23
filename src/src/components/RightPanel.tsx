import { lazy, Suspense, useEffect, useCallback, useRef, useState, type ReactNode } from "react"
import { createPortal } from "react-dom"
import { ChevronLeft, ChevronRight, Maximize2, X } from "lucide-react"
import { motion, AnimatePresence, useDragControls } from "motion/react"
import { Panel, usePanelRef, type PanelSize } from "react-resizable-panels"
import { Button } from "@/components/ui/button"
import { Skeleton } from "@/components/ui/skeleton"
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs"
import MarketView from "@/components/MarketView"
import { RightPanelNavProvider } from "@/components/PaneNav"
import { MARKET_VIEWS, marketViewLabel } from "@/lib/marketViews"
import { useRightPanelStore, type RightPanelView } from "@/stores/rightPanelStore"
import { motionTransition } from "@/lib/motion"
import { useQueryClient } from "@tanstack/react-query"
import { stockRankingQueryOptions } from "@/hooks/query/useStockRanking"

const KlineDrawer = lazy(() => import("./KlineDrawer"))

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

/** K 线抽屉自己的懒加载骨架（行情视图的骨架在 marketViews 里）。 */
function ViewFallback() {
  return (
    <div className="p-6">
      <Skeleton className="h-24 w-full" />
    </div>
  )
}

/**
 * 行情面板（九个视图共用一条 tab 条，可折叠成 48px 图标条）。
 *
 * 它自己就是一个 `Panel`：折叠成图标条要靠 react-resizable-panels 的 collapsible，
 * 外面再套一层 Panel 会变成嵌套面板。所以 AppShell 对 market-tabs 列直接渲染它，
 * 其余列才由 AppShell 统一包 Panel。
 */
export default function RightPanel({
  panelId = "right",
  cornerActions,
}: {
  panelId?: string
  /** 右上角浮层：放大按钮，或结构编辑态下的工具条（由 LayoutNodeView 注入）。 */
  cornerActions?: ReactNode
}) {
  const activeView = useRightPanelStore((s) => s.activeView)
  const setActiveView = useRightPanelStore((s) => s.setActiveView)
  const klineTarget = useRightPanelStore((s) => s.klineTarget)
  const closeKline = useRightPanelStore((s) => s.closeKline)
  const [popoutView, setPopoutView] = useState<RightPanelView | null>(null)
  const dragControls = useDragControls()
  const panelRef = usePanelRef()
  const [collapsed, setCollapsed] = useState(loadCollapsed)
  const tabStripRef = useRef<HTMLDivElement | null>(null)
  const [tabEdges, setTabEdges] = useState({ left: false, right: false })
  const queryClient = useQueryClient()
  const stockRankingOrder = useRightPanelStore((state) => state.stockRankingOrder)
  const stockRankingMarket = useRightPanelStore((state) => state.stockRankingMarket)
  const stockRankingExcludeSt = useRightPanelStore((state) => state.stockRankingExcludeSt)

  const prefetchStockRankings = useCallback(() => {
    void queryClient.prefetchInfiniteQuery(
      stockRankingQueryOptions(stockRankingOrder, stockRankingMarket, stockRankingExcludeSt)
    )
  }, [queryClient, stockRankingOrder, stockRankingMarket, stockRankingExcludeSt])

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

  // The strip holds nine tabs and is narrower than their total width. Track
  // which edges still have hidden tabs so the fade affordance only appears
  // where it means something.
  const syncTabEdges = useCallback(() => {
    const el = tabStripRef.current?.querySelector<HTMLElement>('[data-slot="tabs-list"]')
    if (!el) return
    const left = el.scrollLeft > 2
    const right = el.scrollLeft + el.clientWidth < el.scrollWidth - 2
    setTabEdges((prev) => (prev.left === left && prev.right === right ? prev : { left, right }))
  }, [])

  useEffect(() => {
    const el = tabStripRef.current?.querySelector<HTMLElement>('[data-slot="tabs-list"]')
    if (!el) return
    const observer = new ResizeObserver(syncTabEdges)
    observer.observe(el)
    syncTabEdges()
    return () => observer.disconnect()
  }, [syncTabEdges, collapsed])

  // Keep the active tab visible when switching views from code or shortcuts.
  useEffect(() => {
    const active = tabStripRef.current?.querySelector<HTMLElement>('[data-state="active"]')
    active?.scrollIntoView({ inline: "nearest", block: "nearest" })
    syncTabEdges()
  }, [activeView, syncTabEdges])

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
      <Panel
        id={panelId}
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
        <div className="bg-surface border-hairline relative flex h-full flex-col overflow-hidden border-l">
          {cornerActions}
          <div
            className={`border-hairline flex h-12 shrink-0 items-center gap-1 border-b px-2 ${
              collapsed ? "justify-center" : ""
            }`}
          >
            <Button
              variant="ghost"
              size="icon-xs"
              onClick={toggleCollapse}
              title={collapsed ? "展开面板" : "折叠面板"}
              aria-label={collapsed ? "展开面板" : "折叠面板"}
              className="text-secondary hover:text-primary shrink-0"
            >
              {collapsed ? <ChevronLeft /> : <ChevronRight />}
            </Button>
            {/* 当前视图名由下方 TabsList 的选中态表达，头部不再重复一遍 */}
            {!collapsed && (
              <>
                <div ref={tabStripRef} className="relative min-w-0 flex-1">
                  <Tabs
                    value={activeView}
                    onValueChange={(value) => setActiveView(value as RightPanelView)}
                    className="gap-0"
                  >
                    <TabsList
                      variant="line"
                      onScroll={syncTabEdges}
                      className="h-12 w-full [scrollbar-width:none] justify-start gap-0 overflow-x-auto rounded-none bg-transparent px-0"
                    >
                      {MARKET_VIEWS.map((tab) => (
                        <TabsTrigger
                          key={tab.id}
                          value={tab.id}
                          onPointerEnter={
                            tab.id === "stock-ranking" ? prefetchStockRankings : undefined
                          }
                          onFocus={tab.id === "stock-ranking" ? prefetchStockRankings : undefined}
                          className="text-caption text-secondary data-active:text-brand hover:text-primary data-active:border-b-brand flex-none rounded-none border-b-2 border-transparent px-2.5 py-0 whitespace-nowrap after:hidden"
                        >
                          {tab.label}
                        </TabsTrigger>
                      ))}
                    </TabsList>
                  </Tabs>
                  {tabEdges.left && (
                    <div
                      className="from-surface pointer-events-none absolute inset-y-0 left-0 w-5 bg-linear-to-r to-transparent"
                      aria-hidden
                    />
                  )}
                  {tabEdges.right && (
                    <div
                      className="from-surface pointer-events-none absolute inset-y-0 right-0 w-5 bg-linear-to-l to-transparent"
                      aria-hidden
                    />
                  )}
                </div>
                <Button
                  variant="ghost"
                  size="icon-xs"
                  className="text-secondary hover:text-primary shrink-0"
                  onClick={() => setPopoutView(activeView)}
                  title="弹出到独立窗口"
                  aria-label="弹出到独立窗口"
                >
                  <Maximize2 />
                </Button>
              </>
            )}
          </div>

          {collapsed ? (
            <div className="flex flex-1 flex-col items-center gap-0.5 overflow-y-auto py-3">
              {MARKET_VIEWS.map(({ id, label, icon: Icon }) => (
                <Button
                  key={id}
                  variant="ghost"
                  size="icon"
                  className="data-[active=true]:text-brand data-[active=true]:bg-brand/10 hover:bg-hover size-9 rounded-md"
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
            <div className="relative flex flex-1 overflow-hidden">
              <div className="flex-1 overflow-y-auto">
                <RightPanelNavProvider>
                  <MarketView view={activeView} boundary="RightPanel" />
                </RightPanelNavProvider>
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
                aria-label={marketViewLabel(popoutView)}
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
                    {marketViewLabel(popoutView)}
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
                <div className="relative flex-1 overflow-y-auto">
                  <RightPanelNavProvider>
                    <MarketView view={popoutView} boundary="RightPanelPopout" />
                  </RightPanelNavProvider>
                  {/* 弹出窗口里的表格点股票也要就地出 K 线，否则点击会落到背后那个面板里 */}
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
