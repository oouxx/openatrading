import { StockSearchBox } from "@/components/StockSearchBox"
import type { ActiveStock } from "@/stores/activeStockStore"

/**
 * 看盘格的表头：这一格正在看的标的（名称 + 代码）+ 右侧"换股票"搜索框。
 *
 * 「五档行情」「下单」「关键指标」「所属板块」共用这一份。标的由各自的
 * `usePaneStock` 给（跟随中的格子读工作区标的，解绑的格子读自己的固定值），
 * 表头只负责显示和换票。
 */
export function ActiveStockHeader({
  stock,
  onPick,
  extra,
}: {
  stock: ActiveStock | null
  onPick: (code: string, name: string) => void
  extra?: React.ReactNode
}) {
  return (
    <header className="border-hairline flex h-9 shrink-0 items-center gap-2 border-b px-2">
      {stock ? (
        <span className="flex min-w-0 items-baseline gap-1.5">
          <span className="text-primary text-caption truncate font-medium">{stock.name}</span>
          <span className="text-tertiary font-number text-caption">
            {stock.code.replace(/^[a-z]+/i, "")}
          </span>
        </span>
      ) : (
        <span className="text-tertiary text-caption">未选股票</span>
      )}
      {extra}
      <StockSearchBox
        className="ml-auto w-36 shrink-0"
        placeholder="换股票…"
        ariaLabel="搜索要查看的股票"
        excludeIndexes
        onPick={onPick}
      />
    </header>
  )
}
