import { create } from "zustand";

interface ThreadStoreState {
  activeThreadId: number | null;
  setActiveThreadId: (id: number | null) => void;
}

export const useThreadStore = create<ThreadStoreState>((set) => ({
  activeThreadId: (() => {
    try { const v = localStorage.getItem("active-thread"); return v ? Number(v) : null; } catch { return null; }
  })(),

  setActiveThreadId: (id) => {
    if (id !== null) localStorage.setItem("active-thread", String(id));
    else localStorage.removeItem("active-thread");
    set({ activeThreadId: id });
  },
}));
