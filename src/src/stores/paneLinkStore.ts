import { create } from "zustand"
import { eachLeaf, findLeaf, useLayoutStore, type WorkspaceLayout } from "@/stores/layoutStore"

/**
 * 网格联动：点表格里的股票（自选 / 强势股 / 个股排行 / 晋级分析 / 板块成分股…），
 * 把 K 线送到**关联的那一格**打开，而不是在本格盖一层子页面。
 *
 * 关联粒度是"每个布局一个目标格"：一个工作区里通常只有一格是看 K 线的
 * （「个股详情」），多格各配一个目标只会让人对不上号。没手动关联时按结构自动挑
 * 第一格「个股详情」，实在没有才退回本格子页面（`PaneStockPage`）。
 *
 * 只存本机（localStorage），不进布局快照：关联取决于你屏幕上摆了几格，
 * 换台机器布局不同，同步过去只会指向一个不存在的面板。
 */
const STORAGE_KEY = "openatrading-pane-links"

type LinkMap = Record<string, string>

function readLinks(): LinkMap {
  try {
    const raw: unknown = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "{}")
    if (!raw || typeof raw !== "object") return {}
    // 旧版本存的是 { 布局: { 面板: 目标 } }，只保留能当字符串用的项
    return Object.fromEntries(
      Object.entries(raw as Record<string, unknown>).filter(
        (entry): entry is [string, string] => typeof entry[1] === "string"
      )
    )
  } catch {
    return {}
  }
}

interface PaneLinkState {
  links: LinkMap
  link: (layoutId: string, targetId: string) => void
  unlink: (layoutId: string) => void
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

    link: (layoutId, targetId) => persist({ ...get().links, [layoutId]: targetId }),

    unlink: (layoutId) => {
      const links = { ...get().links }
      delete links[layoutId]
      persist(links)
    },
  }
})

/**
 * 能接住股票的格子：「个股详情」优先（K 线就是它的正文），其次是对话格
 * （K 线开成股票标签，和对话格自己的搜索一致）。
 *
 * 「个股详情」排在前面：自动关联先挑它，用户没手动配过也能落到一处稳定的看盘位。
 */
export function stockTargetCandidates(
  layout: WorkspaceLayout | null | undefined
): { id: string; label: string }[] {
  if (!layout) return []
  const details: { id: string; label: string }[] = []
  const chats: { id: string; label: string }[] = []
  eachLeaf(layout.root, (leaf) => {
    if (leaf.kind === "detail") {
      details.push({
        id: leaf.id,
        label: leaf.symbolName ? `个股详情 · ${leaf.symbolName}` : "个股详情",
      })
    } else if (leaf.kind === "chat") {
      chats.push({ id: leaf.id, label: "对话格 · K 线标签" })
    }
  })
  return [...details, ...chats]
}

/**
 * 解析"这次点击该去哪一格开 K 线"。
 *
 * 手动关联（且目标还在）优先；否则自动取第一格「个股详情」；都没有返回 `null`，
 * 调用方退回本格子页面。
 */
export function resolveStockTarget(layoutId: string | null | undefined): string | null {
  if (!layoutId) return null
  const layout = useLayoutStore.getState().layouts.find((item) => item.id === layoutId)
  if (!layout) return null

  const linked = usePaneLinkStore.getState().links[layoutId]
  if (linked && findLeaf(layout.root, linked)) return linked

  const candidates = stockTargetCandidates(layout)
  if (candidates.length > 0) return candidates[0].id
  return null
}
