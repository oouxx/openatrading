import { create } from "zustand"

export type RightPanelView =
  | "dragon-tiger"
  | "breakout"
  | "replay"
  | "strong-stocks"
  | "promotion"
  | "plate"
  | "timeline"
  | "sentiment-chip"
  | "market-intel"
  | "news"

const ACTIVE_VIEW_KEY = "right-panel-active-view"
const DEFAULT_VIEW: RightPanelView = "dragon-tiger"

const ALL_VIEWS: ReadonlySet<RightPanelView> = new Set<RightPanelView>([
  "dragon-tiger",
  "breakout",
  "replay",
  "strong-stocks",
  "promotion",
  "plate",
  "timeline",
  "sentiment-chip",
  "market-intel",
  "news",
])

function loadActiveView(): RightPanelView {
  try {
    const stored = localStorage.getItem(ACTIVE_VIEW_KEY)
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
  setActiveView: (view: RightPanelView) => void
  openKline: (symbol: string, name: string) => void
  closeKline: () => void
}

export const useRightPanelStore = create<RightPanelState>((set) => ({
  activeView: loadActiveView(),
  klineTarget: null,

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
}))
