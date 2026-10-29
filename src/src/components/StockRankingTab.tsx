import { useEffect, useMemo, useRef } from "react"
import { ArrowDownRight, ArrowUpRight, RefreshCw } from "lucide-react"

import { Button } from "@/components/ui/button"
import { DataEmpty, DataError, DataLoading, DataStale } from "@/components/data-state"
import { Checkbox } from "@/components/ui/checkbox"
import {
  useStockRanking,
  type RankedStock,
  type StockMarketFilter,
  type StockRankingOrder,
} from "@/hooks/query/useStockRanking"
import { formatCompactNumber, formatPct, pctClass } from "@/lib/format"
import { mergeStockRankingPages } from "@/lib/stock-ranking"
import { cn } from "@/lib/utils"
import { useRightPanelStore } from "@/stores/rightPanelStore"

const ORDERS: { id: StockRankingOrder; label: string }[] = [
  { id: "gainers", label: "涨幅榜" },
  { id: "losers", label: "跌幅榜" },
]

const MARKETS: { id: StockMarketFilter; label: string }[] = [
  { id: "all", label: "全部" },
  { id: "main", label: "沪深主板" },
  { id: "chinext", label: "创业板" },
  { id: "star", label: "科创板" },
]

function numberText(value: number | null | undefined, digits = 2): string {
  return value != null && Number.isFinite(value) ? value.toFixed(digits) : "—"
}

function percentText(value: number | null | undefined): string {
  return value != null && Number.isFinite(value) ? formatPct(value) : "—"
}

function codeText(symbol: string, code: string): string {
  return code || symbol.replace(/\.[A-Z]+$/, "")
}

