/**
 * 模拟盘「区间收益分析」面板，参考同花顺「资产分析」的交互：
 *
 * - 维度：当日 / 本月 / 近三月 / 今年 / 全部 / 自定义
 * - 视图：曲线 / 日历
 * - 指标：收益率 / 盈亏金额 / 总资产
 * - 汇总：区间累计收益 + 三大指数同期表现（跑赢/落后多少）
 *
 * 数据全部来自 `/api/roles/:name/pnl-calendar`，后端已用**当日收盘价**给历史
 * 持仓估值，所以每一天都是真实浮动盈亏，而不是只在成交日跳变的阶梯。
 */
import { useEffect, useMemo, useState } from "react"
import { useQueryClient } from "@tanstack/react-query"
import { AlertTriangle, ChevronLeft, ChevronRight } from "lucide-react"

import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Skeleton } from "@/components/ui/skeleton"
import { formatCompactNumber, formatPct, pctClass } from "@/lib/format"
import {
  PNL_CALENDAR_STALE_MS,
  pnlCalendarQueryOptions,
  usePnlCalendar,
} from "@/hooks/query/useRoleQueries"
import type { PnlCalendarPoint } from "@/stores/data"

type Dimension = "today" | "month" | "quarter" | "year" | "all" | "custom"
type ViewMode = "curve" | "calendar"
type Metric = "pct" | "amount" | "equity"

const DIMENSIONS: { key: Dimension; label: string }[] = [
  { key: "today", label: "当日" },
  { key: "month", label: "本月" },
  { key: "quarter", label: "近三月" },
  { key: "year", label: "今年" },
  { key: "all", label: "全部" },
  { key: "custom", label: "自定义" },
]

const METRICS: { key: Metric; label: string }[] = [
  { key: "pct", label: "收益率" },
  { key: "amount", label: "盈亏金额" },
  { key: "equity", label: "总资产" },
]

/** 日历展示的是每日数据，指标名称必须和曲线视图区分开。 */
const CALENDAR_METRICS: { key: Metric; label: string }[] = [
  { key: "pct", label: "每日收益" },
  { key: "amount", label: "每日盈亏" },
  { key: "equity", label: "总资产" },
]

/** 用北京时间的自然日，避免和行情/交易日历差一天。 */
const DATE_FMT = new Intl.DateTimeFormat("sv-SE", {
  timeZone: "Asia/Shanghai",
  year: "numeric",
  month: "2-digit",
  day: "2-digit",
})

function todayStr(): string {
  return DATE_FMT.format(new Date())
}

function monthStart(date: string): string {
  return `${date.slice(0, 7)}-01`
}

function yearStart(date: string): string {
  return `${date.slice(0, 4)}-01-01`
}

/** `2026-10-22` 加/减月份，跨年自动进位；月末溢出的日子夹到目标月最后一天。 */
function addMonths(date: string, delta: number): string {
  const [y, m, d] = date.split("-").map(Number)
  const targetMonth = m - 1 + delta
  const lastDay = new Date(Date.UTC(y, targetMonth + 1, 0)).getUTCDate()
  const shifted = new Date(Date.UTC(y, targetMonth, Math.min(d, lastDay)))
  return shifted.toISOString().slice(0, 10)
}

/** 某月的最后一天（`2026-02` → `2026-02-28`）。 */
function monthEnd(month: string): string {
  const [y, m] = month.split("-").map(Number)
  const lastDay = new Date(Date.UTC(y, m, 0)).getUTCDate()
  return `${month}-${String(lastDay).padStart(2, "0")}`
}

/** 某月的查询区间：月初到月末，但不超过今天。 */
function monthRange(month: string): { from: string; to: string } {
  const end = monthEnd(month)
  const today = todayStr()
  return { from: `${month}-01`, to: end > today ? today : end }
}

/** `2026-10-22` → `2026-10` */
function monthOf(date: string): string {
  return date.slice(0, 7)
}

function monthLabel(month: string): string {
  const [year, value] = month.split("-")
  return `${year}年${Number(value)}月`
}

