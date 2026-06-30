import { useEffect, useCallback } from "react"
import { ChevronLeft, ChevronRight } from "lucide-react"
import { AnimatePresence } from "framer-motion"
import { Button } from "@/components/ui/button"
import { usePanelWidth } from "@/hooks/usePanelWidth"
import { useRightPanelStore, type RightPanelView } from "@/stores/rightPanelStore"
import ResizableHandle from "./ResizableHandle"
import KlineDrawer from "./KlineDrawer"
import DragonTigerView from "@/DragonTigerView"
import BreakoutView from "@/BreakoutView"
import ReplayView from "@/ReplayView"
import RoleView from "@/RoleView"
import StrongStocksTab from "@/components/tabs/StrongStocksTab"
import PromotionTab from "@/components/tabs/PromotionTab"
import PlateTab from "@/components/tabs/PlateTab"
import TimelineTab from "@/components/tabs/TimelineTab"
import SentimentChipTab from "@/components/tabs/SentimentChipTab"
import MarketIntelTab from "@/components/tabs/MarketIntelTab"
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
  { id: "roles", label: "角色策略" },
]

export default function RightPanel() {
  const activeView = useRightPanelStore((s) => s.activeView)
  const setActiveView = useRightPanelStore((s) => s.setActiveView)
  const klineTarget = useRightPanelStore((s) => s.klineTarget)
  const closeKline = useRightPanelStore((s) => s.closeKline)
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
      case "roles":
        return <RoleView />
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
            <span className="flex-1 truncate text-xs font-medium text-white">
              {TABS.find((t) => t.id === activeView)?.label}
            </span>
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
                  <div className="absolute inset-0 z-10 bg-bg-surface overflow-y-auto">
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
    </div>
  )
}
