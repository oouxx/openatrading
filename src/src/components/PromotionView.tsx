import { useMemo, useState } from "react"
import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import MiniChartPreview from "@/components/MiniChartPreview"
import { useLimitUp } from "@/hooks/query/useLimitUp"
import { useYesterdayLimitUp } from "@/hooks/query/useYesterdayLimitUp"
import { formatCompactNumber } from "@/lib/format"
import { useRightPanelStore } from "@/stores/rightPanelStore"
import { type Stock } from "@/stores/data"

type PromotionStatus = "promoted" | "broken" | "first"
type StatusFilter = "all" | PromotionStatus
type BoardFilter = "all" | 1 | 2 | 3 | 4

interface PromotionItem {
  symbol: string
  name: string
  price: number
  yesterdayDays: number
  todayDays: number
  status: PromotionStatus
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

const STATUS_FILTERS: { id: StatusFilter; label: string }[] = [
  { id: "all", label: "全部" },
  { id: "promoted", label: "晋级" },
  { id: "broken", label: "断板" },
  { id: "first", label: "新进" },
]

const BOARD_FILTERS: { id: BoardFilter; label: string }[] = [
  { id: "all", label: "全部" },
  { id: 1, label: "1板" },
  { id: 2, label: "2板" },
  { id: 3, label: "3板" },
  { id: 4, label: "4板+" },
]

function isST(name: string) {
  return name.includes("ST") || name.includes("*")
}

function mergeData(todayList: Stock[], yesterdayList: Stock[]): PromotionItem[] {
  const yesterdayMap = new Map<string, Stock>()
  for (const stock of yesterdayList) yesterdayMap.set(stock.symbol, stock)

  const result: PromotionItem[] = []

  for (const stock of todayList) {
    const previous = yesterdayMap.get(stock.symbol)
    if (previous) yesterdayMap.delete(stock.symbol)

    const reason = stock.surge_reason?.related_plates?.map((plate) => plate.plate_name).join(", ")
    const base = {
      symbol: stock.symbol,
      name: stock.stock_chi_name,
      price: stock.price,
      todayDays: stock.limit_up_days,
      turnover: stock.turnover_ratio,
      capital: stock.total_capital,
      changePercent: stock.change_percent,
      reason: reason || "",
    }

    if (previous) {
      result.push({
        ...base,
        yesterdayDays: stock.yesterday_limit_up_days || previous.yesterday_limit_up_days || 0,
        status: "promoted",
      })
    } else {
      result.push({
        ...base,
        yesterdayDays: 0,
        status: "first",
      })
    }
  }

  for (const stock of yesterdayMap.values()) {
    result.push({
      symbol: stock.symbol,
      name: stock.stock_chi_name,
      price: stock.price,
      yesterdayDays: stock.yesterday_limit_up_days || 0,
      todayDays: 0,
      status: "broken",
      turnover: stock.turnover_ratio || 0,
      capital: stock.total_capital || 0,
      changePercent: 0,
      reason: "",
    })
  }

  return result
}

function buildLadder(items: PromotionItem[]): LadderRow[] {
  const groups = new Map<number, { promoted: PromotionItem[]; broken: PromotionItem[] }>()

  for (const item of items) {
    if (item.status !== "promoted" && item.status !== "broken") continue
    const group = groups.get(item.yesterdayDays) || { promoted: [], broken: [] }
    if (item.status === "promoted") group.promoted.push(item)
    else group.broken.push(item)
    groups.set(item.yesterdayDays, group)
  }

  return [...groups.entries()]
    .map(([days, group]) => {
      const total = group.promoted.length + group.broken.length
      return {
        yesterdayDays: days,
        yesterdayCount: total,
        promotedCount: group.promoted.length,
        brokenCount: group.broken.length,
        rate: total > 0 ? group.promoted.length / total : 0,
        details: [...group.promoted, ...group.broken].sort((a, b) => {
          if (a.status === "promoted" && b.status !== "promoted") return -1
          if (a.status !== "promoted" && b.status === "promoted") return 1
          return b.capital - a.capital
        }),
      }
    })
    .sort((a, b) => b.yesterdayDays - a.yesterdayDays)
}

function formatRate(rate: number | null): string {
  return rate == null ? "—" : `${(rate * 100).toFixed(1)}%`
}

function rateTone(rate: number | null): string | undefined {
  if (rate == null) return undefined
  return rate >= 0.5 ? "text-up" : "text-warning"
}

function rateClass(rate: number | null): string {
  return rateTone(rate) ?? "text-tertiary"
}

function formatTurnover(value: number): string {
  return value > 0 ? `${(value * 100).toFixed(2)}%` : "-"
}

function filterChipClass(active: boolean): string {
  return `text-caption h-7 px-2.5 ${
    active
      ? "border-brand bg-brand/10 text-brand hover:bg-brand/15 hover:text-brand"
      : "border-hairline text-secondary hover:text-primary"
  }`
}

function StockLink({
  item,
  kind,
  onOpenKline,
}: {
  item: PromotionItem
  kind: "code" | "name"
  onOpenKline: (symbol: string, name: string) => void
}) {
  return (
    <MiniChartPreview symbol={item.symbol} name={item.name}>
      <Button
        type="button"
        variant="link"
        size="xs"
        className={
          kind === "code"
            ? "text-brand font-number text-caption h-auto p-0"
            : "text-primary text-body h-auto p-0 font-sans font-medium"
        }
        onClick={() => onOpenKline(item.symbol, item.name)}
      >
        {kind === "code" ? item.symbol : item.name}
      </Button>
    </MiniChartPreview>
  )
}

export default function PromotionView() {
  const [hideST, setHideST] = useState(true)
  const [statusFilter, setStatusFilter] = useState<StatusFilter>("all")
  const [boardFilter, setBoardFilter] = useState<BoardFilter>("all")
  const [selectedBoard, setSelectedBoard] = useState<number | null>(null)

  const {
    data: todayList = [],
    isLoading: todayLoading,
    error: todayError,
    refetch: refetchToday,
  } = useLimitUp()
  const {
    data: yesterdayList = [],
    isLoading: yesterdayLoading,
    error: yesterdayError,
    refetch: refetchYesterday,
  } = useYesterdayLimitUp()
  const openKline = useRightPanelStore((state) => state.openKline)

  const hasYesterdayData = yesterdayList.length > 0
  const isLoading = todayLoading || yesterdayLoading

  const allItems = useMemo(() => {
    if (!todayList.length) return []
    if (!hasYesterdayData) {
      return todayList.map((stock): PromotionItem => {
        const yesterdayDays = stock.yesterday_limit_up_days || 0
        return {
          symbol: stock.symbol,
          name: stock.stock_chi_name,
          price: stock.price,
          yesterdayDays,
          todayDays: stock.limit_up_days,
          status: yesterdayDays > 0 ? "promoted" : "first",
          turnover: stock.turnover_ratio,
          capital: stock.total_capital,
          changePercent: stock.change_percent,
          reason:
            stock.surge_reason?.related_plates?.map((plate) => plate.plate_name).join(", ") || "",
        }
      })
    }
    return mergeData(todayList, yesterdayList)
  }, [todayList, yesterdayList, hasYesterdayData])

  const nonSTItems = useMemo(
    () => (hideST ? allItems.filter((item) => !isST(item.name)) : allItems),
    [allItems, hideST]
  )
  const stFilteredCount = useMemo(
    () => allItems.filter((item) => isST(item.name)).length,
    [allItems]
  )
  const statusFilteredItems = useMemo(
    () =>
      statusFilter === "all"
        ? nonSTItems
        : nonSTItems.filter((item) => item.status === statusFilter),
    [nonSTItems, statusFilter]
  )
  const ladderItems = useMemo(() => {
    const items = statusFilteredItems.filter((item) => item.status !== "first")
    if (boardFilter === "all") return items
    return items.filter((item) =>
      boardFilter === 4 ? item.yesterdayDays >= 4 : item.yesterdayDays === boardFilter
    )
  }, [statusFilteredItems, boardFilter])

  const globalStats = useMemo(() => {
    const promoted = nonSTItems.filter((item) => item.status === "promoted")
    const broken = nonSTItems.filter((item) => item.status === "broken")
    const first = nonSTItems.filter((item) => item.status === "first")
    const ladder = buildLadder(nonSTItems)
    const yesterdayTotal = promoted.length + broken.length
    const l3Rows = ladder.filter((row) => row.yesterdayDays >= 3)
    const l3Promoted = l3Rows.reduce((sum, row) => sum + row.promotedCount, 0)
    const l3Total = l3Rows.reduce((sum, row) => sum + row.yesterdayCount, 0)

    return {
      promoted: promoted.length,
      broken: broken.length,
      first: first.length,
      todayTotal: promoted.length + first.length,
      yesterdayTotal,
      overallRate: yesterdayTotal > 0 ? promoted.length / yesterdayTotal : null,
      l1: ladder.find((row) => row.yesterdayDays === 1),
      l2: ladder.find((row) => row.yesterdayDays === 2),
      l3Rate: l3Total > 0 ? l3Promoted / l3Total : null,
      l3Promoted,
      l3Total,
    }
  }, [nonSTItems])

  const promoted = useMemo(
    () => ladderItems.filter((item) => item.status === "promoted"),
    [ladderItems]
  )
  const broken = useMemo(
    () => ladderItems.filter((item) => item.status === "broken"),
    [ladderItems]
  )
  const firstBoard = useMemo(
    () =>
      statusFilteredItems
        .filter((item) => item.status === "first")
        .sort((a, b) => b.capital - a.capital),
    [statusFilteredItems]
  )
  const ladder = useMemo(() => buildLadder(ladderItems), [ladderItems])
  const visibleCount = ladderItems.length + firstBoard.length

  const filteredTodayTotal = promoted.length
  const filteredYesterdayTotal = promoted.length + broken.length
  const filteredRate = filteredYesterdayTotal > 0 ? promoted.length / filteredYesterdayTotal : null

  const handleStatusFilter = (next: StatusFilter) => {
    setStatusFilter(next)
    if (next === "first") setBoardFilter("all")
    setSelectedBoard(null)
  }

  const handleBoardFilter = (next: BoardFilter) => {
    setBoardFilter(next)
    if (next !== "all" && statusFilter === "first") setStatusFilter("all")
    setSelectedBoard(null)
  }

  const resetFilters = () => {
    setStatusFilter("all")
    setBoardFilter("all")
    setSelectedBoard(null)
  }

  const retry = () => {
    void refetchToday()
    void refetchYesterday()
  }

  if (isLoading) {
    return (
      <div className="flex items-center justify-center gap-3 py-16">
        <span className="bg-brand size-2 animate-pulse rounded-full" />
        <span className="text-caption text-tertiary font-number">加载晋级分析数据...</span>
      </div>
    )
  }

  if (todayError && allItems.length === 0) {
    return (
      <div className="p-6">
        <div className="bg-surface border-hairline p-card-inner text-caption text-tertiary rounded-lg border text-center">
          <p className="text-danger mb-3">{todayError.message}</p>
          <Button variant="link" size="xs" className="h-auto p-0" onClick={retry}>
            重试
          </Button>
        </div>
      </div>
    )
  }

  if (!allItems.length) {
    return (
      <div className="p-6">
        <div className="bg-surface border-hairline p-card-inner text-caption text-tertiary rounded-lg border text-center">
          今日暂无涨停数据
        </div>
      </div>
    )
  }

  const statCards = [
    {
      label: "今日涨停",
      value: String(globalStats.todayTotal),
      sub: `晋级 ${globalStats.promoted} + 新进 ${globalStats.first}`,
      hl: true,
    },
    {
      label: "1→2 晋级率",
      value: formatRate(globalStats.l1?.rate ?? null),
      sub: `${globalStats.l1?.promotedCount ?? 0}/${globalStats.l1?.yesterdayCount ?? 0}`,
      tone: rateTone(globalStats.l1?.rate ?? null),
    },
    {
      label: "2→3 晋级率",
      value: formatRate(globalStats.l2?.rate ?? null),
      sub: `${globalStats.l2?.promotedCount ?? 0}/${globalStats.l2?.yesterdayCount ?? 0}`,
      tone: rateTone(globalStats.l2?.rate ?? null),
    },
    {
      label: "3→4+ 晋级率",
      value: formatRate(globalStats.l3Rate),
      sub: `${globalStats.l3Promoted}/${globalStats.l3Total}`,
      tone: rateTone(globalStats.l3Rate),
    },
    {
      label: "整体晋级率",
      value: formatRate(globalStats.overallRate),
      sub: `${globalStats.promoted}/${globalStats.yesterdayTotal}`,
      tone: rateTone(globalStats.overallRate),
    },
    {
      label: "断板 / 新进",
      value: `${globalStats.broken} / ${globalStats.first}`,
      sub:
        globalStats.yesterdayTotal > 0
          ? `断板率 ${((globalStats.broken / globalStats.yesterdayTotal) * 100).toFixed(1)}%`
          : "断板率 —",
    },
  ]

  return (
    <div className="space-y-4 p-6">
      <div className="@container">
        {/* ST 是否计入由下方「隐藏ST」勾选框表达，标题不再复述 */}
        <div className="text-caption text-tertiary mb-3">全市场晋级概览</div>
        <div className="grid grid-cols-2 gap-3 @sm:grid-cols-3">
          {statCards.map((card) => (
            <div
              key={card.label}
              className={`bg-surface p-card-inner rounded-lg border text-center ${card.hl ? "border-brand bg-brand/5" : "border-hairline"}`}
            >
              <div className="text-caption text-secondary">{card.label}</div>
              <div className={`font-number text-h1 mt-1 ${card.tone ?? "text-primary"}`}>
                {card.value}
              </div>
              <div className="text-caption text-tertiary mt-1">{card.sub}</div>
            </div>
          ))}
        </div>
      </div>

      {todayError && (
        <div className="bg-surface border-hairline p-card-inner text-caption text-danger rounded-lg border text-center">
          今日涨停数据刷新失败：{todayError.message}
          <Button
            variant="link"
            size="xs"
            className="ml-1 h-auto p-0 align-baseline"
            onClick={retry}
          >
            重试
          </Button>
        </div>
      )}

      {!hasYesterdayData && (
        <div className="bg-surface border-hairline p-card-inner text-caption text-tertiary rounded-lg border text-center">
          {yesterdayError
            ? "昨日涨停数据加载失败，晋级/断板统计可能不完整"
            : "未获取到昨日涨停列表，断板数据可能缺失"}
          {yesterdayError && (
            <Button
              variant="link"
              size="xs"
              className="ml-1 h-auto p-0 align-baseline"
              onClick={() => void refetchYesterday()}
            >
              重试
            </Button>
          )}
        </div>
      )}

      <div className="space-y-3">
        <div className="flex flex-wrap items-center gap-3">
          <label className="text-secondary text-caption flex cursor-pointer items-center gap-3 select-none">
            <Checkbox checked={hideST} onCheckedChange={(checked) => setHideST(checked === true)} />
            <span>隐藏ST{hideST && stFilteredCount > 0 ? ` · ${stFilteredCount}只` : ""}</span>
          </label>
          <div className="flex flex-wrap items-center gap-3">
            {STATUS_FILTERS.map((item) => (
              <Button
                key={item.id}
                type="button"
                variant="outline"
                size="xs"
                aria-pressed={statusFilter === item.id}
                onClick={() => handleStatusFilter(item.id)}
                className={filterChipClass(statusFilter === item.id)}
              >
                {item.label}
              </Button>
            ))}
          </div>
          <span className="text-caption text-tertiary ml-auto">显示 {visibleCount} 只</span>
        </div>

        <div className="flex flex-wrap items-center gap-3">
          <span className="text-caption text-tertiary">昨日板级</span>
          <div className="flex flex-wrap items-center gap-3">
            {BOARD_FILTERS.map((item) => (
              <Button
                key={item.id}
                type="button"
                variant="outline"
                size="xs"
                aria-pressed={boardFilter === item.id}
                onClick={() => handleBoardFilter(item.id)}
                className={filterChipClass(boardFilter === item.id)}
              >
                {item.label}
              </Button>
            ))}
          </div>
          {boardFilter !== "all" && (
            <span className="text-caption text-tertiary">仅作用于连板梯队</span>
          )}
          {(statusFilter !== "all" || boardFilter !== "all") && (
            <Button
              type="button"
              variant="link"
              size="xs"
              className="text-caption h-auto p-0"
              onClick={resetFilters}
            >
              清除筛选
            </Button>
          )}
        </div>
      </div>

      {visibleCount === 0 ? (
        <div className="bg-surface border-hairline p-card-inner text-caption text-tertiary rounded-lg border text-center">
          当前筛选下无数据
        </div>
      ) : (
        <>
          {ladder.length > 0 && (
            <div>
              <div className="mb-3 flex flex-wrap items-baseline gap-x-3 gap-y-1">
                <h3 className="text-h2 text-primary">连板晋级梯队</h3>
                <span className="text-caption text-tertiary">
                  昨 {filteredYesterdayTotal} 只 → 今 {filteredTodayTotal} 只 · 综合晋级率{" "}
                  <b className={`font-number ${rateClass(filteredRate)}`}>
                    {formatRate(filteredRate)}
                  </b>
                </span>
              </div>

              <div className="bg-surface border-hairline overflow-hidden rounded-lg border">
                {ladder.map((row) => (
                  <div key={row.yesterdayDays} className="border-hairline border-b last:border-b-0">
                    <Button
                      type="button"
                      variant="ghost"
                      aria-expanded={selectedBoard === row.yesterdayDays}
                      className="hover:bg-hover aria-expanded:bg-hover hover:text-primary flex h-auto w-full flex-col items-stretch justify-start gap-3 rounded-none px-5 py-4 text-left font-normal whitespace-normal"
                      onClick={() =>
                        setSelectedBoard(
                          selectedBoard === row.yesterdayDays ? null : row.yesterdayDays
                        )
                      }
                    >
                      <span className="flex flex-wrap items-center gap-x-3 gap-y-2">
                        <span className="text-caption text-secondary bg-hover min-w-14 rounded-sm px-2.5 py-1 text-center">
                          昨{row.yesterdayDays}板
                        </span>
                        <span className="text-caption text-tertiary">{row.yesterdayCount}只</span>
                        <span className="text-caption text-tertiary">→</span>
                        <span className="text-caption text-primary bg-brand/10 min-w-14 rounded-sm px-2.5 py-1 text-center">
                          今{row.yesterdayDays + 1}板
                        </span>
                        <span className="text-caption text-primary">{row.promotedCount}只</span>
                        <span className="text-caption text-tertiary">晋级率</span>
                        <b className={`font-number text-caption ${rateClass(row.rate)}`}>
                          {formatRate(row.rate)}
                        </b>
                        <span className="text-caption text-tertiary ml-auto">
                          断板 {row.brokenCount}只
                        </span>
                      </span>
                      <span className="bg-hover block h-2 w-full overflow-hidden rounded-full">
                        <span
                          className={`block h-full rounded-full transition-[width] ${
                            row.rate >= 0.5 ? "bg-up" : "bg-warning"
                          }`}
                          style={{ width: `${row.rate * 100}%` }}
                        />
                      </span>
                    </Button>

                    {selectedBoard === row.yesterdayDays && (
                      <div className="border-hairline overflow-x-auto border-t px-5 pt-3 pb-4">
                        <table className="data-table">
                          <thead>
                            <tr>
                              <th className="whitespace-nowrap">代码</th>
                              <th className="whitespace-nowrap">名称</th>
                              <th className="whitespace-nowrap">昨板→今板</th>
                              <th className="text-right whitespace-nowrap">换手率</th>
                              <th className="text-right whitespace-nowrap">流通市值</th>
                              <th className="whitespace-nowrap">状态</th>
                            </tr>
                          </thead>
                          <tbody>
                            {row.details.map((stock) => (
                              <tr key={stock.symbol}>
                                <td className="whitespace-nowrap">
                                  <StockLink item={stock} kind="code" onOpenKline={openKline} />
                                </td>
                                <td className="font-medium whitespace-nowrap">
                                  <StockLink item={stock} kind="name" onOpenKline={openKline} />
                                </td>
                                <td className="font-number whitespace-nowrap">
                                  <span
                                    className={
                                      stock.status === "promoted" ? "text-up" : "text-tertiary"
                                    }
                                  >
                                    {stock.yesterdayDays}板 →{" "}
                                    {stock.status === "promoted" ? `${stock.todayDays}板` : "断板"}
                                  </span>
                                </td>
                                <td className="font-number text-right whitespace-nowrap">
                                  {formatTurnover(stock.turnover)}
                                </td>
                                <td className="font-number text-right whitespace-nowrap">
                                  {formatCompactNumber(stock.capital)}
                                </td>
                                <td className="whitespace-nowrap">
                                  <span
                                    className={`text-caption font-number rounded-sm px-2.5 py-1 ${
                                      stock.status === "promoted"
                                        ? "bg-up/15 text-up"
                                        : "bg-hover text-tertiary"
                                    }`}
                                  >
                                    {stock.status === "promoted" ? "晋级" : "断板"}
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

          {boardFilter !== "all" && ladder.length === 0 && firstBoard.length > 0 && (
            <div className="bg-surface border-hairline p-card-inner text-caption text-tertiary rounded-lg border text-center">
              该板级暂无连板数据
            </div>
          )}

          {firstBoard.length > 0 && (
            <div>
              <div className="mb-3 flex flex-wrap items-baseline gap-x-3 gap-y-1">
                <h3 className="text-h2 text-primary">首板新进</h3>
                <span className="text-caption text-tertiary">共 {firstBoard.length} 只</span>
                <span className="text-caption text-tertiary">按流通市值排序</span>
              </div>
              <div className="bg-surface border-hairline overflow-hidden rounded-lg border">
                <div className="overflow-x-auto">
                  <table className="data-table">
                    <thead>
                      <tr>
                        <th className="whitespace-nowrap">代码</th>
                        <th className="whitespace-nowrap">名称</th>
                        <th className="whitespace-nowrap">现价</th>
                        <th className="text-right whitespace-nowrap">涨跌幅</th>
                        <th className="text-right whitespace-nowrap">换手率</th>
                        <th className="text-right whitespace-nowrap">流通市值</th>
                        <th className="whitespace-nowrap">板块</th>
                      </tr>
                    </thead>
                    <tbody>
                      {firstBoard.map((stock) => (
                        <tr key={stock.symbol}>
                          <td className="whitespace-nowrap">
                            <StockLink item={stock} kind="code" onOpenKline={openKline} />
                          </td>
                          <td className="font-medium whitespace-nowrap">
                            <StockLink item={stock} kind="name" onOpenKline={openKline} />
                          </td>
                          <td className="font-number text-tertiary whitespace-nowrap">
                            {stock.price.toFixed(2)}
                          </td>
                          <td className="text-up font-number text-right whitespace-nowrap">
                            +{(stock.changePercent * 100).toFixed(2)}%
                          </td>
                          <td className="font-number text-right whitespace-nowrap">
                            {formatTurnover(stock.turnover)}
                          </td>
                          <td className="font-number text-right whitespace-nowrap">
                            {formatCompactNumber(stock.capital)}
                          </td>
                          <td
                            className="text-caption text-tertiary max-w-40 truncate font-sans whitespace-nowrap"
                            title={stock.reason || "-"}
                          >
                            {stock.reason || "-"}
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              </div>
            </div>
          )}
        </>
      )}
    </div>
  )
}
