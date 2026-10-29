import { useEffect, useMemo, useRef } from "react"
import { RefreshCw } from "lucide-react"

import { Button } from "@/components/ui/button"
import { Skeleton } from "@/components/ui/skeleton"
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
      <div className="flex flex-wrap items-center gap-1">
        {ORDERS.map((item) => (
          <Button
            key={item.id}
            type="button"
            variant="ghost"
            size="sm"
            aria-pressed={order === item.id}
            onClick={() => setOrder(item.id)}
            className={cn(
              "text-caption rounded-full px-3",
              order === item.id
                ? "bg-brand/20 text-brand hover:bg-brand/20 hover:text-brand"
                : "text-secondary hover:bg-hover hover:text-primary"
            )}
          >
            {item.label}
          </Button>
        ))}
        <Button
          type="button"
          variant="ghost"
          size="sm"
          aria-pressed={excludeSt}
          onClick={() => setExcludeSt(!excludeSt)}
          className={cn(
            "text-caption ml-auto rounded-full px-3",
            excludeSt
              ? "bg-brand/20 text-brand hover:bg-brand/20 hover:text-brand"
              : "text-secondary hover:bg-hover hover:text-primary"
          )}
        >
          排除 ST
        </Button>
      </div>

      <div className="flex flex-wrap items-center gap-1">
        {MARKETS.map((item) => (
          <Button
            key={item.id}
            type="button"
            variant="ghost"
            size="sm"
            aria-pressed={market === item.id}
            onClick={() => setMarket(item.id)}
            className={cn(
              "text-caption rounded-full px-2.5",
              market === item.id
                ? "bg-hover text-primary"
                : "text-tertiary hover:bg-hover hover:text-secondary"
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
          {query.data?.pages.some((page) => page.stale) && (
            <span className="text-warning">上游延迟，展示最近数据</span>
          )}
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

      {query.isPending ? (
        <Skeleton className="h-72 w-full" />
      ) : query.error ? (
        <div className="bg-surface border-hairline rounded-lg border p-5">
          <p className="text-caption text-danger">加载失败：{query.error.message}</p>
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="mt-3"
            onClick={() => void query.refetch()}
          >
            重试
          </Button>
        </div>
      ) : stocks.length === 0 ? (
        <div className="text-caption text-tertiary py-12 text-center">暂无股票</div>
      ) : (
        <div className="bg-surface border-hairline overflow-hidden rounded-lg border">
          <table className="data-table">
            <thead>
              <tr>
                <th>#</th>
                <th>代码</th>
                <th>名称</th>
                <th className="text-right">最新价</th>
                <th className="text-right">涨跌幅</th>
                <th className="text-right">换手率</th>
                <th className="text-right">流通市值</th>
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
          <div ref={loadMoreRef} className="text-caption text-tertiary py-3 text-center">
            {query.isFetchingNextPage ? "加载更多…" : query.hasNextPage ? "滚动加载更多" : "已加载全部"}
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
      <td className="font-number text-right">{numberText(stock.price)}</td>
      <td className={cn("font-number text-right font-medium", pctClass(stock.change_percent))}>
        {percentText(stock.change_percent)}
      </td>
      <td className="font-number text-right">
        {stock.turnover_rate != null ? `${numberText(stock.turnover_rate)}%` : "—"}
      </td>
      <td className="font-number text-right">{formatCompactNumber(stock.float_market_cap)}</td>
    </tr>
  )
}
