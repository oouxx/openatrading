import MarketView from "@/components/MarketView"
import { PaneNavProvider } from "@/components/PaneNav"
import { MARKET_VIEW_ICONS, MARKET_VIEWS } from "@/lib/marketViews"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { useLayoutStore } from "@/stores/layoutStore"
import type { MarketPaneKind } from "@/stores/layoutStore"

import PaneKindOption from "@/components/PaneKindOption"
import StockLinkSelect from "@/components/StockLinkSelect"

/**
 * 行情格：一个格子只看一个榜，表头下拉用来换榜。
 *
 * 表头下拉是"换个榜看"（九个行情视图，高频）；把这一格**换成别的面板**（个股详情 /
 * 五档 / 下单 / 自选 / 模拟盘）走右上角的 `PaneKindSwitcher`——那是低频动作，和放大
 * 按钮放一起，任何格子都用同一套控件。
 *
 * 点表格里的股票不会再在本格盖子页面：K 线派发到关联网格（表头的 `⚯` 选，
 * 没配就自动跟随布局里的第一格「个股详情」）；布局里没有「个股详情」时才退回
 * 本格子页面。
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
  const setPaneKind = useLayoutStore((s) => s.setPaneKind)
  // 表头触发器只画图标：取图标用查表而不是渲染期调函数（会被 fast-refresh 判成
  // "render 期创建组件"）
  const Icon = MARKET_VIEW_ICONS[kind] ?? MARKET_VIEW_ICONS["strong-stocks"]

  return (
    <PaneNavProvider leafId={leafId} layoutId={layoutId}>
      <div className="bg-surface @container flex h-full min-w-0 flex-col overflow-hidden">
        <header className="border-hairline flex h-9 shrink-0 items-center gap-1 border-b px-1.5">
          <Select
            value={kind}
            onValueChange={(next) => setPaneKind(layoutId, leafId, next as MarketPaneKind)}
          >
            <SelectTrigger
              size="sm"
              aria-label="这一格看哪个榜"
              title="这一格看哪个榜"
              className="text-caption text-primary hover:bg-hover h-7 min-w-0 gap-1.5 border-transparent bg-transparent px-1 font-medium shadow-none"
            >
              <Icon className="text-tertiary size-3.5" />
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {MARKET_VIEWS.map((item) => (
                <SelectItem key={item.id} value={item.id}>
                  <PaneKindOption kind={item.id} />
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          {/* 窄格子里让位给榜单下拉：联动设置在有空间的格子里改 */}
          <StockLinkSelect layoutId={layoutId} className="ml-auto hidden @[336px]:flex" />
        </header>

        <div className="relative min-h-0 flex-1 overflow-y-auto">
          <MarketView view={kind} boundary="MarketPane" />
        </div>
      </div>
    </PaneNavProvider>
  )
}
