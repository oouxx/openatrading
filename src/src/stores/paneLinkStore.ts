import { create } from "zustand"

/**
 * 面板联动：点击某一格里的股票，把 K 线送到**关联的那一格**打开。
 *
 * 以「谁派发 → 派发到哪」为一对：`links[布局 id][派发面板 id] = 目标面板 id`。
 * 现在只有自选股用它（点一只票在关联格里开 K 线），但形状是通用的，将来榜单格
 * 想联动也一样用。
 *
 * 和面板宽度一样只存本机（localStorage），不进布局快照：关联关系取决于你屏幕上
 * 摆了几格，换台机器布局不同，硬同步过去只会指向一个不存在的格子。
 */
const STORAGE_KEY = "openatrading-pane-links"

type LinkMap = Record<string, Record<string, string>>

function readLinks(): LinkMap {
  try {
    const raw: unknown = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "{}")
    if (!raw || typeof raw !== "object") return {}
    const out: LinkMap = {}
    for (const [layoutId, value] of Object.entries(raw as Record<string, unknown>)) {
      if (!value || typeof value !== "object") continue
      const pairs = Object.entries(value as Record<string, unknown>).filter(
        (entry): entry is [string, string] => typeof entry[1] === "string"
      )
      if (pairs.length > 0) out[layoutId] = Object.fromEntries(pairs)
    }
    return out
  } catch {
    return {}
  }
}

interface PaneLinkState {
  links: LinkMap
  /** 拿到某一格的联动目标（没有关联时为 `null`，调用方按"本格打开"处理）。 */
  targetOf: (layoutId: string, leafId: string) => string | null
  link: (layoutId: string, leafId: string, targetId: string) => void
  unlink: (layoutId: string, leafId: string) => void
}

export const usePaneLinkStore = create<PaneLinkState>((set, get) => {
  const persist = (links: LinkMap) => {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(links))
    } catch {
      // 隐私模式下写不进去：本次会话内仍然生效
    }
    set({ links })
  }

  return {
    links: readLinks(),

    targetOf: (layoutId, leafId) => get().links[layoutId]?.[leafId] ?? null,

    link: (layoutId, leafId, targetId) => {
      const links = get().links
      persist({ ...links, [layoutId]: { ...links[layoutId], [leafId]: targetId } })
    },

    unlink: (layoutId, leafId) => {
      const links = get().links
      const next = { ...links[layoutId] }
      delete next[leafId]
      persist({ ...links, [layoutId]: next })
    },
  }
})
