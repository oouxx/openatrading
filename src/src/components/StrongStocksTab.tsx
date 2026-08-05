import { useLimitUp } from "@/hooks/query/useLimitUp"
import MiniChartPreview from "@/components/MiniChartPreview"
import { useRightPanelStore } from "@/stores/rightPanelStore"
import type { Stock } from "@/stores/data"

function formatCap(v: number) {
  if (v >= 1e12) return (v / 1e12).toFixed(1) + "万亿"
  if (v >= 1e8) return (v / 1e8).toFixed(1) + "亿"
  if (v >= 1e4) return (v / 1e4).toFixed(1) + "万"
  return String(v)
}

function formatPct(v: number) {
  return (v >= 0 ? "+" : "") + (v * 100).toFixed(2) + "%"
}

function pctClass(v: number) {
  return v >= 0 ? "text-up" : "text-down"
}

export default function StrongStocksTab() {
  const { data: limitUpStocks = [] } = useLimitUp()
  const openKline = useRightPanelStore((s) => s.openKline)

  return (
    <div className="bg-surface border-hairline rounded-lg overflow-hidden border">
      <div className="border-hairline flex items-center justify-between border-b px-4 py-3">
        <span className="text-caption text-secondary font-mono tracking-wider uppercase">
          涨停股 · {limitUpStocks.length}
        </span>
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
            {limitUpStocks.map((s: Stock) => (
              <tr
                key={s.symbol}
                className="cursor-pointer"
                onClick={() => openKline(s.symbol, s.stock_chi_name)}
              >
                <td className="text-secondary">{s.symbol.replace(/\.[A-Z]+$/, "")}</td>
                <td>
                  <MiniChartPreview symbol={s.symbol} name={s.stock_chi_name}>
                    <span className="hover:text-brand text-primary transition-colors">
                      {s.stock_chi_name}
                    </span>
                  </MiniChartPreview>
                </td>
                <td className={pctClass(s.change_percent)}>{formatPct(s.change_percent)}</td>
                <td className="text-brand font-semibold">
                  {s.limit_up_days > 1 ? s.limit_up_days + "连板" : "-"}
                </td>
                <td>{s.break_limit_up_times || "-"}</td>
                <td>{s.turnover_ratio?.toFixed(2) + "%" || "-"}</td>
                <td>{formatCap(s.total_capital)}</td>
                <td className="text-secondary max-w-[200px] truncate text-caption">
                  {s.surge_reason?.stock_reason || "-"}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  )
}
