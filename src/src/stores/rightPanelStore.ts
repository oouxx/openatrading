import { create } from "zustand"
import type { DragonTigerDetailItem } from "@/hooks/query/useDragonTiger"

export const RIGHT_PANEL_VIEWS = [
  "dragon-tiger",
  "replay",
  "strong-stocks",
  "promotion",
  "plate-ranking",
  "stock-ranking",
  "timeline",
  "sentiment-chip",
  "news",
] as const

export type RightPanelView = (typeof RIGHT_PANEL_VIEWS)[number]

export function isRightPanelView(value: unknown): value is RightPanelView {
  return typeof value === "string" && (RIGHT_PANEL_VIEWS as readonly string[]).includes(value)
}

const ACTIVE_VIEW_KEY = "right-panel-active-view"
const DEFAULT_VIEW: RightPanelView = "dragon-tiger"

const ALL_VIEWS: ReadonlySet<string> = new Set<string>(RIGHT_PANEL_VIEWS)

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
  /** 面板内的"下一层"：板块详情。K 线与它共用同一层覆盖区。 */
  plateTarget: { plate_id: number; plate_name: string; pcp: number } | null
  /** 面板内的"下一层"：龙虎榜个股详情。 */
  dragonTarget: DragonTigerDetailItem | null
  stockRankingOrder: "gainers" | "losers"
  stockRankingMarket: "all" | "main" | "chinext" | "star"
  stockRankingExcludeSt: boolean
  setActiveView: (view: RightPanelView) => void
  setStockRankingOrder: (order: "gainers" | "losers") => void
  setStockRankingMarket: (market: "all" | "main" | "chinext" | "star") => void
  setStockRankingExcludeSt: (exclude: boolean) => void
  openKline: (symbol: string, name: string) => void
  closeKline: () => void
  openPlate: (plate: { plate_id: number; plate_name: string; pcp: number }) => void
  closePlate: () => void
  openDragon: (item: DragonTigerDetailItem) => void
  closeDragon: () => void
}

export const useRightPanelStore = create<RightPanelState>((set) => ({
  activeView: loadActiveView(),
  klineTarget: null,
  plateTarget: null,
  dragonTarget: null,
  stockRankingOrder: "gainers",
  stockRankingMarket: "all",
  stockRankingExcludeSt: true,

  setActiveView: (view) => {
    try {
      localStorage.setItem(ACTIVE_VIEW_KEY, view)
    } catch {
      // localStorage 写入失败时仍更新内存状态，保持交互可用
    }
    // 换 tab 关掉面板里的子页面（K 线 / 板块详情），否则新视图会被挡住
    set({ activeView: view, klineTarget: null, plateTarget: null, dragonTarget: null })
  },

  openKline: (symbol, name) => set({ klineTarget: { symbol, name } }),

  closeKline: () => set({ klineTarget: null }),

  openPlate: (plateTarget) => set({ plateTarget, klineTarget: null }),

  closePlate: () => set({ plateTarget: null }),

  openDragon: (dragonTarget) => set({ dragonTarget, klineTarget: null, plateTarget: null }),

  closeDragon: () => set({ dragonTarget: null }),

  setStockRankingOrder: (stockRankingOrder) => set({ stockRankingOrder }),

  setStockRankingMarket: (stockRankingMarket) => set({ stockRankingMarket }),

  setStockRankingExcludeSt: (stockRankingExcludeSt) => set({ stockRankingExcludeSt }),
}))
