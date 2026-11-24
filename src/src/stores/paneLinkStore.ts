import { create } from "zustand"
import { findLeaf, useLayoutStore } from "@/stores/layoutStore"

/**
 * 网格联动：把一格**拖到**另一格上 = 关联，把这一格**拖开** = 解除关联。
 *
 * 关联的方向是"拖动的这一格 → 落点那一格"：拖动的是榜单/自选（派发方），落点是看盘格
 * （接收方）。点股票时如果这一格有链接，就在对方那一格打开 K 线；没有链接就在本格开
 * 子页面——不再猜"应该去哪一格"，一切由用户拖出来。
 *
 * 链接按格子记（`布局 id → 派发格 id → 接收格 id`），只存本机：它描述的是你这块屏幕上
 * 的摆放关系，换台机器布局不同，同步过去只会指向不存在的格子。
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
  /** 这一格派发到哪一格（没有关联时为 `null`）。 */
  targetOf: (
    layoutId: string | null | undefined,
    sourceId: string | null | undefined
  ) => string | null
  /** 把 `sourceId` 关联到 `targetId`；已有关联会被覆盖。 */
  link: (layoutId: string, sourceId: string, targetId: string) => void
  /** 解除与这一格有关的所有关联（不论它是派发方还是接收方）。 */
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

    targetOf: (layoutId, sourceId) => {
      if (!layoutId || !sourceId) return null
      return get().links[layoutId]?.[sourceId] ?? null
    },

    link: (layoutId, sourceId, targetId) => {
      if (sourceId === targetId) return
      const links = get().links
      persist({ ...links, [layoutId]: { ...links[layoutId], [sourceId]: targetId } })
    },

    unlink: (layoutId, leafId) => {
      const links = get().links[layoutId]
      if (!links) return
      // 两个方向都要断：这一格可能是派发方（键），也可能是接收方（值）
      const next = Object.fromEntries(
        Object.entries(links).filter(([source, target]) => source !== leafId && target !== leafId)
      )
      persist({ ...get().links, [layoutId]: next })
    },
  }
})

/**
 * 这一格有没有**有效**的关联（不论它是派发方还是接收方）。
 *
 * 对方格被拆掉/删掉之后链接会留成悬空记录，这里一并校验，别让把手一直亮着。
 */
export function paneHasLink(
  layoutId: string | null | undefined,
  leafId: string | null | undefined
): boolean {
  if (!layoutId || !leafId) return false
  const layout = useLayoutStore.getState().layouts.find((item) => item.id === layoutId)
  if (!layout) return false
  const links = usePaneLinkStore.getState().links[layoutId] ?? {}
  const target = links[leafId]
  if (target) return findLeaf(layout.root, target) !== null
  return Object.entries(links).some(
    ([source, dest]) => dest === leafId && findLeaf(layout.root, source) !== null
  )
}

/** 这一格是否还有效地关联着（目标格被删掉时视为没有）。 */
export function linkedTarget(
  layoutId: string | null | undefined,
  sourceId: string | null | undefined
): string | null {
  if (!layoutId || !sourceId) return null
  const layout = useLayoutStore.getState().layouts.find((item) => item.id === layoutId)
  const target = usePaneLinkStore.getState().links[layoutId]?.[sourceId] ?? null
  if (!layout || !target) return null
  return findLeaf(layout.root, target) ? target : null
}

/** 目标格的名字，给提示文案用（"在「个股详情」格打开"）。 */
export function paneDisplayName(
  layoutId: string | null | undefined,
  leafId: string | null | undefined
): string | null {
  if (!layoutId || !leafId) return null
  const layout = useLayoutStore.getState().layouts.find((item) => item.id === layoutId)
  const leaf = layout ? findLeaf(layout.root, leafId) : null
  if (!leaf) return null
  if (leaf.kind === "detail") return "个股详情"
  if (leaf.kind === "chat") return "对话格"
  return "行情格"
}
