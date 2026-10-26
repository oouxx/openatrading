import type { OrderBook, OrderBookLevel } from "@/stores/data"
import { pctClass } from "@/lib/format"

/**
 * 盘口五档。
 *
 * 排版按同花顺：卖五在上、买五在下，中间夹一行现价。价格按「相对昨收」着色
 * （A 股习惯），量用一根按最大挂单量归一化的横条——一眼能看出哪一档有厚单。
 *
 * 关键边界：**非交易时段/停牌各档为 0**。这时整块换成一句状态说明，而不是
 * 渲染十行 0.00 —— 十行 0 会被读成「有十个空档位」，其实是没有数据。
 */
export function OrderBookLadder({ book, error }: { book?: OrderBook; error?: string | null }) {
  if (error) {
    return <p className="text-tertiary text-caption px-1 py-2">盘口不可用：{error}</p>
  }
  if (!book) {
    return (
      <div className="flex flex-col gap-1.5 px-1 py-1">
        {Array.from({ length: 4 }).map((_, i) => (
          <div key={i} className="bg-hover/60 h-4 rounded" />
        ))}
      </div>
    )
  }

  const hasDepth = [...book.asks, ...book.bids].some((l) => l.price > 0)
  const maxVolume = Math.max(...[...book.asks, ...book.bids].map((l) => l.volume), 0)
  const tone = (price: number) =>
    book.prev_close > 0 && price > 0 ? pctClass(price - book.prev_close) : "text-secondary"

  if (!hasDepth) {
    return (
      <p className="text-tertiary text-caption px-1 py-2 leading-5">
        非交易时段或停牌，暂无挂单。
      </p>
    )
  }

  return (
    <div className="text-caption">
      <div className="text-tertiary mb-1 grid grid-cols-[2.5rem_1fr_3.5rem] items-center gap-1 px-1">
        <span />
        <span>价格</span>
        <span className="text-right">量(手)</span>
      </div>
      {/* 卖五 → 卖一（离现价由远到近） */}
      {[...book.asks].reverse().map((level, i) => (
        <LevelRow
          key={`ask-${i}`}
          label={`卖${book.asks.length - i}`}
          level={level}
          maxVolume={maxVolume}
          tone={tone(level.price)}
          side="ask"
        />
      ))}
      <div className="border-hairline my-1 flex items-baseline justify-between border-y px-1 py-1">
        <span className="text-tertiary">现价</span>
        <span className={`font-number font-medium ${pctClass(book.change_pct)}`}>
          {book.last > 0 ? book.last.toFixed(2) : "—"}
        </span>
      </div>
      {/* 买一 → 买五 */}
      {book.bids.map((level, i) => (
        <LevelRow
          key={`bid-${i}`}
          label={`买${i + 1}`}
          level={level}
          maxVolume={maxVolume}
          tone={tone(level.price)}
          side="bid"
        />
      ))}
    </div>
  )
}

function LevelRow({
  label,
  level,
  maxVolume,
  tone,
  side,
}: {
  label: string
  level: OrderBookLevel
  maxVolume: number
  tone: string
  side: "ask" | "bid"
}) {
  const empty = level.price <= 0
  const width = maxVolume > 0 ? Math.round((level.volume / maxVolume) * 100) : 0
  return (
    <div className="relative grid grid-cols-[2.5rem_1fr_3.5rem] items-center gap-1 px-1 py-[3px]">
      {/* 挂单量条：从右往左铺，避开左侧档位标签 */}
      <span
        aria-hidden
        className={`absolute inset-y-0 right-0 rounded-l-sm ${side === "ask" ? "bg-down/10" : "bg-up/10"}`}
        style={{ width: `${width}%` }}
      />
      <span className="text-tertiary relative">{label}</span>
      <span className={`font-number relative ${empty ? "text-tertiary" : tone}`}>
        {empty ? "—" : level.price.toFixed(2)}
      </span>
      <span className="text-secondary font-number relative text-right">
        {empty ? "—" : Math.round(level.volume)}
      </span>
    </div>
  )
}
