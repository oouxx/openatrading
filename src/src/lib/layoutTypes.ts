import type { DragonTigerDetailItem } from "@/hooks/query/useDragonTiger"
import type { RightPanelView } from "@/stores/rightPanelStore"
import { MARKET_VIEWS, marketViewLabel } from "@/lib/marketViews"

/**
 * 工作区布局的类型与常量（纯数据，不含 React 组件）。
 *
 * 九个行情视图**各自是一种面板类型**，和"对话 / 自选 / 模拟盘"平级：任何一格都能
 * 直接放「龙虎榜」，不需要先放一个"单一行情"再去里面选。这样一格 = 一个明确的东西，
 * 拖到哪、占多大、和谁并排，都由布局决定。
 *
 * 拆这个文件是为了打断「store ← 组件」的循环：store 需要 `PANE_KINDS` 做反序列化
 * 校验，组件需要类型做渲染。组件相关的部分（图标、渲染器）在
 * `lib/paneRegistry.tsx`，新增一种面板类型要同时改两处，其余代码不需要动。
 */

/** 九个行情视图，每一个都是独立的面板类型。 */
export type MarketPaneKind = RightPanelView

/** 顺序沿用右栏 tab 的顺序（强势股在前），下拉里和用户已经记住的位置一致。 */
export const MARKET_PANE_KINDS: readonly MarketPaneKind[] = MARKET_VIEWS.map((view) => view.id)

/** 与行情无关的通用面板。 */
export type GeneralPaneKind =
  | "chat"
  | "detail"
  | "market-tabs"
  | "watchlist"
  | "roles"
  | "orderbook"
  | "trade"
  | "metrics"
  | "sectors"

export type PaneKind = GeneralPaneKind | MarketPaneKind

export function isMarketPaneKind(kind: PaneKind): kind is MarketPaneKind {
  return (MARKET_PANE_KINDS as readonly string[]).includes(kind)
}

/** 顺序即"这一格放什么"下拉里的顺序（行情组按右栏原来的 tab 顺序）。 */
export const PANE_KINDS: readonly PaneKind[] = [
  "chat",
  "detail",
  "market-tabs",
  ...MARKET_PANE_KINDS,
  "watchlist",
  "roles",
  "orderbook",
  "trade",
  "metrics",
  "sectors",
]

/**
 * "换内容"下拉里能选的通用面板。
 *
 * 不含 `market-tabs`（旧的右侧行情竖栏）：它仍然能被解析和渲染（老布局里可能还有），
 * 但不再提供新建入口——九个行情视图各自都是独立面板类型，直接拆一格放就行。
 */
export const GENERAL_PANE_KINDS: readonly GeneralPaneKind[] = [
  "chat",
  "detail",
  "watchlist",
  "roles",
  "orderbook",
  "trade",
  "metrics",
  "sectors",
]

export const PANE_KIND_LABELS: Record<string, string> = {
  chat: "对话",
  detail: "个股详情",
  "market-tabs": "行情面板",
  watchlist: "自选",
  roles: "模拟盘",
  orderbook: "五档行情",
  trade: "下单",
  metrics: "关键指标",
  sectors: "所属板块",
  ...Object.fromEntries(MARKET_PANE_KINDS.map((kind) => [kind, marketViewLabel(kind)])),
}

/**
 * 上下分割里不可用的类型。
 *
 * "行情面板"折叠态是一条 48px 宽的竖条，横过来会散架，所以只允许放在左右分割里。
 */
export const PANE_HORIZONTAL_ONLY: ReadonlySet<PaneKind> = new Set<PaneKind>(["market-tabs"])

export interface PaneSizing {
  minSize: number
  defaultSize: number
}

const GENERIC_SIZING: Record<GeneralPaneKind, PaneSizing> = {
  chat: { minSize: 320, defaultSize: 420 },
  detail: { minSize: 320, defaultSize: 480 },
  "market-tabs": { minSize: 300, defaultSize: 400 },
  watchlist: { minSize: 280, defaultSize: 340 },
  roles: { minSize: 380, defaultSize: 560 },
  orderbook: { minSize: 220, defaultSize: 300 },
  trade: { minSize: 300, defaultSize: 360 },
  metrics: { minSize: 200, defaultSize: 280 },
  sectors: { minSize: 240, defaultSize: 320 },
}

