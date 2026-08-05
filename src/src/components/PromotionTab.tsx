import { useState, useMemo } from "react"
import { type Stock } from "@/stores/data"
import { useLimitUp } from "@/hooks/query/useLimitUp"
import { useYesterdayLimitUp } from "@/hooks/query/useYesterdayLimitUp"
import MiniChartPreview from "@/components/MiniChartPreview"
import { useRightPanelStore } from "@/stores/rightPanelStore"

interface PromotionItem {
  symbol: string
  name: string
  price: number
  yesterdayDays: number
  todayDays: number
  status: "promoted" | "broken" | "first"
  turnover: number
  capital: number
  changePercent: number
  reason: string
}

interface LadderRow {
  yesterdayDays: number
  yesterdayCount: number
  promotedCount: number
  brokenCount: number
  rate: number
  details: PromotionItem[]
}

interface PromotionTabProps {
  stocks: Stock[]
  onOpenKline: (symbol: string, name: string) => void
}

function isST(name: string) {
  return name.includes("ST") || name.includes("*")
}

function mergeData(todayList: Stock[], yesterdayList: Stock[]): PromotionItem[] {
  const yesterdayMap = new Map<string, Stock>()
  for (const s of yesterdayList) yesterdayMap.set(s.symbol, s)

  const result: PromotionItem[] = []

  for (const s of todayList) {
    const prev = yesterdayMap.get(s.symbol)
    if (prev) yesterdayMap.delete(s.symbol)
    const plates = s.surge_reason?.related_plates?.map((p) => p.plate_name).join(", ") || ""
    if (prev) {
      const yestDays = s.yesterday_limit_up_days || prev.yesterday_limit_up_days || 0
      result.push({
        symbol: s.symbol,
        price: s.price,
        name: s.stock_chi_name,
        yesterdayDays: yestDays,
        todayDays: s.limit_up_days,
        status: "promoted",
        turnover: s.turnover_ratio,
        capital: s.total_capital,
        changePercent: s.change_percent,
        reason: plates,
      })
    } else {
      result.push({
        symbol: s.symbol,
        name: s.stock_chi_name,
        price: s.price,
        yesterdayDays: 0,
        todayDays: s.limit_up_days,
        status: "first",
        turnover: s.turnover_ratio,
        capital: s.total_capital,
        changePercent: s.change_percent,
        reason: plates,
      })
    }
  }

  for (const s of yesterdayMap.values()) {
    result.push({
      symbol: s.symbol,
      price: s.price,
      name: s.stock_chi_name,
      yesterdayDays: s.yesterday_limit_up_days || 0,
      todayDays: 0,
      status: "broken",
      turnover: s.turnover_ratio || 0,
      capital: s.total_capital || 0,
      changePercent: 0,
      reason: "",
    })
  }

  return result
}

