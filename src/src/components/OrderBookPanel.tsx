import { OrderBookLadder } from "@/components/OrderBookLadder"
import { useOrderBook } from "@/hooks/query/useOrderBook"

/**
 * 五档行情面板：标题 + 盘口十档（同花顺式排版见 `OrderBookLadder`）。
 *
 * 数据走行情通道（`useOrderBook`，后端每个 symbol 只跑一条上游轮询），
 * 所以面板开着的时间越长数据越新，而不是打开那一刻的快照。
 */
export function OrderBookPanel({
  symbol,
  name,
  placeholder = "选择股票后显示五档行情",
  className,
}: {
  symbol: string
  name?: string
  /** 没有标的时的说明文案（弹窗与格子里的措辞不同） */
  placeholder?: string
  className?: string
}) {
  const { book, error } = useOrderBook(symbol)

  if (!symbol) {
    return (
      <div
        className={`border-hairline text-tertiary text-caption flex h-full min-h-40 items-center justify-center rounded-lg border border-dashed px-3 text-center ${
          className ?? ""
        }`}
      >
        {placeholder}
      </div>
    )
  }

  return (
    <div className={`border-hairline min-w-0 rounded-lg border px-1 py-1.5 ${className ?? ""}`}>
      <div className="text-tertiary text-caption mb-1 px-1">五档{name ? ` · ${name}` : ""}</div>
      <OrderBookLadder book={book} error={error} />
    </div>
  )
}