function compactValue(value: number): string {
  return value === 0 ? "0" : formatCompactNumber(value)
}

/** 曲线点可能按日（`YYYY-MM-DD`）或按月（`YYYY-MM`）聚合。 */
function chartDateValue(value: string): number {
  if (value.length === 7) {
    const [year, month] = value.split("-").map(Number)
    return Date.UTC(year, month, 0)
  }
  const [year, month, day] = value.split("-").map(Number)
  return Date.UTC(year, month - 1, day)
}

function chartTickLabel(value: number, spanDays: number): string {
  const date = new Date(value)
  const month = date.getUTCMonth() + 1
  const day = date.getUTCDate()
  return spanDays >= 365
    ? `${date.getUTCFullYear()}-${String(month).padStart(2, "0")}`
    : `${month}-${day}`
}

function rangeOf(dim: Dimension, custom: { from: string; to: string }) {
  const to = todayStr()
  switch (dim) {
    case "today":
      return { from: to, to }
    case "month":
      return { from: monthStart(to), to }
    case "quarter":
      return { from: addMonths(to, -3), to }
    case "year":
      return { from: yearStart(to), to }
    case "all":
      // 后端只从「首笔成交日」开始取数，所以这个起点不会被真的拉满
      return { from: "2000-01-01", to }
    case "custom":
      return custom
  }
}

function money(value: number): string {
  return value.toLocaleString("zh-CN", { minimumFractionDigits: 2, maximumFractionDigits: 2 })
}

function signedMoney(value: number): string {
  return `${value >= 0 ? "+" : "-"}¥${money(Math.abs(value))}`
}

/** 指标对应的数值与格式化。 */
function metricValue(point: PnlCalendarPoint, metric: Metric): number {
  switch (metric) {
    case "pct":
      return point.cum_pct
    case "amount":
      return point.cum_pnl
    case "equity":
      return point.equity
  }
}

/** 页面内统一的分段控件，避免维度、视图、指标出现三套选中态。 */
function SegmentedControl<T extends string>({
  label,
  value,
  options,
  onChange,
}: {
  label: string
  value: T
  options: readonly { key: T; label: string }[]
  onChange: (value: T) => void
}) {
  return (
    <div
      role="group"
      aria-label={label}
      className="bg-hover flex w-fit max-w-full flex-wrap gap-0.5 rounded-md p-0.5"
    >
      {options.map((option) => (
        <button
          key={option.key}
          type="button"
          aria-pressed={value === option.key}
          onClick={() => onChange(option.key)}
          className={`text-caption focus-visible:ring-brand rounded px-2.5 py-1 transition-colors focus-visible:ring-1 focus-visible:outline-none ${
            value === option.key
              ? "bg-surface text-primary shadow-card"
              : "text-secondary hover:text-primary"
          }`}
        >
          {option.label}
        </button>
      ))}
    </div>
  )
}

/**
 * 年/全部维度下按自然月聚合，避免几百个点挤在一条曲线里看不清趋势。
 * 取每月最后一个交易日，与后端 month 粒度的口径一致。
 */
function bucketPoints(points: PnlCalendarPoint[], dim: Dimension): PnlCalendarPoint[] {
  if (dim !== "year" && dim !== "all") return points
  const byMonth = new Map<string, PnlCalendarPoint>()
  for (const point of points) {
    byMonth.set(point.date.slice(0, 7), point)
  }
  return [...byMonth.entries()].map(([month, last]) => ({ ...last, date: month }))
}