export default function PromotionTab({ stocks: propStocks, onOpenKline }: PromotionTabProps) {
  const [hideST, setHideST] = useState(true)
  const [selectedBoard, setSelectedBoard] = useState<number | null>(null)
  const { data: yesterdayList = [] } = useYesterdayLimitUp()
  const { data: fallbackStocks = [] } = useLimitUp()
  const panelStore = useRightPanelStore()

  const stocks = propStocks.length ? propStocks : fallbackStocks
  const handleOpenKline = onOpenKline ?? panelStore.openKline

  const isLoading = !stocks.length
  const error = isLoading ? "" : stocks.length === 0 ? "今日无涨停数据" : ""
  const hasYesterdayData = yesterdayList.length > 0

  const allItems = useMemo(() => {
    if (!stocks.length) return []
    if (!hasYesterdayData) {
      return stocks.map((s) => {
        const yestDays = s.yesterday_limit_up_days || 0
        return {
          symbol: s.symbol,
          name: s.stock_chi_name,
          price: s.price,
          yesterdayDays: yestDays,
          todayDays: s.limit_up_days,
          status: (yestDays > 0 ? "promoted" : "first") as "promoted" | "first",
          turnover: s.turnover_ratio,
          capital: s.total_capital,
          changePercent: s.change_percent,
          reason: s.surge_reason?.related_plates?.map((p) => p.plate_name).join(", ") || "",
        }
      })
    }
    return mergeData(stocks, yesterdayList)
  }, [stocks, yesterdayList, hasYesterdayData])

  const nonSTItems = useMemo(
    () => (hideST ? allItems.filter((s) => !isST(s.name)) : allItems),
    [allItems, hideST]
  )

  const stFilteredCount = useMemo(() => allItems.filter((s) => isST(s.name)).length, [allItems])

  const firstBoard = useMemo(
    () => nonSTItems.filter((s) => s.status === "first").sort((a, b) => b.capital - a.capital),
    [nonSTItems]
  )

  const promoted = useMemo(() => nonSTItems.filter((s) => s.status === "promoted"), [nonSTItems])
  const broken = useMemo(() => nonSTItems.filter((s) => s.status === "broken"), [nonSTItems])

  const totalYesterday = promoted.length + broken.length
  const totalToday = promoted.length + firstBoard.length
  const overallRate = totalYesterday > 0 ? promoted.length / totalYesterday : 0

  const ladder = useMemo((): LadderRow[] => {
    const groups = new Map<number, { promoted: PromotionItem[]; broken: PromotionItem[] }>()
    for (const s of promoted) {
      const g = groups.get(s.yesterdayDays) || { promoted: [], broken: [] }
      g.promoted.push(s)
      groups.set(s.yesterdayDays, g)
    }
    for (const s of broken) {
      const g = groups.get(s.yesterdayDays) || { promoted: [], broken: [] }
      g.broken.push(s)
      groups.set(s.yesterdayDays, g)
    }

    const rows: LadderRow[] = []
    for (const [days, g] of groups) {
      const total = g.promoted.length + g.broken.length
      rows.push({
        yesterdayDays: days,
        yesterdayCount: total,
        promotedCount: g.promoted.length,
        brokenCount: g.broken.length,
        rate: total > 0 ? g.promoted.length / total : 0,
        details: [...g.promoted, ...g.broken].sort((a, b) => {
          if (a.status === "promoted" && b.status !== "promoted") return -1
          if (a.status !== "promoted" && b.status === "promoted") return 1
          return b.capital - a.capital
        }),
      })
    }
    return rows.sort((a, b) => b.yesterdayDays - a.yesterdayDays)
  }, [promoted, broken])

  // ── Loading / Error / Empty ─────────────────────────

  if (isLoading) {
    return (
      <div className="flex items-center justify-center gap-3 py-16">
        <span className="bg-brand size-2 animate-pulse rounded-full" />
        <span className="text-caption font-mono text-primary/40">加载晋级数据...</span>
      </div>
    )
  }

  if (error) {
    return (
      <div className="bg-surface-dark border-hairline-dark p-card-inner rounded-lg text-caption border text-center text-primary/40">
        {error}
      </div>
    )
  }

  if (ladder.length === 0 && firstBoard.length === 0) {
    return (
      <div className="bg-surface-dark border-hairline-dark p-card-inner rounded-lg text-caption border text-center text-primary/40">
        今日无涨停数据
      </div>
    )
  }

  return (
    <div>
      {/* 昨日数据缺失提示 */}
      {!hasYesterdayData && (
        <div className="bg-surface-dark border-hairline-dark p-card-inner rounded-lg text-caption mb-8 border text-center text-primary/40">
          未获取到昨日涨停列表，断板数据可能缺失
        </div>
      )}

      {/* ST 过滤 */}
      <div className="mb-6 flex items-center gap-4">
        <label className="flex cursor-pointer items-center gap-2 text-primary/60 select-none">
          <input
            type="checkbox"
            checked={hideST}
            onChange={(e) => setHideST(e.target.checked)}
            className="cursor-pointer"
          />
          <span className="text-caption">隐藏ST</span>
        </label>
        {hideST && stFilteredCount > 0 && (
          <span className="text-caption text-primary/40">已过滤 {stFilteredCount} 只ST</span>
        )}
      </div>

      {/* 连板晋级梯队 */}
      {ladder.length > 0 && (
        <div className="mb-12">
          <div className="mb-6 flex items-center gap-3">
            <h3 className="text-hero text-primary">连板晋级梯队</h3>
            <span className="text-caption text-primary/40">
              昨 {totalYesterday} 只 → 今 {totalToday} 只 · 综合晋级率
              <b className={overallRate >= 0.5 ? "text-up" : "text-primary/40"}>
                {" "}
                {(overallRate * 100).toFixed(1)}%
              </b>
            </span>
          </div>

          <div className="bg-surface-dark border-hairline-dark p-card-inner rounded-lg border">
            {ladder.map((row) => (
              <div key={row.yesterdayDays} className="mb-3 last:mb-0">
                <div
                  className="text-caption flex cursor-pointer items-center gap-3 font-mono"
                  onClick={() =>
                    setSelectedBoard(selectedBoard === row.yesterdayDays ? null : row.yesterdayDays)
                  }
                >
                  <span
                    className="rounded-sm text-caption bg-white/10 px-2 py-0.5 text-primary/60"
                    style={{ minWidth: 56, textAlign: "center" }}
                  >
                    昨{row.yesterdayDays}板
                  </span>
                  <span className="text-primary/40" style={{ minWidth: 40 }}>
                    {row.yesterdayCount}只
                  </span>
                  <span className="text-primary/40">→</span>
                  <span
                    className="rounded-sm text-caption px-2 py-0.5"
                    style={{ minWidth: 56, textAlign: "center" }}
                  >
                    今{row.yesterdayDays + 1}板
                  </span>
                  <span className="text-primary" style={{ minWidth: 40 }}>
                    {row.promotedCount}只
                  </span>
                  <span className="text-caption text-primary/40">晋级率</span>
                  <b className={row.rate >= 0.5 ? "text-up" : "text-primary/40"}>
                    {(row.rate * 100).toFixed(1)}%
                  </b>
                  <div className="h-2 flex-1 rounded-full bg-white/10" style={{ minWidth: 60 }}>
                    <div
                      className={`h-full rounded-full transition-all ${row.rate >= 0.5 ? "bg-up" : "bg-down"}`}
                      style={{ width: row.rate * 100 + "%" }}
                    />
                  </div>
                  <span className="text-caption text-primary/40">断板 {row.brokenCount}只</span>
                </div>

                {/* 展开详情表 */}
                {selectedBoard === row.yesterdayDays && (
                  <div className="mt-3 ml-4">
                    <table className="data-table">
                      <thead>
                        <tr>
                          <th>代码</th>
                          <th>名称</th>
                          <th>昨板→今板</th>
                          <th style={{ textAlign: "right" }}>换手率</th>
                          <th style={{ textAlign: "right" }}>流通市值</th>
                          <th>状态</th>
                        </tr>
                      </thead>
                      <tbody>
                        {row.details.map((s) => (
                          <tr key={s.symbol}>
                            <td
                              className="text-brand cursor-pointer font-mono hover:underline"
                              onClick={() => handleOpenKline(s.symbol, s.name)}
                            >
                              <MiniChartPreview symbol={s.symbol} name={s.name}>
                                {s.symbol}
                              </MiniChartPreview>
                            </td>
                            <td className="font-medium text-primary">
                              <MiniChartPreview symbol={s.symbol} name={s.name}>
                                {s.name}
                              </MiniChartPreview>
                            </td>
                            <td className="font-mono">
                              <span
                                className={s.status === "promoted" ? "text-up" : "text-primary/40"}
                              >
                                {s.yesterdayDays}板 →{" "}
                                {s.status === "promoted" ? s.todayDays + "板" : "断板"}
                              </span>
                            </td>
                            <td className="text-right font-mono">
                              {s.turnover ? (s.turnover * 100).toFixed(2) + "%" : "-"}
                            </td>
                            <td className="text-right font-mono">
                              {(s.capital / 1e8).toFixed(1)}亿
                            </td>
                            <td>
                              <span
                                className={`text-caption rounded-sm px-2 py-0.5 font-mono ${
                                  s.status === "promoted"
                                    ? "bg-up/15 text-up"
                                    : "bg-white/10 text-primary/40"
                                }`}
                              >
                                {s.status === "promoted" ? "晋级" : "断板"}
                              </span>
                            </td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                )}
              </div>
            ))}
          </div>
        </div>
      )}

      {/* 首板新进 */}
      {firstBoard.length > 0 && (
        <div>
          <div className="mb-6 flex items-center gap-3">
            <h3 className="text-hero text-primary">首板新进</h3>
            <span className="text-caption text-primary/40">共 {firstBoard.length} 只</span>
          </div>
          <div className="bg-surface-dark border-hairline-dark rounded-lg overflow-hidden border">
            <table className="data-table">
              <thead>
                <tr>
                  <th>代码</th>
                  <th>名称</th>
                  <th>现价</th>
                  <th style={{ textAlign: "right" }}>涨跌幅</th>
                  <th style={{ textAlign: "right" }}>换手率</th>
                  <th style={{ textAlign: "right" }}>流通市值</th>
                  <th>板块</th>
                </tr>
              </thead>
              <tbody>
                {firstBoard.map((s) => (
                  <tr key={s.symbol}>
                    <td
                      className="text-brand cursor-pointer font-mono hover:underline"
                      onClick={() => handleOpenKline(s.symbol, s.name)}
                    >
                      <MiniChartPreview symbol={s.symbol} name={s.name}>
                        {s.symbol}
                      </MiniChartPreview>
                    </td>
                    <td className="font-medium text-primary">
                      <MiniChartPreview symbol={s.symbol} name={s.name}>
                        {s.name}
                      </MiniChartPreview>
                    </td>
                    <td>
                      <span className="font-mono text-primary/40">{s.price}</span>
                    </td>
                    <td className="text-up text-right font-mono">
                      +{(s.changePercent * 100).toFixed(2)}%
                    </td>
                    <td className="text-right font-mono">{(s.turnover * 100).toFixed(2)}%</td>
                    <td className="text-right font-mono">{(s.capital / 1e8).toFixed(1)}亿</td>
                    <td className="text-caption text-primary/40">{s.reason || "-"}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}
    </div>
  )
}
