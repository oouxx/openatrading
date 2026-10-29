import { create } from "zustand"

export type RightPanelView =
  | "dragon-tiger"
  | "replay"
  | "strong-stocks"
  | "promotion"
  | "plate-ranking"
  | "stock-ranking"
  | "timeline"
  | "sentiment-chip"
  | "news"

const ACTIVE_VIEW_KEY = "right-panel-active-view"
const DEFAULT_VIEW: RightPanelView = "dragon-tiger"

const ALL_VIEWS: ReadonlySet<RightPanelView> = new Set<RightPanelView>([
  "dragon-tiger",
  "replay",
  "strong-stocks",
  "promotion",
  "plate-ranking",
  "stock-ranking",
  "timeline",
  "sentiment-chip",
  "news",
])

function loadActiveView(): RightPanelView {
  try {
    const stored = localStorage.getItem(ACTIVE_VIEW_KEY)
    // 旧版 breakout 入口合并后迁移到 promotion
    if (stored === "breakout") return "promotion"
    if (stored && ALL_VIEWS.has(stored as RightPanelView)) {
      return stored as RightPanelView
    }
  } catch {
    // localStorage 不可用或被禁用时回退到默认值
  }
  return DEFAULT_VIEW
}

interface RightPanelState {
  activeView: RightPanelView
  klineTarget: { symbol: string; name: string } | null
  stockRankingOrder: "gainers" | "losers"
  stockRankingMarket: "all" | "main" | "chinext" | "star" | "beijing"
  stockRankingExcludeSt: boolean
  setActiveView: (view: RightPanelView) => void
  setStockRankingOrder: (order: "gainers" | "losers") => void
  setStockRankingMarket: (market: "all" | "main" | "chinext" | "star" | "beijing") => void
  setStockRankingExcludeSt: (exclude: boolean) => void
  openKline: (symbol: string, name: string) => void
  closeKline: () => void
}

export const useRightPanelStore = create<RightPanelState>((set) => ({
  activeView: loadActiveView(),
  klineTarget: null,
  stockRankingOrder: "gainers",
  stockRankingMarket: "all",
  stockRankingExcludeSt: true,

  setActiveView: (view) => {
    try {
      localStorage.setItem(ACTIVE_VIEW_KEY, view)
    } catch {
      // localStorage 写入失败时仍更新内存状态，保持交互可用
    }
    set({ activeView: view })
  },

  openKline: (symbol, name) => set({ klineTarget: { symbol, name } }),

  closeKline: () => set({ klineTarget: null }),

  setStockRankingOrder: (stockRankingOrder) => set({ stockRankingOrder }),

  setStockRankingMarket: (stockRankingMarket) => set({ stockRankingMarket }),

  setStockRankingExcludeSt: (stockRankingExcludeSt) => set({ stockRankingExcludeSt }),
}))
