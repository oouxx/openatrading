import MarketView from "@/components/MarketView"
import KlineDrawer from "@/components/KlineDrawer"
import { PaneNavProvider } from "@/components/PaneNav"
import { MARKET_VIEW_ICONS, MARKET_VIEWS } from "@/lib/marketViews"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { useLayoutStore, type MarketPaneKind } from "@/stores/layoutStore"

/**
 * 行情格：一个格子只看一个榜，表头下拉用来换榜。
 *
 * 九个行情页都是独立的面板类型（`kind` 就是视图 id），所以"换榜"在这里是换内容
 * 而不是换结构——内置的盯盘预设也能直接换，不需要先克隆布局。
 *
 * 点表格里的股票会在**本格内**打开 K 线（子页面），表头仍在，K 线自带关闭按钮，
 * 不会出现"跳进去回不来"。
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
  const stack = useLayoutStore((s) => s.paneDetail[leafId])
  const detail = stack?.[stack.length - 1]
  const closePaneDetail = useLayoutStore((s) => s.closePaneDetail)
  const Icon = MARKET_VIEW_ICONS[kind] ?? MARKET_VIEW_ICONS["strong-stocks"]

  return (
    <PaneNavProvider leafId={leafId}>
      <div className="bg-surface flex h-full min-w-0 flex-col overflow-hidden">
        <header className="border-hairline flex h-9 shrink-0 items-center border-b px-1.5">
          <Select
            value={kind}
            onValueChange={(next) => setPaneKind(layoutId, leafId, next as MarketPaneKind)}
          >
            <SelectTrigger
              size="sm"
              aria-label="这一格看哪个榜"
              className="text-caption text-primary hover:bg-hover h-7 min-w-0 gap-1.5 border-transparent bg-transparent px-1 font-medium shadow-none"
            >
              <Icon className="text-tertiary size-3.5" />
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {MARKET_VIEWS.map((item) => (
                <SelectItem key={item.id} value={item.id}>
                  {item.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </header>

        <div className="relative min-h-0 flex-1 overflow-y-auto">
          <MarketView view={kind} boundary="MarketPane" />
          {detail?.type === "stock" && (
            // 子页面：覆盖正文但保留表头，K 线自己的关闭按钮就是"返回"
            <div className="bg-canvas absolute inset-0 z-20 overflow-y-auto">
              <KlineDrawer
                symbol={detail.symbol}
                name={detail.name}
                onClose={() => closePaneDetail(leafId)}
              />
            </div>
          )}
        </div>
      </div>
    </PaneNavProvider>
  )
}
