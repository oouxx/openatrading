import { useState } from "react"
import { ChevronLeft } from "lucide-react"
import { Button } from "@/components/ui/button"
import { DataLoading } from "@/components/data-state"
import {
  useDragonTigerStockSeat,
  type DragonTigerDetailItem,
  type SeatDetail,
} from "@/hooks/query/useDragonTiger"
import { formatCompactNumber } from "@/lib/format"
import { cn } from "@/lib/utils"

/**
 * 龙虎榜个股详情（上榜指标 + 买卖席位），作为面板里的"下一层页面"。
 *
 * 之前它是整屏 Dialog：在 3×3 的一格里点开一个盖住全屏的弹窗，既跳出了格子的
 * 上下文，又得靠 Esc/点外面才能退出。现在和板块详情、K 线同一套：开在格子里、
 * 左上角返回、Esc 也能退。
 */
export default function DragonTigerDetailView({
  item,
  onBack,
}: {
  item: DragonTigerDetailItem
  onBack: () => void
}) {
  const [selectedDate, setSelectedDate] = useState<string | null>(item.trade_date)
  const seatDate = selectedDate ?? item.trade_date ?? null
  const seatQueryDate = seatDate ? seatDate.slice(0, 10).replace(/-/g, "") : null
  const { data: buySeats = [], isLoading: buyLoading } = useDragonTigerStockSeat(
    item.code,
    seatQueryDate,
    "buy"
  )
  const { data: sellSeats = [], isLoading: sellLoading } = useDragonTigerStockSeat(
    item.code,
    seatQueryDate,
    "sell"
  )

  const reasons = item.reasons?.length ? item.reasons : item.explanation ? [item.explanation] : []

  const metrics = [
    { label: "收盘价", value: item.close_price?.toFixed(2) || "—", up: undefined },
    {
      label: "涨跌幅",
      value: item.change_rate != null ? `${item.change_rate.toFixed(2)}%` : "—",
      up: (item.change_rate || 0) >= 0,
    },
    {
      label: "换手率",
      value: item.turnover_rate != null ? `${item.turnover_rate.toFixed(2)}%` : "—",
      up: undefined,
    },
    {
      label: "流通市值",
      value: item.free_market_cap ? formatCompactNumber(item.free_market_cap) : "—",
      up: undefined,
    },
  ]

  return (
    <div className="@container space-y-4 p-4 @2xl:p-6">
      <div className="flex items-start gap-2">
        <Button
          type="button"
          variant="ghost"
          size="icon-xs"
          aria-label="返回龙虎榜"
          title="返回"
          onClick={onBack}
        >
          <ChevronLeft className="size-4" />
        </Button>
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-baseline gap-2">
            <h2 className="text-body text-primary truncate font-semibold">{item.name}</h2>
            <span className="text-tertiary font-number text-caption">{item.code}</span>
            <span className="text-tertiary font-number text-caption">{seatDate?.slice(0, 10)}</span>
          </div>
        </div>
      </div>

      {(item.trade_dates?.length ?? 0) > 1 && (
        <div className="flex flex-wrap items-center gap-2">
          <span className="text-tertiary text-caption">席位日期</span>
          {item.trade_dates?.map((date) => (
            <Button
              key={date}
              type="button"
              variant={selectedDate === date ? "secondary" : "ghost"}
              size="xs"
              onClick={() => setSelectedDate(date)}
              className="text-caption font-number h-auto px-2 py-0.5"
            >
              {date.slice(5)}
            </Button>
          ))}
        </div>
      )}

      <div className="bg-hairline border-hairline grid grid-cols-2 gap-px overflow-hidden rounded-lg border @2xl:grid-cols-4">
        {metrics.map((metric) => (
          <div key={metric.label} className="bg-canvas px-3 py-3">
            <p className="text-tertiary text-micro">{metric.label}</p>
            <p
              className={cn(
                "font-number text-body mt-1 font-medium",
                metric.up === undefined ? "text-primary" : metric.up ? "text-up" : "text-primary"
              )}
            >
              {metric.value}
            </p>
          </div>
        ))}
      </div>

      {reasons.length > 0 && (
        <div>
          <h4 className="text-tertiary text-micro mb-2">上榜原因</h4>
          <ul className="text-secondary text-caption space-y-1.5 leading-[1.7]">
            {reasons.map((reason) => (
              <li key={reason} className="flex gap-2">
                <span className="bg-brand/50 mt-2 size-1 shrink-0 rounded-full" aria-hidden />
                {reason}
              </li>
            ))}
          </ul>
        </div>
      )}

      {buyLoading || sellLoading ? (
        <DataLoading label="加载席位数据…" size="compact" />
      ) : (
        <div className="space-y-6">
          {buySeats.length > 0 && (
            <div>
              <h4 className="text-up text-caption mb-2 font-medium">
                买方席位
                <span className="text-tertiary font-number ml-1.5 font-normal">
                  {buySeats.length}
                </span>
              </h4>
              <SeatTable seats={buySeats} side="buy" />
            </div>
          )}
          {sellSeats.length > 0 && (
            <div>
              <h4 className="text-down text-caption mb-2 font-medium">
                卖方席位
                <span className="text-tertiary font-number ml-1.5 font-normal">
                  {sellSeats.length}
                </span>
              </h4>
              <SeatTable seats={sellSeats} side="sell" />
            </div>
          )}
        </div>
      )}
    </div>
  )
}

function SeatTable({ seats, side }: { seats: SeatDetail[]; side: "buy" | "sell" }) {
  const isBuy = side === "buy"
  return (
    <div className="border-hairline overflow-hidden rounded-md border">
      <div className="overflow-x-auto">
        <table className="data-table">
          <thead>
            <tr>
              <th>营业部</th>
              <th className="num">{isBuy ? "买入" : "卖出"}</th>
              <th className="num">{isBuy ? "卖出" : "买入"}</th>
              <th className="num">净额</th>
            </tr>
          </thead>
          <tbody>
            {seats.map((seat) => (
              <tr key={`${seat.dept_name}-${seat.buy_amt}-${seat.sell_amt}`}>
                <td className="max-w-[18rem] truncate" title={seat.dept_name}>
                  {seat.dept_name}
                </td>
                <td className="num">{formatAmount(isBuy ? seat.buy_amt : seat.sell_amt)}</td>
                <td className="num">{formatAmount(isBuy ? seat.sell_amt : seat.buy_amt)}</td>
                <td
                  className={cn(
                    "num font-medium",
                    (seat.net_amt ?? 0) >= 0 ? "text-up" : "text-down"
                  )}
                >
                  {formatAmount(seat.net_amt)}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  )
}

function formatAmount(value: number | null | undefined): string {
  if (value == null || !Number.isFinite(value)) return "—"
  const yi = value / 1e8
  return `${yi >= 0 ? "+" : ""}${yi.toFixed(2)} 亿`
}
