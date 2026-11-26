import { StockSearchBox } from "@/components/StockSearchBox"
import { useActiveStockStore } from "@/stores/activeStockStore"

/**
 * 跟随型格子的表头：工作区当前标的（名称 + 代码）+ 右侧"换股票"搜索框。
 *
 * 「五档行情」「下单」「关键指标」「所属板块」共用这一份——它们都读
 * `activeStockStore`，表头长得一样，换票入口也只该有一个写法。
 */
export function ActiveStockHeader({ extra }: { extra?: React.ReactNode }) {
  const stock = useActiveStockStore((s) => s.stock)
  const setStock = useActiveStockStore((s) => s.set)

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
        onPick={(code, name) => setStock({ code, name })}
      />
    </header>
  )
}
