import { lazy, Suspense } from "react"
import ErrorBoundary from "@/components/ErrorBoundary"
import { Skeleton } from "@/components/ui/skeleton"
import type { RightPanelView } from "@/stores/rightPanelStore"

// 九个行情视图都是重组件（图表/无限列表），一律懒加载：
// 右侧行情面板和各种行情格共用同一个渲染入口，避免两份 switch 各自漏掉一个 case。
const DragonTigerView = lazy(() => import("@/components/DragonTigerView"))
const ReplayView = lazy(() => import("@/components/ReplayView"))
const StrongStocksTab = lazy(() => import("@/components/StrongStocksTab"))
const PromotionView = lazy(() => import("@/components/PromotionView"))
const PlateRankingTab = lazy(() => import("@/components/PlateRankingTab"))
const StockRankingTab = lazy(() => import("@/components/StockRankingTab"))
const TimelineTab = lazy(() => import("@/components/TimelineTab"))
const SentimentChipTab = lazy(() => import("@/components/SentimentChipTab"))
const NewsFeed = lazy(() => import("@/components/NewsFeed"))

function ViewSkeleton() {
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

/** 单个行情视图，自带错误边界与懒加载骨架；调用方只管给尺寸和滚动容器。 */
export default function MarketView({
  view,
  boundary = "MarketView",
}: {
  view: RightPanelView
  boundary?: string
}) {
  return (
    <ErrorBoundary name={boundary}>
      <Suspense fallback={<ViewSkeleton />}>
        <ViewRenderer view={view} />
      </Suspense>
    </ErrorBoundary>
  )
}
