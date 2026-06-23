import { create } from "zustand";

interface Tab {
  id: string;
  name: string;
}

interface StockTabsState {
  stockTabs: Tab[];
  activeStockTab: string | null;
  activeThreadId: number | null;
  openTab: (id: string, name: string) => void;
  closeTab: (id: string) => void;
  setActiveStockTab: (id: string | null) => void;
  setActiveThreadId: (id: number | null) => void;
  clearTabs: () => void;
}

export const useStockTabStore = create<StockTabsState>((set, get) => ({
  stockTabs: (() => {
    try { return JSON.parse(localStorage.getItem("stock-tabs") || "[]"); } catch { return []; }
  })(),
  activeStockTab: (() => {
    try { return localStorage.getItem("stock-active-tab"); } catch { return null; }
  })(),
  activeThreadId: (() => {
    try { const v = localStorage.getItem("active-thread"); return v ? Number(v) : null; } catch { return null; }
  })(),

  openTab: (id, name) => {
    const prev = get().stockTabs;
    if (prev.some((t) => t.id === id)) {
      set({ activeStockTab: id });
      localStorage.setItem("stock-active-tab", id);
      return;
    }
    const next = [...prev, { id, name }];
    localStorage.setItem("stock-tabs", JSON.stringify(next));
    localStorage.setItem("stock-active-tab", id);
    set({ stockTabs: next, activeStockTab: id });
  },

  closeTab: (id) => {
    const prev = get().stockTabs;
    const next = prev.filter((t) => t.id !== id);
    localStorage.setItem("stock-tabs", JSON.stringify(next));
    const active = get().activeStockTab === id ? (next[next.length - 1]?.id ?? null) : get().activeStockTab;
    if (active !== get().activeStockTab) {
      if (active) localStorage.setItem("stock-active-tab", active);
      else localStorage.removeItem("stock-active-tab");
    }
    set({ stockTabs: next, activeStockTab: active });
  },

  setActiveStockTab: (id) => {
    if (id) localStorage.setItem("stock-active-tab", id);
    else localStorage.removeItem("stock-active-tab");
    set({ activeStockTab: id });
  },

  setActiveThreadId: (id) => {
    if (id !== null) localStorage.setItem("active-thread", String(id));
    else localStorage.removeItem("active-thread");
    set({ activeThreadId: id });
  },

  clearTabs: () => {
    localStorage.removeItem("stock-tabs");
    localStorage.removeItem("stock-active-tab");
    set({ stockTabs: [], activeStockTab: null });
  },
}));
