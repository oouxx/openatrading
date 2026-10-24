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
import { formatPct, pctClass } from "@/lib/format"
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
}: {
  points: PnlCalendarPoint[]
  metric: Metric
}) {
  const W = 720
  const H = 240
  const padL = 56
  const padR = 16
  const padT = 16
  const padB = 28

  const values = points.map((p) => metricValue(p, metric))
  const zeroBased = metric !== "equity"
  const rawMin = Math.min(...values, zeroBased ? 0 : Number.POSITIVE_INFINITY)
  const rawMax = Math.max(...values, zeroBased ? 0 : Number.NEGATIVE_INFINITY)
  const padding = (rawMax - rawMin || Math.abs(rawMax) || 1) * 0.08
  const min = rawMin - padding
  const max = rawMax + padding
  const range = max - min || 1

  const plotW = W - padL - padR
  const plotH = H - padT - padB
  const x = (i: number) => padL + (points.length <= 1 ? plotW / 2 : (i / (points.length - 1)) * plotW)
  const y = (v: number) => padT + (1 - (v - min) / range) * plotH

  const path = points
    .map((p, i) => `${i === 0 ? "M" : "L"}${x(i).toFixed(1)},${y(metricValue(p, metric)).toFixed(1)}`)
    .join(" ")
  const last = points.length - 1
  const area =
    points.length > 1
      ? `${path} L${x(last).toFixed(1)},${y(zeroBased ? 0 : min).toFixed(1)} L${x(0).toFixed(1)},${y(zeroBased ? 0 : min).toFixed(1)} Z`
      : ""

  const gridLines = Array.from({ length: 5 }, (_, i) => min + (range * i) / 4)
  const labelStep = Math.max(1, Math.ceil(points.length / 6))

  return (
    <svg viewBox={`0 0 ${W} ${H}`} className="h-[260px] w-full" role="img" aria-label="区间收益曲线">
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
          <text x={padL - 8} y={y(v) + 3} textAnchor="end" fontSize="9" fill="var(--color-tertiary)">
            {metric === "equity" ? money(v).split(".")[0] : metric === "pct" ? `${v.toFixed(1)}%` : v.toFixed(0)}
          </text>
        </g>
      ))}

      {zeroBased && (
        <line
          x1={padL}
          y1={y(0)}
          x2={W - padR}
          y2={y(0)}
          stroke="var(--color-tertiary)"
          strokeWidth="0.8"
        />
      )}

      {area && <path d={area} fill="var(--color-brand)" opacity="0.08" />}
      <path d={path} fill="none" stroke="var(--color-brand)" strokeWidth="1.8" strokeLinejoin="round" />

      {points.map((p, i) =>
        i === last || points.length <= 1 ? (
          <circle
            key={`dot-${p.date}`}
            cx={x(i)}
            cy={y(metricValue(p, metric))}
            r="3.5"
            fill="var(--color-brand)"
            stroke="var(--color-surface)"
            strokeWidth="1.5"
          />
        ) : null,
      )}

      {points.map((p, i) =>
        i % labelStep === 0 || i === last ? (
          <text
            key={`xt-${p.date}`}
            x={x(i)}
            y={H - 8}
            textAnchor="middle"
            fontSize="9"
            fill="var(--color-tertiary)"
          >
            {p.date.slice(5)}
          </text>
        ) : null,
      )}
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
      <div className="text-tertiary mb-1 grid grid-cols-7 gap-1 text-center text-micro">
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
                  ? "bg-brand text-brand-foreground inline-flex size-5 items-center justify-center rounded-full text-micro"
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

          const value = metric === "equity" ? point.equity : metric === "pct" ? point.pct : point.pnl
          return (
            <div
              key={cell.date}
              className="flex flex-col items-center gap-0.5 rounded-md py-1 hover:bg-hover"
              title={`${cell.date}  当日盈亏 ${signedMoney(point.pnl)}（${formatPct(point.pct)}）\n累计 ${signedMoney(point.cum_pnl)}（${formatPct(point.cum_pct)}）\n权益 ¥${money(point.equity)}`}
            >
              {dayLabel}
              <span className={`text-micro tabular-nums ${metricClass(value, metric)}`}>
                {metric === "equity"
                  ? money(value).split(".")[0]
                  : metric === "pct"
                    ? value.toFixed(2)
                    : value.toFixed(0)}
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
  // 日历**按月独立取数**：维度只决定曲线和汇总的区间，不该限制日历能翻哪些月。
  // 之前把日历绑在维度区间上，选「本月」时前后箭头全被禁用，看起来像切换坏了。
  const [browsedMonth, setBrowsedMonth] = useState(() => monthOf(todayStr()))

  const dimensionRange = rangeOf(dimension, { from: customFrom, to: customTo })
  // 曲线看维度区间；日历看当前浏览的整月（不超过今天）
  const queryRange = view === "calendar" ? monthRange(browsedMonth) : dimensionRange

  const query = usePnlCalendar(roleName, queryRange.from, queryRange.to)
  const calendar = query.data
  const { from, to } = queryRange
  const displayMonth = browsedMonth

  const curvePoints = useMemo(
    () => bucketPoints(calendar?.points ?? [], dimension),
    [calendar, dimension],
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

  if (query.isPending) {
    return (
      <div className="space-y-3 px-5 py-6">
        <Skeleton className="h-8 w-64" />
        <Skeleton className="h-[240px] w-full" />
      </div>
    )
  }

  if (query.error) {
    return (
      <div className="flex flex-col items-center gap-2 px-5 py-10 text-center">
        <AlertTriangle className="text-danger size-4" />
        <p className="text-danger text-caption">
          区间收益加载失败：{query.error instanceof Error ? query.error.message : String(query.error)}
        </p>
        <Button variant="outline" size="xs" onClick={() => void query.refetch()}>
          重试
        </Button>
      </div>
    )
  }

  if (!calendar) return null

  const summary = calendar.summary
  const shanghai = calendar.benchmarks.find((b) => b.name === "上证")
  const excess = shanghai?.pct != null ? summary.total_pct - shanghai.pct : null

  return (
    <div>
      {/* 维度 */}
      <div className="border-hairline flex flex-wrap items-center gap-1 border-b px-5 py-2.5">
        {DIMENSIONS.map((d) => (
          <button
            key={d.key}
            type="button"
            onClick={() => {
              setDimension(d.key)
              setBrowsedMonth(monthOf(todayStr()))
            }}
            className={`rounded-md px-3 py-1 text-caption transition-colors ${
              dimension === d.key
                ? "bg-brand text-brand-foreground"
                : "text-secondary hover:bg-ink-dim hover:text-primary"
            }`}
          >
            {d.label}
          </button>
        ))}
        {dimension === "custom" && (
          <div className="ml-2 flex items-center gap-1.5">
            <Input
              type="date"
              value={customFrom}
              max={customTo}
              onChange={(e) => setCustomFrom(e.target.value)}
              className="h-7 w-[130px] text-caption"
            />
            <span className="text-tertiary text-caption">至</span>
            <Input
              type="date"
              value={customTo}
              min={customFrom}
              onChange={(e) => setCustomTo(e.target.value)}
              className="h-7 w-[130px] text-caption"
            />
          </div>
        )}
      </div>

      {/* 汇总：区间收益 + 跑赢大盘 */}
      <div className="flex flex-wrap items-end justify-between gap-4 px-5 pt-5">
        <div>
          {/* 区间由上方维度 chip（当日/本月/近三月/…）的选中态给出，这里只说这个数字是什么 */}
          <div className="text-tertiary text-caption">{from === to ? "当日收益" : "区间累计收益"}</div>
          <div className={`text-hero leading-tight font-medium tabular-nums ${pctClass(summary.total_pct)}`}>
            {formatPct(summary.total_pct)}
          </div>
          <div className={`text-caption tabular-nums ${pctClass(summary.total_pnl)}`}>
            {signedMoney(summary.total_pnl)}  权益 ¥{money((curvePoints.at(-1)?.equity ?? 0))}
          </div>
        </div>
        <div className="text-right">
          {excess != null && (
            <div className={`text-caption ${pctClass(excess)}`}>
              {excess >= 0 ? "跑赢" : "跑输"}上证 {Math.abs(excess).toFixed(2)}%
            </div>
          )}
          <div className="text-tertiary text-caption mt-1 flex flex-wrap justify-end gap-x-3 gap-y-0.5">
            {calendar.benchmarks.map((b) => (
              <span key={b.code}>
                {b.name} <span className={pctClass(b.pct ?? 0)}>{b.pct == null ? "—" : formatPct(b.pct)}</span>
              </span>
            ))}
          </div>
        </div>
      </div>

      {/* 视图 / 指标 */}
      <div className="flex flex-wrap items-center justify-between gap-2 px-5 pt-4">
        <div className="border-hairline inline-flex overflow-hidden rounded-md border">
          {(["curve", "calendar"] as ViewMode[]).map((v) => (
            <button
              key={v}
              type="button"
              onClick={() => setView(v)}
              className={`px-3 py-1 text-caption ${
                view === v ? "bg-ink-dim text-primary" : "text-secondary hover:text-primary"
              }`}
            >
              {v === "curve" ? "曲线" : "日历"}
            </button>
          ))}
        </div>
        <div className="border-hairline inline-flex overflow-hidden rounded-md border">
          {METRICS.map((m) => (
            <button
              key={m.key}
              type="button"
              onClick={() => setMetric(m.key)}
              className={`px-3 py-1 text-caption ${
                metric === m.key ? "bg-ink-dim text-primary" : "text-secondary hover:text-primary"
              }`}
            >
              {m.label}
            </button>
          ))}
        </div>
      </div>

      {view === "curve" ? (
        curvePoints.length === 0 ? (
          <p className="text-tertiary text-caption py-14 text-center">该区间内没有收益数据</p>
        ) : (
          <div className="px-2 pt-2">
            <CurveChart points={curvePoints} metric={metric} />
          </div>
        )
      ) : (
        <div>
          <div className="flex items-center justify-center gap-3 pt-3">
            <button
              type="button"
              aria-label="上个月"
              onClick={() => setBrowsedMonth((m) => monthOf(addMonths(`${m}-01`, -1)))}
              className="text-tertiary hover:text-primary p-1 disabled:opacity-30"
              disabled={!canGoPrev}
            >
              <ChevronLeft className="size-4" />
            </button>
            <span className="text-primary text-body font-medium tabular-nums">
              {displayMonth.replace("-", " 年 ")} 月
            </span>
            <button
              type="button"
              aria-label="下个月"
              onClick={() => setBrowsedMonth((m) => monthOf(addMonths(`${m}-01`, 1)))}
              className="text-tertiary hover:text-primary p-1 disabled:opacity-30"
              disabled={!canGoNext}
            >
              <ChevronRight className="size-4" />
            </button>
          </div>
          <CalendarGrid
            month={displayMonth}
            points={calendar.points}
            tradingDates={calendar.trading_dates}
            metric={metric}
            from={from}
            to={to}
          />
        </div>
      )}

      {/* 统计脚注 */}
      <div className="border-hairline text-secondary text-caption flex flex-wrap gap-x-5 gap-y-1 border-t px-5 py-3">
        <span>
          最大回撤 <span className="text-primary tabular-nums">{summary.max_drawdown_pct.toFixed(2)}%</span>
        </span>
        <span>
          上涨 <span className="text-up tabular-nums">{summary.win_days}</span> 天 / 下跌{" "}
          <span className="text-down tabular-nums">{summary.lose_days}</span> 天
        </span>
        <span className="text-tertiary">有持仓 {summary.trading_days} 个交易日</span>
      </div>

      {calendar.incomplete && (
        <p className="text-warning border-hairline border-t px-5 py-2.5 text-micro">
          部分持仓在某日拿不到收盘价，已按成本价估算，该日收益仅供参考
        </p>
      )}
    </div>
  )
}