export default function StockRankingTab() {
  const order = useRightPanelStore((state) => state.stockRankingOrder)
  const setOrder = useRightPanelStore((state) => state.setStockRankingOrder)
  const market = useRightPanelStore((state) => state.stockRankingMarket)
  const setMarket = useRightPanelStore((state) => state.setStockRankingMarket)
  const excludeSt = useRightPanelStore((state) => state.stockRankingExcludeSt)
  const setExcludeSt = useRightPanelStore((state) => state.setStockRankingExcludeSt)
  const openKline = useRightPanelStore((state) => state.openKline)
  const query = useStockRanking(order, market, excludeSt)
  const stocks = useMemo(
    () => mergeStockRankingPages(query.data?.pages ?? []),
    [query.data?.pages]
  )
  const marketLabel = MARKETS.find((item) => item.id === market)?.label ?? "全部"
  const latestPage = query.data?.pages.at(-1)
  const updatedAt = latestPage?.generated_at
    ? new Date(latestPage.generated_at * 1000).toLocaleTimeString("zh-CN", {
        hour: "2-digit",
        minute: "2-digit",
        second: "2-digit",
      })
    : ""
  const loadMoreRef = useRef<HTMLDivElement | null>(null)
  const { hasNextPage, isFetchingNextPage, fetchNextPage } = query

  useEffect(() => {
    const target = loadMoreRef.current
    if (!target || !hasNextPage || isFetchingNextPage) return
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) {
          void fetchNextPage()
        }
      },
      { rootMargin: "200px" }
    )
    observer.observe(target)
    return () => observer.disconnect()
  }, [hasNextPage, isFetchingNextPage, fetchNextPage, stocks.length])

  return (
    <div className="space-y-3 p-4">
      <div className="flex items-center justify-between gap-3">
        <div className="bg-hover/60 border-hairline inline-flex rounded-md border p-0.5">
          {ORDERS.map((item) => {
            const active = order === item.id
            const Icon = item.id === "gainers" ? ArrowUpRight : ArrowDownRight
            return (
              <button
                key={item.id}
                type="button"
                aria-pressed={active}
                onClick={() => setOrder(item.id)}
                className={cn(
                  "text-caption focus-visible:ring-brand/40 flex h-7 items-center gap-1 rounded-sm px-2.5 transition-colors outline-none focus-visible:ring-2",
                  active
                    ? "bg-surface text-primary shadow-card"
                    : "text-tertiary hover:text-secondary"
                )}
              >
                <Icon className={cn("size-3.5", item.id === "gainers" ? "text-up" : "text-down")} />
                {item.label}
              </button>
            )
          })}
        </div>
        <label className="text-caption text-secondary flex cursor-pointer items-center gap-1.5">
          <Checkbox
            checked={excludeSt}
            onCheckedChange={(checked) => setExcludeSt(checked === true)}
          />
          排除 ST
        </label>
      </div>

      <div className="border-hairline flex items-center gap-0.5 border-b">
        {MARKETS.map((item) => (
          <Button
            key={item.id}
            type="button"
            variant="ghost"
            size="sm"
            aria-pressed={market === item.id}
            onClick={() => setMarket(item.id)}
            className={cn(
              "text-caption h-8 rounded-none border-b-2 border-transparent px-2.5",
              market === item.id
                ? "border-b-brand text-primary hover:bg-transparent"
                : "text-tertiary hover:bg-transparent hover:text-secondary"
            )}
          >
            {item.label}
          </Button>
        ))}
      </div>

      <div className="flex items-center justify-between">
        <div className="text-caption text-tertiary flex items-center gap-2">
          <span>
            {marketLabel} · {stocks.length} 只
          </span>
          {updatedAt && <span>{updatedAt} 更新</span>}
          {query.data?.pages.some((page) => page.stale) && <DataStale />}
        </div>
        <Button
          type="button"
          variant="ghost"
          size="icon-xs"
          aria-label="刷新个股排行"
          title="刷新"
          disabled={query.isFetching}
          onClick={() => void query.refetch()}
        >
          <RefreshCw className={cn("size-3.5", query.isFetching && "animate-spin")} />
        </Button>
      </div>

      {query.isPending && stocks.length === 0 ? (
        <DataLoading label="加载个股排行…" size="table" rows={6} />
      ) : query.error ? (
        <DataError
          title="个股排行获取失败"
          description={query.error.message}
          onRetry={() => void query.refetch()}
        />
      ) : stocks.length === 0 ? (
        <DataEmpty title="暂无股票" description="当前筛选条件下没有可展示的个股" size="table" />
      ) : (
        <div
          className={cn(
            "bg-surface border-hairline overflow-hidden rounded-lg border transition-opacity",
            query.isFetching && !query.isFetchingNextPage && "opacity-75"
          )}
        >
          <table className="data-table">
            <thead>
              <tr>
                <th>#</th>
                <th>代码</th>
                <th>名称</th>
                <th className="num">最新价</th>
                <th className="num">涨跌幅</th>
                <th className="num">换手率</th>
                <th className="num">流通市值</th>
              </tr>
            </thead>
            <tbody>
              {stocks.map((stock, index) => (
                <RankingRow
                  key={stock.symbol}
                  stock={stock}
                  rank={index + 1}
                  onOpen={() => openKline(stock.symbol, stock.name)}
                />
              ))}
            </tbody>
          </table>
          <div
            ref={loadMoreRef}
            className="border-hairline text-caption text-tertiary flex min-h-9 items-center justify-center border-t px-3"
          >
            {query.isFetchingNextPage
              ? "加载更多…"
              : query.hasNextPage
                ? "滚动加载更多"
                : "已加载全部"}
          </div>
        </div>
      )}
    </div>
  )
}

function RankingRow({
  stock,
  rank,
  onOpen,
}: {
  stock: RankedStock
  rank: number
  onOpen: () => void
}) {
  return (
    <tr className="cursor-pointer" onClick={onOpen}>
      <td className="text-tertiary font-number w-8">{rank}</td>
      <td className="text-secondary font-number">{codeText(stock.symbol, stock.code)}</td>
      <td className="text-primary max-w-28 truncate">{stock.name}</td>
      <td className="num">{numberText(stock.price)}</td>
      <td className={cn("num font-medium", pctClass(stock.change_percent))}>
        {percentText(stock.change_percent)}
      </td>
      <td className="num">
        {stock.turnover_rate != null ? `${numberText(stock.turnover_rate)}%` : "—"}
      </td>
      <td className="num">{formatCompactNumber(stock.float_market_cap)}</td>
    </tr>
  )
}
