import { create } from "zustand"

interface ThreadStoreState {
  activeThreadId: string | null
  setActiveThreadId: (id: string | null) => void
}

export const useThreadStore = create<ThreadStoreState>((set) => ({
  activeThreadId: (() => {
    try {
      const v = localStorage.getItem("active-thread")
      return v ? JSON.parse(v) : null
    } catch {
      return null
    }
  })(),

  setActiveThreadId: (id) => {
    if (id !== null) localStorage.setItem("active-thread", JSON.stringify(id))
    else localStorage.removeItem("active-thread")
    set({ activeThreadId: id })
  },
}))
