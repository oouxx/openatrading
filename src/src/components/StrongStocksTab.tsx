import { useState } from "react"

import { Button } from "@/components/ui/button"
import { useLimitUp } from "@/hooks/query/useLimitUp"
import { useSuperStock } from "@/hooks/query/useSuperStock"
import { cn } from "@/lib/utils"
import MiniChartPreview from "@/components/MiniChartPreview"
import { useRightPanelStore } from "@/stores/rightPanelStore"
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
  const openKline = useRightPanelStore((s) => s.openKline)

  return (
    <div className="bg-surface border-hairline overflow-hidden rounded-lg border">
      <div className="border-hairline flex items-center gap-1 border-b px-3 py-1.5">
        {POOLS.map((p) => {
          const query = p.id === "limit-up" ? limitUp : superStock
          const count = query.isPending ? "…" : query.error ? "—" : (query.data?.length ?? 0)
          const active = pool === p.id

          return (
            <Button
              key={p.id}
              type="button"
              variant="ghost"
              size="sm"
              aria-pressed={active}
              onClick={() => setPool(p.id)}
              className={cn(
                "text-caption rounded-full px-3",
                active
                  ? "bg-brand/20 text-brand hover:bg-brand/20 hover:text-brand"
                  : "text-secondary hover:bg-hover hover:text-primary",
              )}
            >
              {p.label}
              <span className="font-number tabular-nums">{count}</span>
            </Button>
          )
        })}
      </div>
      <div className="overflow-x-auto">
        <table className="data-table">
          <thead>
            <tr>
              <th>代码</th>
              <th>名称</th>
              <th>涨幅</th>
              <th>连板</th>
              <th>封板次数</th>
              <th>换手率</th>
              <th>市值</th>
              <th>原因</th>
            </tr>
          </thead>
          <tbody>
            {activeQuery.isPending ? (
              <tr>
                <td colSpan={8} className="text-caption text-tertiary py-10 text-center">
                  加载中…
                </td>
              </tr>
            ) : activeQuery.error ? (
              <tr>
                <td colSpan={8} className="text-caption text-danger py-10 text-center">
                  加载失败：{activeQuery.error.message}
                </td>
              </tr>
            ) : stocks.length === 0 ? (
              <tr>
                <td colSpan={8} className="text-caption text-tertiary py-10 text-center">
                  暂无数据
                </td>
              </tr>
            ) : (
              stocks.map((s: Stock) => (
                <tr
                  key={s.symbol}
                  className="cursor-pointer"
                  onClick={() => openKline(s.symbol, s.stock_chi_name)}
                >
                  <td className="text-secondary">{s.symbol.replace(/\.[A-Z]+$/, "")}</td>
                  <td>
                    <MiniChartPreview symbol={s.symbol} name={s.stock_chi_name}>
                      <button
                        type="button"
                        className="hover:text-brand text-primary cursor-pointer transition-colors"
                        onClick={(e) => {
                          e.stopPropagation()
                          openKline(s.symbol, s.stock_chi_name)
                        }}
                      >
                        {s.stock_chi_name}
                      </button>
                    </MiniChartPreview>
                  </td>
                  <td className={pctClass(s.change_percent)}>{formatRatioPct(s.change_percent)}</td>
                  <td className="text-brand font-semibold">
                    {s.limit_up_days > 1 ? s.limit_up_days + "连板" : "-"}
                  </td>
                  <td>{s.break_limit_up_times || "-"}</td>
                  <td>
                    {s.turnover_ratio != null ? (s.turnover_ratio * 100).toFixed(2) + "%" : "-"}
                  </td>
                  <td>{formatCompactNumber(s.total_capital)}</td>
                  <td className="text-secondary text-caption max-w-[200px] truncate">
                    {s.surge_reason?.stock_reason || "-"}
                  </td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>
    </div>
  )
}
