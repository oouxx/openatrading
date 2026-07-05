import { useEffect, useCallback, useState, useRef } from "react"
import { ChevronLeft, ChevronRight, Maximize2, X } from "lucide-react"
import { motion, AnimatePresence } from "framer-motion"
import { Button } from "@/components/ui/button"
import { usePanelWidth } from "@/hooks/usePanelWidth"
import { useRightPanelStore, type RightPanelView } from "@/stores/rightPanelStore"
import ResizableHandle from "./ResizableHandle"
import KlineDrawer from "./KlineDrawer"
import DragonTigerView from "@/components/DragonTigerView"
import BreakoutView from "@/components/BreakoutView"
import ReplayView from "@/components/ReplayView"
import StrongStocksTab from "@/components/StrongStocksTab"
import PromotionTab from "@/components/PromotionTab"
import PlateTab from "@/components/PlateTab"
import TimelineTab from "@/components/TimelineTab"
import SentimentChipTab from "@/components/SentimentChipTab"
import MarketIntelTab from "@/components/MarketIntelTab"
import NewsFeed from "@/components/NewsFeed"

const TABS: { id: RightPanelView; label: string }[] = [
  { id: "strong-stocks", label: "强势股" },
  { id: "breakout", label: "晋级分析1" },
  { id: "promotion", label: "晋级分析2" },
  { id: "plate", label: "板块异动" },
  { id: "dragon-tiger", label: "龙虎榜" },
  { id: "timeline", label: "时间线" },
  { id: "sentiment-chip", label: "情绪筹码" },
  { id: "market-intel", label: "市场情报" },
  { id: "news", label: "资讯" },
  { id: "replay", label: "行情回放" },
]