function CurveChart({
  points,
  metric,
  from,
  to,
  loading = false,
}: {
  points: PnlCalendarPoint[]
  metric: Metric
  from: string
  to: string
  loading?: boolean
}) {
  const W = 720
  const H = 240
  const padL = 56
  const padR = 16
  const padT = 16
  const padB = 28

  // 横轴属于所选时间区间，而不是数据点本身；这样无数据时也能提前画好，
  // 数据到达后只需按日期落到固定位置，不会因为点数变化而整条轴伸缩。
  const domainStart = chartDateValue(from)
  const domainEnd = Math.max(domainStart, chartDateValue(to))
  const spanDays = Math.round((domainEnd - domainStart) / 86_400_000)
  const tickCount = spanDays === 0 ? 1 : Math.min(6, spanDays <= 5 ? spanDays + 1 : 6)
  const xTicks = Array.from({ length: tickCount }, (_, i) => {
    if (tickCount === 1) return domainStart
    return domainStart + ((domainEnd - domainStart) * i) / (tickCount - 1)
  })

  const hasData = points.length > 0
  const values = points.map((p) => metricValue(p, metric))
  const zeroBased = metric !== "equity"
  const rawMin = hasData
    ? Math.min(...values, zeroBased ? 0 : Number.POSITIVE_INFINITY)
    : zeroBased
      ? 0
      : Number.POSITIVE_INFINITY
  const rawMax = hasData
    ? Math.max(...values, zeroBased ? 0 : Number.NEGATIVE_INFINITY)
    : zeroBased
      ? 0
      : Number.NEGATIVE_INFINITY
  const padding = (rawMax - rawMin || Math.abs(rawMax) || 1) * 0.08
  const min = rawMin - padding
  const max = rawMax + padding
  const range = max - min || 1

  const plotW = W - padL - padR
  const plotH = H - padT - padB
  const dateToX = (value: string) => {
    if (domainEnd === domainStart) return padL + plotW / 2
    const ratio = (chartDateValue(value) - domainStart) / (domainEnd - domainStart)
    return padL + Math.min(1, Math.max(0, ratio)) * plotW
  }
  const x = (value: number) => {
    if (domainEnd === domainStart) return padL + plotW / 2
    const ratio = (value - domainStart) / (domainEnd - domainStart)
    return padL + Math.min(1, Math.max(0, ratio)) * plotW
  }
  const y = (v: number) => padT + (1 - (v - min) / range) * plotH

  const path = points
    .map(
      (p, i) =>
        `${i === 0 ? "M" : "L"}${dateToX(p.date).toFixed(1)},${y(metricValue(p, metric)).toFixed(1)}`
    )
    .join(" ")
  const last = points.length - 1
  const lastValue = hasData ? metricValue(points[last], metric) : 0
  const tone =
    metric === "equity"
      ? "var(--color-brand)"
      : lastValue > 0
        ? "var(--color-up)"
        : lastValue < 0
          ? "var(--color-down)"
          : "var(--color-brand)"
  const area =
    points.length > 1
      ? `${path} L${dateToX(points[last].date).toFixed(1)},${y(zeroBased ? 0 : min).toFixed(1)} L${dateToX(points[0].date).toFixed(1)},${y(zeroBased ? 0 : min).toFixed(1)} Z`
      : ""

  const gridLines = Array.from({ length: 5 }, (_, i) => min + (range * i) / 4)

  return (
    <svg
      viewBox={`0 0 ${W} ${H}`}
      className="h-[260px] w-full cursor-crosshair"
      role="img"
      aria-label="区间收益曲线"
    >
      {gridLines.map((v) => (
        <g key={`grid-${v}`}>
          <line
            x1={padL}
            y1={y(v)}
            x2={W - padR}
            y2={y(v)}
            stroke="var(--color-hairline)"
            strokeWidth="0.5"
            strokeDasharray="3,3"
          />
          {hasData && (
            <text
              x={padL - 8}
              y={y(v) + 3}
              textAnchor="end"
              fontSize="9"
              fill="var(--color-tertiary)"
            >
              {metric === "pct" ? `${v.toFixed(1)}%` : compactValue(v)}
            </text>
          )}
        </g>
      ))}

      {xTicks.map((value, i) => (
        <g key={`x-grid-${i}`}>
          <line
            x1={x(value)}
            y1={padT}
            x2={x(value)}
            y2={H - padB}
            stroke="var(--color-hairline)"
            strokeWidth="0.5"
            opacity="0.55"
          />
          <text
            x={x(value)}
            y={H - 8}
            textAnchor="middle"
            fontSize="9"
            fill="var(--color-tertiary)"
          >
            {chartTickLabel(value, spanDays)}
          </text>
        </g>
      ))}

      <line
        x1={padL}
        y1={H - padB}
        x2={W - padR}
        y2={H - padB}
        stroke="var(--color-tertiary)"
        strokeWidth="0.6"
        opacity="0.45"
      />

      {hasData && zeroBased && (
        <line
          x1={padL}
          y1={y(0)}
          x2={W - padR}
          y2={y(0)}
          stroke="var(--color-tertiary)"
          strokeWidth="0.8"
        />
      )}

      {area && <path d={area} fill={tone} opacity="0.08" />}
      {path && <path d={path} fill="none" stroke={tone} strokeWidth="1.8" strokeLinejoin="round" />}

      {!hasData && (
        <text
          x={padL + plotW / 2}
          y={padT + plotH / 2}
          textAnchor="middle"
          dominantBaseline="middle"
          fontSize="11"
          fill="var(--color-tertiary)"
        >
          {loading ? "加载收益数据…" : "暂无收益数据"}
        </text>
      )}

      {hasData &&
        points.map((p, i) =>
          i === last || points.length <= 1 ? (
            <circle
              key={`dot-${p.date}`}
              cx={dateToX(p.date)}
              cy={y(metricValue(p, metric))}
              r="3.5"
              fill={tone}
              stroke="var(--color-surface)"
              strokeWidth="1.5"
            />
          ) : null
        )}

      {/* 透明热区提供逐点原生提示，不增加折线视觉噪音。 */}
      {hasData &&
        points.map((p) => (
          <circle
            key={`hit-${p.date}`}
            cx={dateToX(p.date)}
            cy={y(metricValue(p, metric))}
            r="10"
            fill="transparent"
          >
            <title>{`${p.date}
累计 ${signedMoney(p.cum_pnl)}（${formatPct(p.cum_pct)}）
权益 ¥${money(p.equity)}`}</title>
          </circle>
        ))}
    </svg>
  )
}

