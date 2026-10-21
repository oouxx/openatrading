import { useState, useMemo } from "react"
import { Checkbox } from "@/components/ui/checkbox"
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
        <span className="text-caption font-number text-tertiary">加载晋级数据...</span>
      </div>
    )
  }

  if (error) {
    return (
      <div className="bg-surface border-hairline p-card-inner text-caption text-tertiary rounded-lg border text-center">
        {error}
      </div>
    )
  }

  if (ladder.length === 0 && firstBoard.length === 0) {
    return (
      <div className="bg-surface border-hairline p-card-inner text-caption text-tertiary rounded-lg border text-center">
        今日无涨停数据
      </div>
    )
  }

  return (
    <div>
      {/* 昨日数据缺失提示 */}
      {!hasYesterdayData && (
        <div className="bg-surface border-hairline p-card-inner text-caption text-tertiary mb-8 rounded-lg border text-center">
          未获取到昨日涨停列表，断板数据可能缺失
        </div>
      )}

      {/* ST 过滤 */}
      <div className="mb-6 flex items-center gap-4">
        <label className="text-secondary flex cursor-pointer items-center gap-2 select-none">
          <Checkbox checked={hideST} onCheckedChange={(checked) => setHideST(checked === true)} />
          <span className="text-caption">隐藏ST</span>
        </label>
        {hideST && stFilteredCount > 0 && (
          <span className="text-caption text-tertiary">已过滤 {stFilteredCount} 只ST</span>
        )}
      </div>

      {/* 连板晋级梯队 */}
      {ladder.length > 0 && (
        <div className="mb-12">
          <div className="mb-6 flex items-center gap-3">
            <h3 className="text-hero text-primary">连板晋级梯队</h3>
            <span className="text-caption text-tertiary">
              昨 {totalYesterday} 只 → 今 {totalToday} 只 · 综合晋级率
              <b className={overallRate >= 0.5 ? "text-up" : "text-tertiary"}>
                {" "}
                {(overallRate * 100).toFixed(1)}%
              </b>
            </span>
          </div>

          <div className="bg-surface border-hairline p-card-inner rounded-lg border">
            {ladder.map((row) => (
              <div key={row.yesterdayDays} className="mb-3 last:mb-0">
                <button
                  type="button"
                  aria-expanded={selectedBoard === row.yesterdayDays}
                  className="text-caption font-number flex w-full cursor-pointer items-center gap-3 text-left"
                  onClick={() =>
                    setSelectedBoard(selectedBoard === row.yesterdayDays ? null : row.yesterdayDays)
                  }
                >
                  <span className="text-caption text-secondary bg-hover min-w-14 rounded-sm px-2 py-0.5 text-center">
                    昨{row.yesterdayDays}板
                  </span>
                  <span className="text-tertiary min-w-10">{row.yesterdayCount}只</span>
                  <span className="text-tertiary">→</span>
                  <span className="text-caption min-w-14 rounded-sm px-2 py-0.5 text-center">
                    今{row.yesterdayDays + 1}板
                  </span>
                  <span className="text-primary min-w-10">{row.promotedCount}只</span>
                  <span className="text-caption text-tertiary">晋级率</span>
                  <b className={row.rate >= 0.5 ? "text-up" : "text-tertiary"}>
                    {(row.rate * 100).toFixed(1)}%
                  </b>
                  <div className="bg-hover h-2 min-w-15 flex-1 rounded-full">
                    <div
                      className={`h-full rounded-full transition-[width] duration-300 ${row.rate >= 0.5 ? "bg-up" : "bg-down"}`}
                      style={{ width: row.rate * 100 + "%" }}
                    />
                  </div>
                  <span className="text-caption text-tertiary">断板 {row.brokenCount}只</span>
                </button>

                {/* 展开详情表 */}
                {selectedBoard === row.yesterdayDays && (
                  <div className="mt-3 ml-4">
                    <table className="data-table">
                      <thead>
                        <tr>
                          <th>代码</th>
                          <th>名称</th>
                          <th>昨板→今板</th>
                          <th className="text-right">换手率</th>
                          <th className="text-right">流通市值</th>
                          <th>状态</th>
                        </tr>
                      </thead>
                      <tbody>
                        {row.details.map((s) => (
                          <tr key={s.symbol}>
                            <td className="text-brand font-number">
                              <MiniChartPreview symbol={s.symbol} name={s.name}>
                                <button
                                  type="button"
                                  className="cursor-pointer transition-colors hover:underline"
                                  onClick={() => handleOpenKline(s.symbol, s.name)}
                                >
                                  {s.symbol}
                                </button>
                              </MiniChartPreview>
                            </td>
                            <td className="text-primary font-medium">
                              <MiniChartPreview symbol={s.symbol} name={s.name}>
                                <span>{s.name}</span>
                              </MiniChartPreview>
                            </td>
                            <td className="font-number">
                              <span
                                className={s.status === "promoted" ? "text-up" : "text-tertiary"}
                              >
                                {s.yesterdayDays}板 →{" "}
                                {s.status === "promoted" ? s.todayDays + "板" : "断板"}
                              </span>
                            </td>
                            <td className="font-number text-right">
                              {s.turnover ? (s.turnover * 100).toFixed(2) + "%" : "-"}
                            </td>
                            <td className="font-number text-right">
                              {(s.capital / 1e8).toFixed(1)}亿
                            </td>
                            <td>
                              <span
                                className={`text-caption font-number rounded-sm px-2 py-0.5 ${
                                  s.status === "promoted"
                                    ? "bg-up/15 text-up"
                                    : "text-tertiary bg-hover"
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
            <span className="text-caption text-tertiary">共 {firstBoard.length} 只</span>
          </div>
          <div className="bg-surface border-hairline overflow-hidden rounded-lg border">
            <table className="data-table">
              <thead>
                <tr>
                  <th>代码</th>
                  <th>名称</th>
                  <th>现价</th>
                  <th className="text-right">涨跌幅</th>
                  <th className="text-right">换手率</th>
                  <th className="text-right">流通市值</th>
                  <th>板块</th>
                </tr>
              </thead>
              <tbody>
                {firstBoard.map((s) => (
                  <tr key={s.symbol}>
                    <td className="text-brand font-number">
                      <MiniChartPreview symbol={s.symbol} name={s.name}>
                        <button
                          type="button"
                          className="cursor-pointer transition-colors hover:underline"
                          onClick={() => handleOpenKline(s.symbol, s.name)}
                        >
                          {s.symbol}
                        </button>
                      </MiniChartPreview>
                    </td>
                    <td className="text-primary font-medium">
                      <MiniChartPreview symbol={s.symbol} name={s.name}>
                        <span>{s.name}</span>
                      </MiniChartPreview>
                    </td>
                    <td>
                      <span className="font-number text-tertiary">{s.price}</span>
                    </td>
                    <td className="text-up font-number text-right">
                      +{(s.changePercent * 100).toFixed(2)}%
                    </td>
                    <td className="font-number text-right">{(s.turnover * 100).toFixed(2)}%</td>
                    <td className="font-number text-right">{(s.capital / 1e8).toFixed(1)}亿</td>
                    <td className="text-caption text-tertiary">{s.reason || "-"}</td>
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