export default function RightPanel() {
  const activeView = useRightPanelStore((s) => s.activeView)
  const setActiveView = useRightPanelStore((s) => s.setActiveView)
  const klineTarget = useRightPanelStore((s) => s.klineTarget)
  const closeKline = useRightPanelStore((s) => s.closeKline)
  const [popoutView, setPopoutView] = useState<RightPanelView | null>(null)
  const [popoutSize, setPopoutSize] = useState({ w: 896, h: 600 })
  const isResizing = useRef(false)
  const { width, setWidth, collapsed, toggleCollapse } = usePanelWidth(
    "layout:right-width",
    400,
    300,
    700,
    "layout:right-collapsed"
  )

  useEffect(() => {
    const handler = (e: Event) => {
      const detail = (e as CustomEvent).detail as RightPanelView
      if (collapsed) toggleCollapse()
      setActiveView(detail)
    }
    window.addEventListener("open-right-panel", handler)
    return () => window.removeEventListener("open-right-panel", handler)
  }, [collapsed, toggleCollapse, setActiveView])

  const handleResize = useCallback((delta: number) => setWidth((prev) => prev + delta), [setWidth])

  const renderView = () => {
    switch (activeView) {
      case "dragon-tiger":
        return <DragonTigerView />
      case "breakout":
        return <BreakoutView />
      case "replay":
        return <ReplayView />
      case "strong-stocks":
        return <StrongStocksTab />
      case "promotion":
        return <PromotionTab stocks={[]} onOpenKline={() => {}} />
      case "plate":
        return <PlateTab />
      case "timeline":
        return <TimelineTab />
      case "sentiment-chip":
        return <SentimentChipTab />
      case "market-intel":
        return <MarketIntelTab />
      case "news":
        return <NewsFeed />
    }
  }

  return (
    <div className="relative flex h-full flex-shrink-0">
      {!collapsed && <ResizableHandle onResize={handleResize} />}
      <div
        className="border-hairline bg-bg-surface flex flex-col overflow-hidden border-l transition-[width] duration-150"
        style={{ width: collapsed ? 48 : width }}
      >
        <div className="border-hairline flex flex-shrink-0 items-center gap-1 border-b p-2">
          <Button
            variant="ghost"
            size="icon"
            className="size-6"
            onClick={toggleCollapse}
            title={collapsed ? "展开面板" : "折叠面板"}
          >
            {collapsed ? <ChevronLeft className="size-3" /> : <ChevronRight className="size-3" />}
          </Button>
          {!collapsed && (
            <>
              <span className="flex-1 truncate text-xs font-medium text-white">
                {TABS.find((t) => t.id === activeView)?.label}
              </span>
              <button
                className="text-ink-muted hover:text-white ml-1 cursor-pointer"
                onClick={() => setPopoutView(activeView)}
                title="弹出到独立窗口"
              >
                <Maximize2 className="size-3" />
              </button>
            </>
          )}
        </div>

        {collapsed ? (
          <div className="border-hairline flex flex-1 flex-col items-center gap-1 overflow-y-auto border-t py-2">
            {TABS.map((tab) => (
              <button
                key={tab.id}
                className="rounded-card hover:bg-bg-hover data-[active=true]:text-ai flex size-10 items-center justify-center text-[10px] transition-colors"
                data-active={activeView === tab.id}
                title={tab.label}
                onClick={() => {
                  setActiveView(tab.id)
                  toggleCollapse()
                }}
              >
                {tab.label.slice(0, 2)}
              </button>
            ))}
          </div>
        ) : (
          <>
            <div className="border-hairline flex flex-shrink-0 gap-0 overflow-x-auto border-b px-3">
              {TABS.map((tab) => (
                <button
                  key={tab.id}
                  className="data-[active=false]:text-ink-muted data-[active=true]:border-ai data-[active=true]:text-ai border-b-2 px-3 py-1.5 text-xs whitespace-nowrap transition-colors data-[active=false]:border-transparent data-[active=false]:hover:text-white"
                  data-active={activeView === tab.id}
                  onClick={() => setActiveView(tab.id)}
                >
                  {tab.label}
                </button>
              ))}
            </div>

            <div className="relative flex flex-1 overflow-hidden">
              <div className="flex-1 overflow-y-auto">{renderView()}</div>

              <AnimatePresence>
                {klineTarget && (
                  <div className="bg-bg-surface absolute inset-0 z-10 overflow-y-auto">
                    <KlineDrawer
                      symbol={klineTarget.symbol}
                      name={klineTarget.name}
                      onClose={closeKline}
                    />
                  </div>
                )}
              </AnimatePresence>
            </div>
          </>
        )}
      </div>

      <AnimatePresence>
        {popoutView && (
          <motion.div
            className="fixed inset-0 z-50 flex items-center justify-center bg-black/60"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            onClick={() => { if (!isResizing.current) setPopoutView(null) }}
          >
            <motion.div
              className="border-hairline bg-bg-surface relative flex flex-col rounded-lg border shadow-2xl"
              style={{ width: popoutSize.w, height: popoutSize.h, maxWidth: "90vw", maxHeight: "90vh" }}
              drag
              dragMomentum={false}
              dragHandleClassName="modal-drag-handle"
              initial={{ opacity: 0, scale: 0.95 }}
              animate={{ opacity: 1, scale: 1 }}
              exit={{ opacity: 0, scale: 0.95 }}
              transition={{ duration: 0.2 }}
              onClick={(e) => e.stopPropagation()}
            >
              <div
                className="modal-drag-handle border-hairline flex cursor-grab items-center justify-between border-b px-4 py-3 active:cursor-grabbing"
              >
                <span className="text-sm font-medium text-white">
                  {TABS.find((t) => t.id === popoutView)?.label}
                </span>
                <button
                  className="text-ink-muted hover:text-white cursor-pointer"
                  onClick={() => setPopoutView(null)}
                >
                  <X className="size-4" />
                </button>
              </div>
              <div className="flex-1 overflow-y-auto p-4">
                {popoutView === "dragon-tiger" && <DragonTigerView />}
                {popoutView === "breakout" && <BreakoutView />}
                {popoutView === "replay" && <ReplayView />}
                {popoutView === "strong-stocks" && <StrongStocksTab />}
                {popoutView === "promotion" && <PromotionTab stocks={[]} onOpenKline={() => {}} />}
                {popoutView === "plate" && <PlateTab />}
                {popoutView === "timeline" && <TimelineTab />}
                {popoutView === "sentiment-chip" && <SentimentChipTab />}
                {popoutView === "market-intel" && <MarketIntelTab />}
                {popoutView === "news" && <NewsFeed />}
              </div>
              {/* resize handle */}
              <div
                className="absolute right-0 bottom-0 z-10 cursor-se-resize"
                style={{ width: 20, height: 20 }}
                onMouseDown={(e) => {
                  e.preventDefault()
                  e.stopPropagation()
                  isResizing.current = true
                  const startX = e.clientX
                  const startY = e.clientY
                  const startW = popoutSize.w
                  const startH = popoutSize.h
                  document.body.style.userSelect = "none"
                  const onMove = (ev: MouseEvent) => {
                    setPopoutSize({
                      w: Math.max(480, startW + ev.clientX - startX),
                      h: Math.max(300, startH + ev.clientY - startY),
                    })
                  }
                  const onUp = () => {
                    document.body.style.userSelect = ""
                    window.removeEventListener("mousemove", onMove)
                    window.removeEventListener("mouseup", onUp)
                    // Reset after click event had a chance to fire
                    setTimeout(() => { isResizing.current = false }, 0)
                  }
                  window.addEventListener("mousemove", onMove)
                  window.addEventListener("mouseup", onUp)
                }}
                onMouseUp={(e) => e.stopPropagation()}
                onClick={(e) => e.stopPropagation()}
              >
                <svg viewBox="0 0 10 10" className="text-ink-muted size-4">
                  <path d="M10 0v10H0" stroke="currentColor" strokeWidth="1.5" fill="none" />
                </svg>
              </div>
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  )
}
