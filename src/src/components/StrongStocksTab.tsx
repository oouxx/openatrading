import { useState } from "react"

import { DataEmpty, DataError, DataLoading } from "@/components/data-state"
import { useLimitUp } from "@/hooks/query/useLimitUp"
import { useSuperStock } from "@/hooks/query/useSuperStock"
import { cn } from "@/lib/utils"
import MiniChartPreview from "@/components/MiniChartPreview"
import { usePaneNav } from "@/components/paneNavContext"
import type { Stock } from "@/stores/data"
import { formatCompactNumber, formatRatioPct, pctClass } from "@/lib/format"

const POOLS = [
  { id: "limit-up", label: "涨停股" },
  { id: "super", label: "强势股" },
] as const

type Pool = (typeof POOLS)[number]["id"]

export default function StrongStocksTab() {
  const [pool, setPool] = useState<Pool>("limit-up")
  const limitUp = useLimitUp()
  const superStock = useSuperStock()
  const activeQuery = pool === "limit-up" ? limitUp : superStock
  const stocks = activeQuery.data ?? []
  const { openStock: openKline } = usePaneNav()

  return (
    <div className="@container space-y-3 p-4 @2xl:p-6">
      <div className="bg-hover border-hairline inline-flex rounded-md border p-0.5">
        {POOLS.map((p) => {
          const active = pool === p.id
          const query = p.id === "limit-up" ? limitUp : superStock
          const count = query.isPending ? "…" : query.error ? "—" : (query.data?.length ?? 0)

          return (
            <button
              key={p.id}
              type="button"
              aria-pressed={active}
              onClick={() => setPool(p.id)}
              className={cn(
                "text-caption focus-visible:ring-brand/40 flex h-7 items-center gap-1.5 rounded-sm px-2.5 transition-colors outline-none focus-visible:ring-2",
                active
                  ? "bg-surface text-primary shadow-card"
                  : "text-tertiary hover:text-secondary"
              )}
            >
              {p.label}
              <span className="font-number tabular-nums">{count}</span>
            </button>
          )
        })}
      </div>

      <div className="bg-surface border-hairline overflow-hidden rounded-lg border">
        <div className="overflow-x-auto">
          <table className="data-table">
            <thead>
              <tr>
                <th>个股</th>
                <th className="num">涨幅</th>
                <th className="num">连板</th>
                <th className="num hidden @2xl:table-cell">封板次数</th>
                <th className="num">换手率</th>
                <th className="num hidden @2xl:table-cell">市值</th>
                <th className="hidden @2xl:table-cell">原因</th>
              </tr>
            </thead>
            <tbody>
              {activeQuery.isPending ? (
                <tr>
                  <td colSpan={7} className="p-0">
                    <DataLoading
                      label={`加载${pool === "limit-up" ? "涨停股" : "强势股"}…`}
                      size="table"
                      rows={4}
                    />
                  </td>
                </tr>
              ) : activeQuery.error ? (
                <tr>
                  <td colSpan={7} className="p-0">
                    <DataError
                      title="股票池获取失败"
                      description={activeQuery.error.message}
                      size="table"
                      onRetry={() => void activeQuery.refetch()}
                    />
                  </td>
                </tr>
              ) : stocks.length === 0 ? (
                <tr>
                  <td colSpan={7} className="p-0">
                    <DataEmpty title="暂无数据" description="当前股票池为空" size="table" />
                  </td>
                </tr>
              ) : (
                stocks.map((s: Stock) => (
                  <tr
                    key={s.symbol}
                    className="cursor-pointer"
                    onClick={() => openKline(s.symbol, s.stock_chi_name)}
                  >
                    <td>
                      <div className="flex min-w-0 flex-col">
                        <MiniChartPreview symbol={s.symbol} name={s.stock_chi_name}>
                          <button
                            type="button"
                            className="hover:text-brand text-primary block max-w-full cursor-pointer truncate text-left transition-colors"
                            onClick={(e) => {
                              e.stopPropagation()
                              openKline(s.symbol, s.stock_chi_name)
                            }}
                          >
                            {s.stock_chi_name}
                          </button>
                        </MiniChartPreview>
                        <span className="text-tertiary text-micro font-number">
                          {s.symbol.replace(/\.[A-Z]+$/, "")}
                        </span>
                      </div>
                    </td>
                    <td className={cn("num", pctClass(s.change_percent))}>
                      {formatRatioPct(s.change_percent)}
                    </td>
                    <td className="num text-brand font-medium">
                      {s.limit_up_days > 1 ? s.limit_up_days + "连板" : "—"}
                    </td>
                    <td className="num hidden @2xl:table-cell">{s.break_limit_up_times || "—"}</td>
                    <td className="num">
                      {s.turnover_ratio != null ? (s.turnover_ratio * 100).toFixed(2) + "%" : "—"}
                    </td>
                    <td className="num text-secondary hidden @2xl:table-cell">
                      {formatCompactNumber(s.total_capital)}
                    </td>
                    <td className="text-secondary hidden max-w-48 truncate @2xl:table-cell">
                      {s.surge_reason?.stock_reason || "—"}
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  )
}