/** 每类面板的尺寸约束：min 保证不塌，default 是首次打开时的比例基准。 */
export function paneSizing(kind: PaneKind): PaneSizing {
  return isMarketPaneKind(kind) ? { minSize: 240, defaultSize: 420 } : GENERIC_SIZING[kind]
}

/**
 * 面板内打开的"下一层页面"。
 *
 * - `stock`：点某只股票打开的 K 线；
 * - `plate`：板块排行里点某个板块打开的板块详情；
 * - `dragon`：龙虎榜里点某只股票打开的上榜详情（含买卖席位）。
 *
 * 状态挂在这一格的 id 上（而不是组件的 useState 里），所以放大/还原、切换布局
 * 回来后还在，换榜时又能被统一清掉。
 */
export interface PlateDetailTarget {
  plate_id: number
  plate_name: string
  pcp: number
}

export type DragonDetailTarget = DragonTigerDetailItem

export type PaneDetail =
  | { type: "stock"; symbol: string; name: string }
  | { type: "plate"; plate: PlateDetailTarget }
  | { type: "dragon"; item: DragonDetailTarget }

export type SplitOrientation = "horizontal" | "vertical"

export interface PaneLeaf {
  id: string
  type: "pane"
  kind: PaneKind
  /**
   * 仅 kind === "detail"：**解绑后**固定看的那只股票（如 600519）。
   * 为空表示这一格跟随工作区当前标的（默认）。
   */
  symbol?: string
  /** 股票名，仅用于标题与自选，避免每格都再查一次代码表。 */
  symbolName?: string
}

export interface SplitNode {
  id: string
  type: "split"
  /** horizontal = 左右分栏，vertical = 上下分栏。 */
  orientation: SplitOrientation
  children: LayoutNode[]
}

export type LayoutNode = PaneLeaf | SplitNode

export interface WorkspaceLayout {
  id: string
  name: string
  /**
   * 内置布局：结构只读（防止用户把预设改坏），但行情格看哪个榜仍可改——换榜是
   * 换内容不是换结构。想要拆分/删除/换类型就先克隆成自定义布局。
   */
  builtin: boolean
  /** 面板尺寸的存储 key。内置布局沿用旧 key，老用户之前调好的宽度不会丢。 */
  storageId: string
  root: LayoutNode
}

// ── 树查询 ────────────────────────────────────────────────────────────────

export function countLeaves(node: LayoutNode): number {
  return node.type === "pane"
    ? 1
    : node.children.reduce((sum, child) => sum + countLeaves(child), 0)
}

export function findLeaf(node: LayoutNode, id: string): PaneLeaf | null {
  if (node.type === "pane") return node.id === id ? node : null
  for (const child of node.children) {
    const found = findLeaf(child, id)
    if (found) return found
  }
  return null
}

export function eachLeaf(node: LayoutNode, visit: (leaf: PaneLeaf) => void) {
  if (node.type === "pane") {
    visit(node)
    return
  }
  node.children.forEach((child) => eachLeaf(child, visit))
}

/** 当前布局里出现过的面板类型，用于隐藏侧栏里重复的入口。 */
export function paneKinds(node: LayoutNode): Set<PaneKind> {
  const kinds = new Set<PaneKind>()
  eachLeaf(node, (leaf) => kinds.add(leaf.kind))
  return kinds
}

/**
 * 解析一个格子最终展示的类型。
 *
 * 内置布局的结构写在代码里，用户改过的行情格记在 `presetKinds` 覆盖表里——所以
 * 行情格换个榜不需要克隆布局。
 */
export function resolvePaneKind(
  layout: WorkspaceLayout,
  leaf: PaneLeaf,
  presetKinds: Record<string, Record<string, PaneKind>>
): PaneKind {
  if (!layout.builtin) return leaf.kind
  return presetKinds[layout.id]?.[leaf.id] ?? leaf.kind
}
