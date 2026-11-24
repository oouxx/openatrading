import MarketView from "@/components/MarketView"
import { PaneNavProvider } from "@/components/PaneNav"
import { MARKET_VIEW_ICONS } from "@/lib/marketViews"
import { PANE_KIND_LABELS } from "@/stores/layoutStore"
import type { MarketPaneKind } from "@/stores/layoutStore"

/**
 * 行情格：一个格子只看一个榜。表头只显示当前榜名，**不**放切换器——换内容统一走
 * 右上角的 `PaneKindSwitcher`（14 种面板同一套控件、同一个位置），否则行情格有两个
 * 入口、其它格子只有角落那一个，位置还不一致。
 *
 * 点表格里的股票：这一格被拖到别的格上关联过，就在那一格打开 K 线；否则在本格开
 * 子页面。关联 / 解除都由格子角落的把手拖动完成（见 `usePaneDragLink`）。
 */
export default function MarketPane({
  leafId,
  layoutId,
  kind,
}: {
  leafId: string
  layoutId: string
  kind: MarketPaneKind
}) {
  // 表头只显示当前榜名；图标用查表而不是渲染期调函数（会被 fast-refresh 判成
  // "render 期创建组件"）
  const Icon = MARKET_VIEW_ICONS[kind] ?? MARKET_VIEW_ICONS["strong-stocks"]
  const title = PANE_KIND_LABELS[kind] ?? kind

  return (
    <PaneNavProvider leafId={leafId} layoutId={layoutId}>
      <div className="bg-surface @container flex h-full min-w-0 flex-col overflow-hidden">
        <header className="border-hairline flex h-9 shrink-0 items-center gap-1 border-b px-1.5">
          {/* 这一格看哪个榜：只做**展示**。换内容统一走右上角那颗（14 种面板同一套
              控件、同一个位置），表头这里不再放第二个下拉——否则行情格有两个入口，
              别的格子又只能去角落，位置不一致。 */}
          <span className="flex min-w-0 items-center gap-1.5 px-1">
            <Icon className="text-tertiary size-3.5 shrink-0" />
            <span className="text-caption text-primary truncate font-medium">{title}</span>
          </span>
        </header>

        <div className="relative min-h-0 flex-1 overflow-y-auto">
          <MarketView view={kind} boundary="MarketPane" />
        </div>
      </div>
    </PaneNavProvider>
  )
}