/** 月历：每格显示当日收益，休市日显示「休」，今天显示「今」。 */
function CalendarGrid({
  month,
  points,
  tradingDates,
  metric,
  from,
  to,
}: {
  month: string
  points: PnlCalendarPoint[]
  tradingDates: string[]
  metric: Metric
  from: string
  to: string
}) {
  const byDate = useMemo(() => new Map(points.map((p) => [p.date, p])), [points])
  const tradingSet = useMemo(() => new Set(tradingDates), [tradingDates])
  const [year, mon] = month.split("-").map(Number)
  const firstWeekday = new Date(Date.UTC(year, mon - 1, 1)).getUTCDay()
  const daysInMonth = new Date(Date.UTC(year, mon, 0)).getUTCDate()
  const today = todayStr()

  const cells: Array<{ date: string | null; day: number }> = []
  for (let i = 0; i < firstWeekday; i += 1) cells.push({ date: null, day: 0 })
  for (let d = 1; d <= daysInMonth; d += 1) {
    cells.push({ date: `${month}-${String(d).padStart(2, "0")}`, day: d })
  }

  return (
    <div className="px-5 py-4">
      <div className="text-tertiary text-micro mb-1 grid grid-cols-7 gap-1 text-center">
        {["日", "一", "二", "三", "四", "五", "六"].map((w) => (
          <span key={w}>{w}</span>
        ))}
      </div>
      <div className="grid grid-cols-7 gap-1">
        {cells.map((cell, idx) => {
          if (!cell.date) return <div key={`empty-${idx}`} />
          const point = byDate.get(cell.date)
          const inRange = cell.date >= from && cell.date <= to
          const isToday = cell.date === today
          const isFuture = cell.date > today

          const dayLabel = (
            <span
              className={
                isToday
                  ? "bg-brand text-brand-foreground text-micro inline-flex size-5 items-center justify-center rounded-full"
                  : "text-tertiary text-micro"
              }
            >
              {isToday ? "今" : cell.day}
            </span>
          )

          if (!inRange || isFuture) {
            return (
              <div key={cell.date} className="flex flex-col items-center gap-0.5 py-1 opacity-40">
                {dayLabel}
              </div>
            )
          }

          if (!point) {
            // 开市但空仓：收益为 0（和同花顺一致）；只有真正的非交易日才显示「休」
            const isTradingDay = tradingSet.has(cell.date)
            return (
              <div
                key={cell.date}
                className="flex flex-col items-center gap-0.5 py-1"
                title={isTradingDay ? `${cell.date} · 无持仓，当日收益 0` : `${cell.date} · 休市`}
              >
                {dayLabel}
                {isTradingDay ? (
                  <span className="text-tertiary text-micro tabular-nums">0</span>
                ) : (
                  <span className="text-warning text-micro">休</span>
                )}
              </div>
            )
          }

          const value =
            metric === "equity" ? point.equity : metric === "pct" ? point.pct : point.pnl
          return (
            <div
              key={cell.date}
              className="hover:bg-hover flex flex-col items-center gap-0.5 rounded-md py-1"
              title={`${cell.date}  当日盈亏 ${signedMoney(point.pnl)}（${formatPct(point.pct)}）\n累计 ${signedMoney(point.cum_pnl)}（${formatPct(point.cum_pct)}）\n权益 ¥${money(point.equity)}`}
            >
              {dayLabel}
              <span className={`text-micro tabular-nums ${metricClass(value, metric)}`}>
                {metric === "pct" ? value.toFixed(2) : compactValue(value)}
              </span>
            </div>
          )
        })}
      </div>
    </div>
  )
}

