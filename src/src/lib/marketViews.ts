import {
  BarChart3,
  CirclePlay,
  Clock,
  Gauge,
  Layers,
  ListOrdered,
  Rss,
  TrendingUp,
  Trophy,
} from "lucide-react"
import type { RightPanelView } from "@/stores/rightPanelStore"

/**
 * 行情视图的标签与图标。
 *
 * 右侧面板的 tab 条和行情格的选择器都从这里取，各自抄一份 TABS 迟早会漂移成
 * 两套文案。放在 lib 里而不是组件文件里，是为了不打断 fast refresh 的模块边界。
 */
export interface MarketViewMeta {
  id: RightPanelView
  label: string
  icon: typeof TrendingUp
}

export const MARKET_VIEWS: MarketViewMeta[] = [
  { id: "strong-stocks", label: "强势股", icon: TrendingUp },
  { id: "promotion", label: "晋级分析", icon: Layers },
  { id: "plate-ranking", label: "板块排行", icon: ListOrdered },
  { id: "stock-ranking", label: "个股排行", icon: BarChart3 },
  { id: "dragon-tiger", label: "龙虎榜", icon: Trophy },
  { id: "timeline", label: "时间线", icon: Clock },
  { id: "sentiment-chip", label: "情绪筹码", icon: Gauge },
  { id: "news", label: "资讯", icon: Rss },
  { id: "replay", label: "行情回放", icon: CirclePlay },
]

/** 按 id 直接查图标（不要在渲染期调函数拿组件，那会被 fast-refresh 判成动态组件）。 */
export const MARKET_VIEW_ICONS: Record<string, typeof TrendingUp> = Object.fromEntries(
  MARKET_VIEWS.map((item) => [item.id, item.icon])
)

export function marketViewLabel(view: RightPanelView): string {
  return MARKET_VIEWS.find((item) => item.id === view)?.label ?? view
}
