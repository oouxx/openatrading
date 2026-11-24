import type { ReactNode } from "react"
import {
  ArrowLeftRight,
  CandlestickChart,
  Layers,
  MessageSquare,
  PanelRight,
  Star,
  Wallet,
} from "lucide-react"
import { RolePane, WatchlistPane } from "@/components/EmbeddedPages"
import MarketPane from "@/components/MarketPane"
import StockDetailPane from "@/components/StockDetailPane"
import { OrderBookPane, TradePane } from "@/components/TradePanes"
import {
  MARKET_PANE_KINDS,
  resolvePaneKind,
  type MarketPaneKind,
  type PaneKind,
  type PaneLeaf,
  type WorkspaceLayout,
} from "@/lib/layoutTypes"
import { MARKET_VIEW_ICONS } from "@/lib/marketViews"

/**
 * 面板类型的注册表：**新增一种可放进网格的内容，只需要改这里**（加上
 * `lib/layoutTypes.ts` 里的类型/标签/尺寸），不用再动 LayoutNodeView、AppShell
 * 或编辑面板——它们都只认 `kind` 这一个字段。
 *
 * 约定：
 * - `selfPanel` 的类型自己就是一个 `Panel`（行情面板折叠成图标条依赖
 *   `collapsible`，外面再包一层会变成嵌套面板），由 LayoutNodeView 单独渲染；
 * - 其余类型只返回内容，尺寸与拖拽由 LayoutNodeView 统一包。
 */
export interface PaneRenderContext {
  /** 对话面板的内容由 AppShell 提供（它持有会话与传输层的句柄）。 */
  chat: ReactNode
  layout: WorkspaceLayout
  presetKinds: Record<string, Record<string, PaneKind>>
}

interface PaneDefinition {
  icon: typeof MessageSquare
  /** 自己就是 Panel，LayoutNodeView 不再外包一层。 */
  selfPanel?: boolean
  render?: (leaf: PaneLeaf, ctx: PaneRenderContext) => ReactNode
}

/** 九个行情视图各自是一个面板类型，图标直接取视图的图标。 */
const MARKET_DEFINITIONS = Object.fromEntries(
  MARKET_PANE_KINDS.map((kind) => [
    kind,
    {
      icon: MARKET_VIEW_ICONS[kind] ?? MessageSquare,
      render: (leaf: PaneLeaf, ctx: PaneRenderContext) => (
        <MarketPane
          leafId={leaf.id}
          layoutId={ctx.layout.id}
          kind={resolvedMarketKind(leaf, ctx)}
        />
      ),
    },
  ])
) as Record<MarketPaneKind, PaneDefinition>

/** 内置布局里被换过榜的格子，实际展示的是覆盖表里的类型。 */
function resolvedMarketKind(leaf: PaneLeaf, ctx: PaneRenderContext): MarketPaneKind {
  const effective = resolvePaneKind(ctx.layout, leaf, ctx.presetKinds)
  return (MARKET_PANE_KINDS as readonly string[]).includes(effective)
    ? (effective as MarketPaneKind)
    : "strong-stocks"
}

export const PANE_DEFINITIONS: Record<PaneKind, PaneDefinition> = {
  ...MARKET_DEFINITIONS,
  chat: {
    icon: MessageSquare,
    render: (_leaf, ctx) => ctx.chat,
  },
  detail: {
    icon: CandlestickChart,
    render: (leaf, ctx) => (
      <StockDetailPane
        leafId={leaf.id}
        layoutId={ctx.layout.id}
        symbol={leaf.symbol}
        symbolName={leaf.symbolName}
      />
    ),
  },
  "market-tabs": {
    icon: PanelRight,
    selfPanel: true,
  },
  watchlist: {
    icon: Star,
    render: (leaf, ctx) => <WatchlistPane leafId={leaf.id} layoutId={ctx.layout.id} />,
  },
  roles: {
    icon: Wallet,
    render: () => <RolePane />,
  },
  // 这两个格子跟随工作区当前标的（activeStockStore），和 K 线派发同一条链路
  orderbook: {
    icon: Layers,
    render: () => <OrderBookPane />,
  },
  trade: {
    icon: ArrowLeftRight,
    render: () => <TradePane />,
  },
}

export function paneIcon(kind: PaneKind): typeof MessageSquare {
  return PANE_DEFINITIONS[kind].icon
}

/** 渲染一格的内容；`selfPanel` 的类型由 LayoutNodeView 自己处理，这里返回 null。 */
export function renderWorkspacePane(leaf: PaneLeaf, ctx: PaneRenderContext): ReactNode {
  const definition = PANE_DEFINITIONS[leaf.kind]
  return definition.render ? definition.render(leaf, ctx) : null
}
