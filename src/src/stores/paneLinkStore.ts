import { create } from "zustand"
import { eachLeaf, findLeaf, useLayoutStore, type WorkspaceLayout } from "@/stores/layoutStore"

/**
 * 网格联动：点表格里的股票（自选 / 强势股 / 个股排行 / 晋级分析 / 板块成分股…），
 * 是把 K 线**派发到看盘那一格**，还是在本格打开。
 *
 * 界面上只有一个锁，两态：
 * - 锁定（默认）：点股票 → K 线在关联的看盘格打开，本格的榜单不动
 * - 解锁：点股票 → 就在本格打开（子页面，关掉回到榜单）
 *
 * 目标不让人选：一个工作区里通常只有一格在看 K 线，就让"关联"自动指向布局里第一格
 * 「个股详情」（没有就落到对话格的 K 线标签；实在没有才在本格打开）。手动指定目标
 * 需要维护一张"目标面板"列表，而那个列表的标签会随对方钉的股票变来变去——不如不给。
 *
 * 锁按格子记（`布局 id → 格子 id → 是否锁定`），只存本机：它取决于你这块屏幕上
 * 摆了几格，换台机器布局不同，同步过去只会指向不存在的格子。
 */
const STORAGE_KEY = "openatrading-pane-locks"

type LockMap = Record<string, Record<string, boolean>>

function readLocks(): LockMap {
  try {
    const raw: unknown = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "{}")
    if (!raw || typeof raw !== "object") return {}
    const out: LockMap = {}
    for (const [layoutId, value] of Object.entries(raw as Record<string, unknown>)) {
      if (!value || typeof value !== "object") continue
      const pairs = Object.entries(value as Record<string, unknown>).filter(
        (entry): entry is [string, boolean] => typeof entry[1] === "boolean"
      )
      if (pairs.length > 0) out[layoutId] = Object.fromEntries(pairs)
    }
    return out
  } catch {
    return {}
  }
}

interface PaneLinkState {
  locks: LockMap
  /** 这一格是否锁定联动（默认锁）。 */
  isLocked: (layoutId: string | null | undefined, leafId: string | null | undefined) => boolean
  setLocked: (layoutId: string, leafId: string, locked: boolean) => void
}

export const usePaneLinkStore = create<PaneLinkState>((set, get) => {
  const persist = (locks: LockMap) => {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(locks))
    } catch {
      // 隐私模式下写不进去：本次会话内仍然生效
    }
    set({ locks })
  }

  return {
    locks: readLocks(),

    isLocked: (layoutId, leafId) => {
      if (!layoutId || !leafId) return true
      return get().locks[layoutId]?.[leafId] ?? true
    },

    setLocked: (layoutId, leafId, locked) => {
      const locks = get().locks
      persist({ ...locks, [layoutId]: { ...locks[layoutId], [leafId]: locked } })
    },
  }
})

/**
 * 能接住股票的格子：「个股详情」优先（K 线就是它的正文），其次是对话格
 * （K 线开成股票标签，和对话格自己的搜索一致）。
 */
export function stockTargetCandidates(
  layout: WorkspaceLayout | null | undefined
): { id: string; label: string }[] {
  if (!layout) return []
  const details: { id: string; label: string }[] = []
  const chats: { id: string; label: string }[] = []
  eachLeaf(layout.root, (leaf) => {
    if (leaf.kind === "detail") {
      details.push({ id: leaf.id, label: "个股详情" })
    } else if (leaf.kind === "chat") {
      chats.push({ id: leaf.id, label: "对话格" })
    }
  })
  return [...details, ...chats]
}

/** 联动目标：布局里第一格「个股详情」，没有就对话格；都没有返回 `null`。 */
export function resolveStockTarget(layoutId: string | null | undefined): string | null {
  if (!layoutId) return null
  const layout = useLayoutStore.getState().layouts.find((item) => item.id === layoutId)
  if (!layout) return null
  const candidates = stockTargetCandidates(layout)
  if (candidates.length === 0) return null
  // 目标格被删掉后也要能自愈：候选是现算的，不用额外校验
  return findLeaf(layout.root, candidates[0].id) ? candidates[0].id : null
}

/** 联动目标的人话名称，给锁的 tooltip 用。 */
export function stockTargetLabel(layoutId: string | null | undefined): string | null {
  if (!layoutId) return null
  const layout = useLayoutStore.getState().layouts.find((item) => item.id === layoutId)
  if (!layout) return null
  return stockTargetCandidates(layout)[0]?.label ?? null
}