function metricClass(value: number, metric: Metric): string {
  if (metric === "equity") return "text-primary"
  return pctClass(value)
}

export function PnlPanel({ roleName }: { roleName: string }) {
  const [dimension, setDimension] = useState<Dimension>("month")
  const [view, setView] = useState<ViewMode>("curve")
  const [metric, setMetric] = useState<Metric>("pct")
  const [customFrom, setCustomFrom] = useState(() => monthStart(todayStr()))
  const [customTo, setCustomTo] = useState(() => todayStr())
  // 日历按月独立取数；曲线和区间汇总仍由维度决定。
  const [browsedMonth, setBrowsedMonth] = useState(() => monthOf(todayStr()))

  const dimensionRange = rangeOf(dimension, { from: customFrom, to: customTo })
  const queryRange = view === "calendar" ? monthRange(browsedMonth) : dimensionRange

  const query = usePnlCalendar(roleName, queryRange.from, queryRange.to)
  const calendar = query.data
  const { from, to } = queryRange
  const displayMonth = browsedMonth

  const curvePoints = useMemo(
    () => bucketPoints(calendar?.points ?? [], dimension),
    [calendar, dimension]
  )

  // 往前翻到「首笔成交月」；账户还没有成交时给 11 个月的回看窗口，
  // 否则箭头一开始就是灰的，看着像坏了。往后不越过当前月。
  const currentMonth = monthOf(todayStr())
  const windowFloor = monthOf(addMonths(`${currentMonth}-01`, -11))
  const tradeFloor = calendar?.first_trade_date ? monthOf(calendar.first_trade_date) : windowFloor
  const earliestMonth = tradeFloor < windowFloor ? tradeFloor : windowFloor
  const canGoPrev = browsedMonth > earliestMonth
  const canGoNext = browsedMonth < currentMonth

  // 预取前后各一个月：切月时命中缓存，不会因为新 query key 的 isPending
  // 把整个面板替换成骨架屏。月份边界与翻页按钮保持一致。
  const queryClient = useQueryClient()
  const calendarReady = calendar != null
  const prefetchMonths = useMemo(() => {
    if (view !== "calendar" || !calendarReady) return []
    const months: string[] = []
    if (canGoPrev) months.push(monthOf(addMonths(`${browsedMonth}-01`, -1)))
    if (canGoNext) months.push(monthOf(addMonths(`${browsedMonth}-01`, 1)))
    return months
  }, [view, calendarReady, browsedMonth, canGoPrev, canGoNext])

  useEffect(() => {
    for (const month of prefetchMonths) {
      const range = monthRange(month)
      void queryClient.prefetchQuery({
        ...pnlCalendarQueryOptions(roleName, range.from, range.to),
        staleTime: PNL_CALENDAR_STALE_MS,
        // 面板常开时当前月会轮询，但相邻月无人观察；默认 5 分钟 GC 会让预取失效。
        gcTime: 30 * 60_000,
      })
    }
  }, [prefetchMonths, queryClient, roleName])

  if (query.error) {
    return (
      <div className="flex flex-col items-center gap-2 px-5 py-10 text-center">
        <AlertTriangle className="text-danger size-4" />
        <p className="text-danger text-caption">
          区间收益加载失败：
          {query.error instanceof Error ? query.error.message : String(query.error)}
        </p>
        <Button variant="outline" size="xs" onClick={() => void query.refetch()}>
          重试
        </Button>
      </div>
    )
  }

  const summary = calendar?.summary
  const lastEquity = calendar?.points.at(-1)?.equity ?? 0
  const shanghai = calendar?.benchmarks.find((b) => b.name === "上证")
  const excess = summary && shanghai?.pct != null ? summary.total_pct - shanghai.pct : null
  const otherBenchmarks = calendar?.benchmarks.filter((b) => b.name !== "上证") ?? []
  const summaryLabel =
    view === "calendar"
      ? `${monthLabel(displayMonth)}收益`
      : from === to
        ? "当日收益"
        : "区间累计收益"
  const metricOptions = view === "calendar" ? CALENDAR_METRICS : METRICS
  // “全部”的查询起点是 2000 年，只用于取数；横轴应从首笔成交日开始，
  // 否则真实数据会被压缩到最右侧。无成交时用今年作为可读的默认区间。
  const curveRange = {
    from:
      dimension === "all"
        ? (calendar?.first_trade_date ?? yearStart(todayStr()))
        : dimensionRange.from,
    to: dimensionRange.to,
  }

  return (
    <div>
      {/* 先给结论，再提供筛选与明细。 */}
      <div className="flex flex-wrap items-end justify-between gap-x-8 gap-y-5 px-5 py-5">
        <div>
          <div className="text-tertiary text-caption">{summaryLabel}</div>
          {summary ? (
            <>
              <div
                className={`text-hero mt-0.5 leading-tight font-medium tabular-nums ${pctClass(summary.total_pct)}`}
              >
                {formatPct(summary.total_pct)}
              </div>
              <div className="mt-3 flex flex-wrap gap-x-7 gap-y-2">
                <div>
                  <div className="text-tertiary text-micro">盈亏</div>
                  <div
                    className={`text-body font-medium tabular-nums ${pctClass(summary.total_pnl)}`}
                  >
                    {signedMoney(summary.total_pnl)}
                  </div>
                </div>
                <div>
                  <div className="text-tertiary text-micro">权益</div>
                  <div className="text-body text-primary font-medium tabular-nums">
                    ¥{money(lastEquity)}
                  </div>
                </div>
              </div>
            </>
          ) : (
            <div className="mt-2 space-y-2">
              <Skeleton className="h-9 w-36" />
              <Skeleton className="h-5 w-52" />
            </div>
          )}
        </div>

        <div className="flex flex-col gap-1.5 sm:items-end">
          {excess != null && (
            <div className={`text-caption tabular-nums ${pctClass(excess)}`}>
              较上证 <span className="font-medium">{formatPct(excess)}</span>
            </div>
          )}
          {otherBenchmarks.length > 0 && (
            <div className="text-tertiary text-caption flex flex-wrap gap-x-3 gap-y-0.5 sm:justify-end">
              {otherBenchmarks.map((b) => (
                <span key={b.code}>
                  {b.name}{" "}
                  <span className={pctClass(b.pct ?? 0)}>
                    {b.pct == null ? "—" : formatPct(b.pct)}
                  </span>
                </span>
              ))}
            </div>
          )}
        </div>
      </div>

      <div className="border-hairline space-y-2 border-y px-5 py-3">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <SegmentedControl<ViewMode>
            label="视图"
            value={view}
            options={[
              { key: "curve", label: "曲线" },
              { key: "calendar", label: "日历" },
            ]}
            onChange={setView}
          />
          <SegmentedControl<Metric>
            label="指标"
            value={metric}
            options={metricOptions}
            onChange={setMetric}
          />
        </div>

        <div className="flex flex-wrap items-center gap-2">
          {view === "curve" ? (
            <>
              <SegmentedControl
                label="时间范围"
                value={dimension}
                options={DIMENSIONS}
                onChange={(value) => {
                  setDimension(value)
                  setBrowsedMonth(monthOf(todayStr()))
                }}
              />
              {dimension === "custom" && (
                <div className="flex items-center gap-1.5">
                  <Input
                    type="date"
                    aria-label="开始日期"
                    value={customFrom}
                    max={customTo}
                    onChange={(e) => setCustomFrom(e.target.value)}
                    className="text-caption h-7 w-[130px]"
                  />
                  <span className="text-tertiary text-caption">至</span>
                  <Input
                    type="date"
                    aria-label="结束日期"
                    value={customTo}
                    min={customFrom}
                    onChange={(e) => setCustomTo(e.target.value)}
                    className="text-caption h-7 w-[130px]"
                  />
                </div>
              )}
            </>
          ) : (
            <div className="flex items-center gap-1">
              <Button
                type="button"
                variant="ghost"
                size="icon-xs"
                aria-label="上个月"
                onClick={() => setBrowsedMonth((m) => monthOf(addMonths(`${m}-01`, -1)))}
                disabled={!canGoPrev}
              >
                <ChevronLeft className="size-4" />
              </Button>
              <span className="text-secondary text-caption min-w-24 text-center tabular-nums">
                {monthLabel(displayMonth)}
              </span>
              <Button
                type="button"
                variant="ghost"
                size="icon-xs"
                aria-label="下个月"
                onClick={() => setBrowsedMonth((m) => monthOf(addMonths(`${m}-01`, 1)))}
                disabled={!canGoNext}
              >
                <ChevronRight className="size-4" />
              </Button>
            </div>
          )}
        </div>
      </div>

      {view === "curve" ? (
        <div className="px-2 pt-3">
          <CurveChart
            points={curvePoints}
            metric={metric}
            from={curveRange.from}
            to={curveRange.to}
            loading={query.isPending}
          />
        </div>
      ) : calendar ? (
        <CalendarGrid
          month={displayMonth}
          points={calendar.points}
          tradingDates={calendar.trading_dates}
          metric={metric}
          from={from}
          to={to}
        />
      ) : (
        <div className="px-5 py-4">
          <Skeleton className="h-[270px] w-full" />
        </div>
      )}

      {summary && (
        <div className="border-hairline text-secondary text-caption flex flex-wrap gap-x-5 gap-y-1 border-t px-5 py-3">
          <span>
            最大回撤{" "}
            <span className="text-primary tabular-nums">
              {summary.max_drawdown_pct.toFixed(2)}%
            </span>
          </span>
          <span>
            上涨 <span className="text-up tabular-nums">{summary.win_days}</span> 天 / 下跌{" "}
            <span className="text-down tabular-nums">{summary.lose_days}</span> 天
          </span>
          <span className="text-tertiary">有持仓 {summary.trading_days} 个交易日</span>
        </div>
      )}

      {calendar?.incomplete && (
        <p className="text-warning border-hairline text-micro border-t px-5 py-2.5">
          部分持仓在某日拿不到收盘价，已按成本价估算，该日收益仅供参考
        </p>
      )}
    </div>
  )
}
