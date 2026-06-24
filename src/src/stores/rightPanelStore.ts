import { create } from "zustand";

export type RightPanelView =
  | "dragon-tiger"
  | "breakout"
  | "replay"
  | "roles"
  | "strong-stocks"
  | "promotion"
  | "plate"
  | "timeline"
  | "sentiment-chip"
  | "market-intel"
  | "news";

interface RightPanelState {
  activeView: RightPanelView;
  klineTarget: { symbol: string; name: string } | null;
  setActiveView: (view: RightPanelView) => void;
  openKline: (symbol: string, name: string) => void;
  closeKline: () => void;
}

export const useRightPanelStore = create<RightPanelState>((set) => ({
  activeView: "dragon-tiger",
  klineTarget: null,

  setActiveView: (view) => set({ activeView: view }),

  openKline: (symbol, name) => set({ klineTarget: { symbol, name } }),

  closeKline: () => set({ klineTarget: null }),
}));
