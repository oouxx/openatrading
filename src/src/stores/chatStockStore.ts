import { create } from "zustand"

/** 对话格里打开过的一只股票（VS Code 式标签）。 */
export interface StockTab {
  code: string
  name: string
}

interface ChatStockState {
  tabs: StockTab[]
  /** 当前显示 K 线的那只；`null` = 显示对话。 */
  activeCode: string | null
  /** 打开（已打开则切过去）：搜索框选中一只股票时调用。 */
  open: (code: string, name: string) => void
  activate: (code: string | null) => void
  /** 关掉一个标签；关的是当前标签时自动切到相邻一个，全关回到对话。 */
  close: (code: string) => void
}

/**
 * 标签数量上限。
 *
 * 横条是单行滚动的，不设上限会变成一条永远滑不到头的带子；超过就挤掉
 * 「最早打开且不是当前标签」的那个——保留当前这只，用户手里的页面不会被换掉。
 */
const MAX_TABS = 8

const TABS_KEY = "openatrading-chat-stock-tabs"
const ACTIVE_KEY = "openatrading-chat-stock-active"

function readTabs(): StockTab[] {
  try {
    const raw = JSON.parse(localStorage.getItem(TABS_KEY) ?? "[]") as unknown
    if (!Array.isArray(raw)) return []
    return raw
      .map((item) => item as Partial<StockTab>)
      .filter((item): item is StockTab => Boolean(item.code && item.name))
      .slice(0, MAX_TABS)
  } catch {
    return []
  }
}

function readActive(): string | null {
  try {
    return localStorage.getItem(ACTIVE_KEY)
  } catch {
    return null
  }
}

/**
 * 对话格的股票标签。
 *
 * 只存"打开过哪些票、现在看哪只"这类浏览状态：不参与布局同步（换台机器重新搜一次
 * 就行），也不需要后端接口。K 线本身的数据仍然走 `KlineDrawer` 的 SSE。
 */
export const useChatStockStore = create<ChatStockState>((set, get) => {
  const persist = (tabs: StockTab[], activeCode: string | null) => {
    try {
      localStorage.setItem(TABS_KEY, JSON.stringify(tabs))
      if (activeCode) localStorage.setItem(ACTIVE_KEY, activeCode)
      else localStorage.removeItem(ACTIVE_KEY)
    } catch {
      // 隐私模式下 localStorage 会抛：标签是易失状态，写不进去不影响本次使用
    }
    set({ tabs, activeCode })
  }

  return {
    tabs: readTabs(),
    activeCode: readActive(),

    open: (code, name) => {
      const { tabs, activeCode } = get()
      if (tabs.some((tab) => tab.code === code)) {
        persist(tabs, code)
        return
      }
      const next = [...tabs, { code, name }]
      if (next.length > MAX_TABS) {
        const evict = next.findIndex((tab) => tab.code !== activeCode && tab.code !== code)
        if (evict >= 0) next.splice(evict, 1)
      }
      persist(next, code)
    },

    activate: (code) => persist(get().tabs, code),

    close: (code) => {
      const { tabs, activeCode } = get()
      const index = tabs.findIndex((tab) => tab.code === code)
      if (index < 0) return
      const next = tabs.filter((tab) => tab.code !== code)
      if (activeCode !== code) {
        persist(next, activeCode)
        return
      }
      // 关掉当前标签：接替它的是右邻，没有右邻就取新的最后一个
      persist(next, next[index]?.code ?? next[next.length - 1]?.code ?? null)
    },
  }
})
