import { useState } from "react"
import {
  ArrowDownRight,
  ArrowUpRight,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  RefreshCw,
} from "lucide-react"

import { Button } from "@/components/ui/button"
import { Skeleton } from "@/components/ui/skeleton"
import {
  usePlateDetail,
  usePlateRankings,
  type PlateRankType,
  type PlateStock,
  type PlateRankingItem,
} from "@/hooks/query/usePlateRanking"
import { formatCompactNumber, formatPct, formatRatioPct, pctClass } from "@/lib/format"
import { cn } from "@/lib/utils"
import { useRightPanelStore } from "@/stores/rightPanelStore"

const RANK_TYPES: { id: PlateRankType; label: string }[] = [
  { id: 0, label: "全部" },
  { id: 2, label: "行业" },
  { id: 1, label: "概念" },
  { id: 3, label: "风格" },
]

function updatedAt(timestamp: number): string {
  if (!Number.isFinite(timestamp) || timestamp <= 0) return ""
  return new Date(timestamp * 1000).toLocaleTimeString("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
  })
}

function priceText(value: number | null | undefined): string {
  return value != null && Number.isFinite(value) ? value.toFixed(2) : "—"
}

function percentText(value: number | null | undefined): string {
  return value != null && Number.isFinite(value) ? `${value.toFixed(2)}%` : "—"
}

function ratioText(value: number | null | undefined): string {
  return value != null && Number.isFinite(value) ? formatRatioPct(value) : "—"
}

function compactText(value: number | null | undefined): string {
  return value != null && Number.isFinite(value) ? formatCompactNumber(value) : "—"
}

function quoteTimeText(timestamp: number | null | undefined): string {
  if (timestamp == null || !Number.isFinite(timestamp) || timestamp <= 0) return "—"
  return new Date(timestamp * 1000).toLocaleTimeString("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  })
}

function latestLimitUpText(timestamp: number | null | undefined): string {
  if (timestamp == null || !Number.isFinite(timestamp) || timestamp <= 0) return "—"
  return new Date(timestamp * 1000).toLocaleString("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  })
}

function limitUpDaysText(days: number | null | undefined): string {
  if (days == null || !Number.isFinite(days) || days <= 0) return "—"
  return days === 1 ? "首板" : `${days}连板`
}

function sortStocksByChangeRate(stocks: PlateStock[]): PlateStock[] {
  return [...stocks].sort((left, right) => {
    const leftRate = left.px_change_rate
    const rightRate = right.px_change_rate
    const leftValid = leftRate != null && Number.isFinite(leftRate)
    const rightValid = rightRate != null && Number.isFinite(rightRate)
    if (leftValid && rightValid) return rightRate - leftRate
    if (leftValid) return -1
    if (rightValid) return 1
    return 0
  })
}

function tradeStatusText(status: string | null | undefined): string {
  if (!status) return "—"
  if (status === "TRADE") return "交易"
  if (status === "SUSP") return "停牌"
  return status
}

function labelClass(effect: number): string {
  if (effect > 0) return "bg-up/10 text-up"
  if (effect < 0) return "bg-down/10 text-down"
  return "bg-hover text-secondary"
}

function rankChangeText(change: number | null | undefined): string {
  if (change == null || !Number.isFinite(change) || change === 0) return "—"
  return change > 0 ? `↑${change}` : `↓${Math.abs(change)}`
}

export default function PlateRankingTab() {
  const [selected, setSelected] = useState<PlateRankingItem | null>(null)
  const [rankType, setRankType] = useState<PlateRankType>(0)
  const rankings = usePlateRankings(rankType)

  if (selected) {
    return <PlateDetailView plate={selected} onBack={() => setSelected(null)} />
  }

  if (rankings.isPending) {
    return (
      <div className="space-y-4 p-4">
        <Skeleton className="h-10 w-full" />
        <Skeleton className="h-64 w-full" />
        <Skeleton className="h-64 w-full" />
      </div>
    )
  }

  if (rankings.error) {
    return (
      <div className="p-4">
        <div className="bg-surface border-hairline rounded-lg border p-5">
          <p className="text-caption text-danger">加载失败：{rankings.error.message}</p>
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="mt-3"
            onClick={() => void rankings.refetch()}
          >
            重试
          </Button>
        </div>
      </div>
    )
  }

  const data = rankings.data ?? { generated_at: 0, gainers: [], losers: [] }

  return (
    <div className="space-y-4 p-4">
      <div className="flex items-center gap-1">
        {RANK_TYPES.map((type) => {
          const active = rankType === type.id
          return (
            <Button
              key={type.id}
              type="button"
              variant="ghost"
              size="sm"
              aria-pressed={active}
              onClick={() => setRankType(type.id)}
              className={cn(
                "text-caption rounded-full px-3",
                active
                  ? "bg-brand/20 text-brand hover:bg-brand/20 hover:text-brand"
                  : "text-secondary hover:bg-hover hover:text-primary"
              )}
            >
              {type.label}
            </Button>
          )
        })}
      </div>

      <div className="flex items-center justify-between">
        <span className="text-caption text-tertiary">
          {data.generated_at ? `${updatedAt(data.generated_at)} 更新` : "当日板块"}
        </span>
        <Button
          type="button"
          variant="ghost"
          size="icon-xs"
          aria-label="刷新板块排行"
          title="刷新"
          disabled={rankings.isFetching}
          onClick={() => void rankings.refetch()}
        >
          <RefreshCw className={cn("size-3.5", rankings.isFetching && "animate-spin")} />
        </Button>
      </div>

      <RankingSection
        title="涨幅前 10"
        items={data.gainers}
        direction="up"
        onSelect={setSelected}
      />
      <RankingSection
        title="跌幅前 10"
        items={data.losers}
        direction="down"
        onSelect={setSelected}
      />
    </div>
  )
}

function RankingSection({
  title,
  items,
  direction,
  onSelect,
}: {
  title: string
  items: PlateRankingItem[]
  direction: "up" | "down"
  onSelect: (item: PlateRankingItem) => void
}) {
  const Icon = direction === "up" ? ArrowUpRight : ArrowDownRight

  return (
    <section className="bg-surface border-hairline overflow-hidden rounded-lg border">
      <div className="border-hairline flex items-center justify-between border-b px-4 py-3">
        <div className="flex items-center gap-2">
          <Icon className={cn("size-4", direction === "up" ? "text-up" : "text-down")} />
          <h2 className="text-caption text-primary font-medium">{title}</h2>
        </div>
        <span className="text-caption font-number text-tertiary">{items.length}</span>
      </div>
      {items.length === 0 ? (
        <div className="text-caption text-tertiary px-4 py-8 text-center">暂无板块</div>
      ) : (
        <div className="divide-hairline divide-y">
          {items.map((item, index) => (
            <button
              key={item.plate_id}
              type="button"
              onClick={() => onSelect(item)}
              className="hover:bg-hover group block w-full px-4 py-3 text-left transition-colors"
            >
              <div className="flex items-center gap-3">
                <span className="text-caption font-number text-tertiary w-4 tabular-nums">
                  {index + 1}
                </span>
                <span className="text-body text-primary min-w-0 flex-1 truncate">
                  {item.plate_name}
                </span>
                <span className={cn("text-caption font-number font-semibold", pctClass(item.pcp))}>
                  {formatRatioPct(item.pcp)}
                </span>
                <ChevronRight className="text-tertiary size-3.5 transition-transform group-hover:translate-x-0.5" />
              </div>

              <div className="text-caption font-number text-secondary mt-2 grid grid-cols-2 gap-x-4 gap-y-1 pl-7">
                <span>资金流 {compactText(item.fund_flow)}</span>
                <span>
                  涨跌家数 {item.rise_count}/{item.fall_count}/{item.stay_count}
                </span>
                <span>涨停 {item.limit_up_count}</span>
                <span>
                  核心排名 {item.core_avg_pcp_rank ?? "—"}{" "}
                  <span
                    className={cn(
                      item.core_avg_pcp_rank_change != null && item.core_avg_pcp_rank_change > 0
                        ? "text-up"
                        : item.core_avg_pcp_rank_change != null && item.core_avg_pcp_rank_change < 0
                          ? "text-down"
                          : "text-tertiary"
                    )}
                  >
                    {rankChangeText(item.core_avg_pcp_rank_change)}
                  </span>
                </span>
              </div>

              <LeaderStocksLine label="领涨" stocks={item.top_n_stocks?.items ?? []} />
              <LeaderStocksLine label="领跌" stocks={item.bottom_n_stocks?.items ?? []} />
            </button>
          ))}
        </div>
      )}
    </section>
  )
}

function LeaderStocksLine({
  label,
  stocks,
}: {
  label: string
  stocks: PlateRankingItem["top_n_stocks"]["items"]
}) {
  return (
    <div className="text-caption mt-1 flex items-start gap-2 pl-7">
      <span className="text-tertiary shrink-0">{label}</span>
      {stocks.length === 0 ? (
        <span className="text-tertiary">—</span>
      ) : (
        <span className="flex min-w-0 flex-wrap gap-x-2 gap-y-1">
          {stocks.map((stock) => (
            <span key={stock.symbol} className="text-secondary">
              {stock.stock_chi_name || stock.symbol}
              {stock.change_percent != null && (
                <span className={cn("font-number ml-1", pctClass(stock.change_percent))}>
                  {formatRatioPct(stock.change_percent)}
                </span>
              )}
            </span>
          ))}
        </span>
      )}
    </div>
  )
}

function PlateDetailView({ plate, onBack }: { plate: PlateRankingItem; onBack: () => void }) {
  const detail = usePlateDetail(plate.plate_id)
  const data = detail.data
  const stocks = data ? sortStocksByChangeRate(data.stocks) : []

  return (
    <div className="space-y-4 p-4">
      <div className="flex items-start gap-2">
        <Button
          type="button"
          variant="ghost"
          size="icon-xs"
          aria-label="返回板块排行"
          title="返回"
          onClick={onBack}
        >
          <ChevronLeft className="size-4" />
        </Button>
        <div className="min-w-0 flex-1">
          <div className="flex items-baseline gap-2">
            <h2 className="text-body text-primary truncate font-semibold">{plate.plate_name}</h2>
            <span className={cn("text-caption font-number", pctClass(data?.avg_pcp ?? plate.pcp))}>
              {formatRatioPct(data?.avg_pcp ?? plate.pcp)}
            </span>
          </div>
          {data?.desc && (
            <p className="text-caption text-secondary mt-1 leading-relaxed">{data.desc}</p>
          )}
        </div>
      </div>

      {detail.isPending ? (
        <div className="space-y-3">
          <Skeleton className="h-36 w-full" />
          <Skeleton className="h-64 w-full" />
        </div>
      ) : detail.error ? (
        <div className="bg-surface border-hairline rounded-lg border p-5">
          <p className="text-caption text-danger">加载失败：{detail.error.message}</p>
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="mt-3"
            onClick={() => void detail.refetch()}
          >
            重试
          </Button>
        </div>
      ) : data ? (
        <>
          <section className="bg-surface border-hairline rounded-lg border p-3">
            <div className="grid grid-cols-2 gap-x-4 gap-y-3">
              <DetailMetric label="平均涨幅" value={ratioText(data.avg_pcp)} />
              <DetailMetric label="核心涨幅" value={ratioText(data.core_avg_pcp)} />
              <DetailMetric label="题材资金流" value={compactText(data.fund_flow)} />
              <DetailMetric label="核心股" value={String(data.core_stocks_count)} />
              <DetailMetric label="龙头股" value={String(data.hang_ye_long_tou_stocks_count)} />
              <DetailMetric label="北向关联" value={data.has_north > 0 ? "有" : "—"} />
            </div>
            <div className="border-hairline mt-3 grid grid-cols-3 gap-2 border-t pt-3">
              <CountItem label="上涨" value={data.rise_count} tone="up" compact />
              <CountItem label="下跌" value={data.fall_count} tone="down" compact />
              <CountItem label="平盘" value={data.stay_count} compact />
            </div>
            {data.sub_sets?.length ? (
              <div className="border-hairline mt-3 flex flex-wrap gap-1.5 border-t pt-3">
                {data.sub_sets.map((subset) => (
                  <span
                    key={subset.plate_id}
                    className="bg-hover text-caption text-secondary rounded-sm px-2 py-0.5"
                  >
                    {subset.plate_name}
                  </span>
                ))}
              </div>
            ) : null}
            {data.premium_desc && (
              <p className="text-caption text-tertiary mt-3 leading-relaxed">{data.premium_desc}</p>
            )}
          </section>

          <section className="bg-surface border-hairline overflow-hidden rounded-lg border">
            <div className="border-hairline flex items-center justify-between border-b px-4 py-3">
              <h3 className="text-caption text-primary font-medium">成分股</h3>
              <span className="text-caption font-number text-tertiary">{data.stocks.length}</span>
            </div>
            {data.stocks.length === 0 ? (
              <div className="text-caption text-tertiary px-4 py-10 text-center">暂无成分股</div>
            ) : (
              <div className="divide-hairline divide-y">
                {stocks.map((stock) => (
                  <ComponentStockRow key={stock.symbol} stock={stock} />
                ))}
              </div>
            )}
          </section>
        </>
      ) : null}
    </div>
  )
}

function ComponentStockRow({ stock }: { stock: PlateStock }) {
  const [expanded, setExpanded] = useState(false)
  const openKline = useRightPanelStore((state) => state.openKline)
  const change = stock.px_change_rate
  const name = stock.name || stock.symbol
  const labels = stock.labels ?? []

  return (
    <div className="hover:bg-hover/40 transition-colors">
      <div className="flex items-center gap-2 px-4 py-2.5">
        <button
          type="button"
          onClick={() => openKline(stock.symbol, name)}
          className="flex min-w-0 flex-1 items-center gap-3 text-left"
        >
          <span className="text-caption font-number text-secondary w-20 shrink-0">
            {stock.symbol.replace(/\.[A-Z]+$/, "")}
          </span>
          <span className="text-body text-primary min-w-0 flex-1 truncate">{name}</span>
          <span className="text-caption font-number text-primary">{priceText(stock.last_px)}</span>
          <span className={cn("text-caption font-number font-semibold", pctClass(change))}>
            {change != null ? formatPct(change) : "—"}
          </span>
        </button>
        {stock.is_new && (
          <span className="bg-warning/15 text-warning text-caption shrink-0 rounded-sm px-1.5 py-0.5">
            新
          </span>
        )}
        <Button
          type="button"
          variant="ghost"
          size="icon-xs"
          aria-label={expanded ? `收起 ${name}` : `展开 ${name}`}
          title={expanded ? "收起字段" : "展开全部字段"}
          onClick={() => setExpanded((value) => !value)}
        >
          <ChevronDown className={cn("size-3.5 transition-transform", expanded && "rotate-180")} />
        </Button>
      </div>

      <div className="text-caption font-number text-secondary flex flex-wrap gap-x-4 gap-y-1 px-4 pb-2.5 pl-[6.5rem]">
        <span>换手 {percentText(stock.turnover_ratio)}</span>
        <span>流通市值 {compactText(stock.circulation_value)}</span>
        <span>总市值 {compactText(stock.market_value)}</span>
        <span className={pctClass(stock.limit_up_days)}>
          连板 {limitUpDaysText(stock.limit_up_days)}
        </span>
        <span>涨停 {latestLimitUpText(stock.last_limit_up)}</span>
      </div>

      {stock.desc && (
        <p className="text-caption text-tertiary px-4 pb-2.5 pl-[6.5rem] leading-relaxed break-words whitespace-pre-wrap">
          {stock.desc}
        </p>
      )}

      {labels.length > 0 && (
        <div className="flex flex-wrap gap-1.5 px-4 pb-2.5 pl-[6.5rem]">
          {labels.slice(0, 3).map((label, index) => (
            <span
              key={`${label.label_type}-${label.label_name}-${index}`}
              title={label.description}
              className={cn("text-caption rounded-sm px-1.5 py-0.5", labelClass(label.effect))}
            >
              {label.label_name}
            </span>
          ))}
          {labels.length > 3 && (
            <span className="text-caption text-tertiary px-1 py-0.5">+{labels.length - 3}</span>
          )}
        </div>
      )}

      {expanded && (
        <div className="border-hairline bg-canvas/50 mx-4 mb-3 grid grid-cols-2 gap-x-4 gap-y-3 rounded-lg border p-3">
          <DetailMetric label="涨跌额" value={priceText(stock.px_change)} tone={stock.px_change} />
          <DetailMetric label="今开" value={priceText(stock.open_px)} />
          <DetailMetric label="昨收" value={priceText(stock.preclose_px)} />
          <DetailMetric label="最高" value={priceText(stock.high_px)} />
          <DetailMetric label="最低" value={priceText(stock.low_px)} />
          <DetailMetric label="振幅" value={percentText(stock.amplitude)} />
          <DetailMetric label="成交量" value={compactText(stock.turnover_volume)} />
          <DetailMetric label="流通市值" value={compactText(stock.circulation_value)} />
          <DetailMetric label="市盈率" value={priceText(stock.pe_rate)} />
          <DetailMetric label="动态 PE" value={priceText(stock.dyn_pe)} />
          <DetailMetric label="市净率" value={priceText(stock.dyn_pb_rate)} />
          <DetailMetric label="总股本" value={compactText(stock.total_shares)} />
          <DetailMetric label="流通股本" value={compactText(stock.circulation_shares)} />
          <DetailMetric label="交易状态" value={tradeStatusText(stock.trade_status)} />
          <DetailMetric label="更新时间" value={quoteTimeText(stock.update_time)} />
          <DetailMetric label="产业链" value={stock.industrial_chain_name || "—"} />
          <DetailMetric label="付费内容" value={stock.need_pay ? "是" : "否"} />
          <DetailMetric label="核心标记（上游）" value={(stock.core_flag ?? 0) > 0 ? "是" : "—"} />
          <div className="border-hairline col-span-2 border-t pt-3">
            <div className="text-caption text-tertiary mb-2">股票标签</div>
            {labels.length === 0 ? (
              <div className="text-caption text-tertiary">暂无标签</div>
            ) : (
              <div className="space-y-2">
                {labels.map((label, index) => (
                  <div
                    key={`${label.label_type}-${label.label_name}-${index}`}
                    className="flex items-start gap-2"
                  >
                    <span
                      className={cn(
                        "text-caption shrink-0 rounded-sm px-1.5 py-0.5",
                        labelClass(label.effect)
                      )}
                    >
                      {label.label_name}
                    </span>
                    <span className="text-caption text-secondary leading-relaxed">
                      {label.description || label.label_type}
                    </span>
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  )
}

function DetailMetric({
  label,
  value,
  tone,
}: {
  label: string
  value: string
  tone?: number | null
}) {
  return (
    <div>
      <div className="text-caption text-tertiary">{label}</div>
      <div className={cn("text-caption font-number text-primary mt-0.5", pctClass(tone))}>
        {value}
      </div>
    </div>
  )
}

function CountItem({
  label,
  value,
  tone,
  compact = false,
}: {
  label: string
  value: number
  tone?: "up" | "down"
  compact?: boolean
}) {
  return (
    <div
      className={cn(
        compact ? "text-center" : "bg-surface border-hairline rounded-lg border px-3 py-2.5"
      )}
    >
      <div className="text-caption text-tertiary">{label}</div>
      <div
        className={cn(
          compact
            ? "text-caption font-number mt-0.5 font-semibold"
            : "text-body font-number mt-0.5 font-semibold",
          tone === "up" ? "text-up" : tone === "down" ? "text-down" : "text-primary"
        )}
      >
        {value}
      </div>
    </div>
  )
}
