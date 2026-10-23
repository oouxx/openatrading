import { useCallback, useEffect, useMemo, useState } from "react"
import { Pause, Play, RefreshCw, RotateCcw, X } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Skeleton } from "@/components/ui/skeleton"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import {
  useReplayDay,
  useReplayDays,
  useRefreshReplay,
  type ReplayDaySummary,
  type ReplayEvent,
} from "@/hooks/query/useReplay"
import { chartColors } from "@/lib/chartColors"
import { IndexMiniChart, RotationHeatmap, type IndexSeries } from "@/components/replay/ReplayCharts"
import {
  TRADING_MINUTES,
  buildRotation,
  buildSignals,
  clamp,
  eventDirection,
  eventKindLabel,
  eventName,
  eventPcp,
  eventTradingMinute,
  filterReplayEvents,
  formatPct,
  formatTradingMinute,
  normalizeIndex,
  type DirectionFilter,
  type Signal,
} from "@/components/replay/replayAnalysis"

const PCT_OPTIONS = [0, 1, 2, 3, 5]
const SPEEDS = [
  { value: 1, label: "1×" },
  { value: 5, label: "5×" },
  { value: 15, label: "15×" },
]
const EMPTY_EVENTS: ReplayEvent[] = []
const EVENT_PAGE_SIZE = 80
const HEATMAP_ROW_LIMIT = 36

function statusLabel(status: "partial" | "complete" | undefined): string {
  return status === "complete" ? "完整" : "盘中"
}

export default function ReplayView() {
  const [selectedDate, setSelectedDate] = useState<string | null>(null)
  const { data: days = [] } = useReplayDays()
  const {
    data: day,
    isLoading,
    isFetching,
    error,
    refetch,
  } = useReplayDay(selectedDate ?? undefined)
  const refresh = useRefreshReplay()

  const [cursor, setCursor] = useState(TRADING_MINUTES)
  const [playing, setPlaying] = useState(false)
  const [speed, setSpeed] = useState(5)
  const [direction, setDirection] = useState<DirectionFilter>("all")
  const [minPct, setMinPct] = useState(2)
  const [selectedId, setSelectedId] = useState<number | null>(null)
  const [showAllPlates, setShowAllPlates] = useState(false)
  const [visibleEventCount, setVisibleEventCount] = useState(EVENT_PAGE_SIZE)

  const events = day?.events ?? EMPTY_EVENTS
  const index = day?.index ?? null
  const activeDate = selectedDate ?? day?.date ?? ""
  const isSwitching = selectedDate !== null && day?.date !== selectedDate

  const dateOptions = useMemo(() => {
    const byDate = new Map<string, ReplayDaySummary>()
    for (const item of days) byDate.set(item.date, item)
    if (day) {
      byDate.set(day.date, {
        date: day.date,
        status: day.status,
        event_count: day.event_count,
        first_event_timestamp: day.first_event_timestamp,
        last_event_timestamp: day.last_event_timestamp,
        captured_at: "",
        completed_at: day.completed_at,
      })
    }
    return [...byDate.values()].sort((a, b) => b.date.localeCompare(a.date))
  }, [days, day])

  const filtered = useMemo(
    () => filterReplayEvents(events, { direction, minPct }),
    [events, direction, minPct]
  )
  const rotation = useMemo(() => buildRotation(filtered), [filtered])
  const signals = useMemo(() => buildSignals(filtered), [filtered])
  const topSignals = useMemo(() => {
    const seen = new Set<string>()
    const picked: Signal[] = []
    for (const signal of signals) {
      if (seen.has(signal.name)) continue
      seen.add(signal.name)
      picked.push(signal)
      if (picked.length >= 12) break
    }
    return picked
  }, [signals])
  const displayRotation = useMemo(() => {
    if (showAllPlates || rotation.rows.length <= HEATMAP_ROW_LIMIT) return rotation
    const keep = new Set(
      [...rotation.rows]
        .sort((a, b) => b.total - a.total)
        .slice(0, HEATMAP_ROW_LIMIT)
        .map((row) => row.name)
    )
    return { ...rotation, rows: rotation.rows.filter((row) => keep.has(row.name)) }
  }, [rotation, showAllPlates])
  const visibleEvents = filtered.slice(0, visibleEventCount)

  const selected = useMemo(
    () => events.find((event) => event.id === selectedId) ?? null,
    [events, selectedId]
  )
  const selectedSignal = useMemo(
    () => signals.find((signal) => signal.event.id === selectedId) ?? null,
    [signals, selectedId]
  )

  const indexSeries = useMemo<IndexSeries[]>(() => {
    if (!index || !day) return []
    const candidates: IndexSeries[] = [
      {
        key: "sh",
        label: "上证",
        color: chartColors.brand(),
        points: normalizeIndex(index.sh, day.date),
      },
      {
        key: "sz",
        label: "深成",
        color: chartColors.ma20(),
        points: normalizeIndex(index.sz, day.date),
      },
      {
        key: "cyb",
        label: "创业板",
        color: chartColors.ma10(),
        points: normalizeIndex(index.cyb, day.date),
      },
    ]
    return candidates.filter((series) => series.points.length > 0)
  }, [index, day])

  // 播放时钟：每 220ms 前进 N 个交易分钟，到收盘自动暂停。
  useEffect(() => {
    if (!playing) return
    const timer = window.setTimeout(() => {
      const next = Math.min(TRADING_MINUTES, cursor + speed)
      setCursor(next)
      if (next >= TRADING_MINUTES) setPlaying(false)
    }, 220)
    return () => window.clearTimeout(timer)
  }, [playing, speed, cursor])

  const togglePlay = useCallback(() => {
    if (playing) {
      setPlaying(false)
      return
    }
    if (cursor >= TRADING_MINUTES) setCursor(0)
    setPlaying(true)
  }, [playing, cursor])

  const restart = useCallback(() => {
    setCursor(0)
    setSelectedId(null)
    setPlaying(true)
  }, [])

  const step = useCallback((delta: number) => {
    setPlaying(false)
    setCursor((previous) => clamp(previous + delta, 0, TRADING_MINUTES))
  }, [])

  const jumpToEvent = useCallback((event: ReplayEvent) => {
    setCursor(eventTradingMinute(event))
    setSelectedId(event.id)
    setPlaying(false)
  }, [])

  const onSelectCell = useCallback((eventId: number, tradingMinute: number) => {
    setSelectedId(eventId)
    setCursor(tradingMinute)
    setPlaying(false)
  }, [])

  const onKeyDown = (event: React.KeyboardEvent<HTMLDivElement>) => {
    const target = event.target as HTMLElement
    if (
      target.closest(
        "input, textarea, select, [contenteditable='true'], [data-slot='select-trigger'], [role='listbox'], [role='option']"
      )
    ) {
      return
    }
    if (event.key === "ArrowLeft") {
      event.preventDefault()
      step(event.shiftKey ? -15 : -1)
    } else if (event.key === "ArrowRight") {
      event.preventDefault()
      step(event.shiftKey ? 15 : 1)
    } else if (event.key === " ") {
      event.preventDefault()
      togglePlay()
    } else if (event.key === "Escape") {
      setSelectedId(null)
    }
  }

  const onRefresh = () => {
    if (activeDate) refresh.mutate(activeDate)
  }

  if (error) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-3 px-6">
        <p className="text-secondary text-caption text-center">{error.message}</p>
        <div className="flex items-center gap-2">
          <Button
            variant="outline"
            size="xs"
            className="border-hairline"
            onClick={() => void refetch()}
          >
            重试
          </Button>
          <Button variant="ghost" size="xs" onClick={() => setSelectedDate(null)}>
            最近交易日
          </Button>
        </div>
      </div>
    )
  }

  if (isLoading || isSwitching) {
    return (
      <div className="flex h-full flex-col">
        <div className="bg-surface border-hairline flex shrink-0 items-center gap-2 border-b px-4 py-3">
          <Skeleton className="h-7 w-28" />
          <Skeleton className="h-5 w-14" />
        </div>
        <div className="flex flex-1 items-center justify-center">
          <div className="text-secondary text-caption flex items-center gap-2">
            <span className="bg-brand size-2 animate-pulse rounded-full" />
            加载回放数据...
          </div>
        </div>
      </div>
    )
  }

  return (
    <div className="flex h-full min-h-0 flex-col outline-none" tabIndex={0} onKeyDown={onKeyDown}>
      {/* 日期 + 状态 */}
      <div className="bg-surface border-hairline sticky top-0 z-10 flex shrink-0 items-center gap-3 border-b px-4 py-3">
        <Select
          value={activeDate}
          onValueChange={(value) => {
            setSelectedDate(value)
            setSelectedId(null)
            setPlaying(false)
            refresh.reset()
          }}
          disabled={dateOptions.length === 0}
        >
          <SelectTrigger size="sm" aria-label="交易日" className="text-caption h-7 min-w-28">
            <SelectValue placeholder="选择交易日" />
          </SelectTrigger>
          <SelectContent>
            {dateOptions.map((option) => (
              <SelectItem key={option.date} value={option.date} className="text-caption">
                {option.date}
                {option.status === "partial" ? " · 盘中" : ""}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        <span
          className={`text-caption rounded-sm px-1.5 py-0.5 ${
            day?.status === "complete" ? "bg-success/10 text-success" : "bg-warning/10 text-warning"
          }`}
        >
          {statusLabel(day?.status)}
        </span>
        <span className="text-caption text-tertiary font-number ml-auto">
          {day?.event_count ?? 0} 条
        </span>
        <Button
          variant="ghost"
          size="icon-xs"
          onClick={onRefresh}
          disabled={refresh.isPending || isFetching || !activeDate}
          aria-label="重新采集该交易日"
          title="重新采集该交易日"
        >
          <RefreshCw className={refresh.isPending || isFetching ? "animate-spin" : ""} />
        </Button>
      </div>

      {refresh.error && (
        <p className="border-hairline text-danger text-caption border-b px-4 py-2">
          {refresh.error.message}
        </p>
      )}

      {/* 回放控制 + 筛选 */}
      <div className="bg-surface border-hairline flex shrink-0 flex-col gap-3 border-b px-4 py-3">
        <div className="flex items-center gap-1.5">
          <Button
            variant="outline"
            size="icon-xs"
            className="border-hairline"
            onClick={togglePlay}
            aria-label={playing ? "暂停" : "播放"}
            title="播放/暂停 (空格)"
          >
            {playing ? <Pause /> : <Play />}
          </Button>
          <Button
            variant="ghost"
            size="icon-xs"
            onClick={restart}
            aria-label="从头回放"
            title="从头回放"
          >
            <RotateCcw />
          </Button>
          <span className="font-number text-body text-primary ml-1 min-w-10 font-semibold">
            {formatTradingMinute(cursor)}
          </span>
          {/* shadcn registry 没有 slider 组件，回放时间轴用原生 range，仅通过 token 着色。 */}
          <input
            type="range"
            min={0}
            max={TRADING_MINUTES}
            step={1}
            value={cursor}
            onChange={(event) => {
              setPlaying(false)
              setCursor(Number(event.target.value))
            }}
            aria-label="回放时间"
            className="h-1 min-w-0 flex-1 cursor-pointer"
            style={{ accentColor: "var(--color-brand)" }}
          />
          <div className="flex gap-0.5">
            {SPEEDS.map((item) => (
              <Button
                key={item.value}
                variant="outline"
                size="xs"
                onClick={() => setSpeed(item.value)}
                className={`text-caption font-number h-6 px-1.5 ${
                  speed === item.value
                    ? "border-brand bg-brand text-brand-foreground"
                    : "border-hairline text-secondary"
                }`}
              >
                {item.label}
              </Button>
            ))}
          </div>
        </div>

        <div className="flex flex-wrap items-center gap-1">
          {(
            [
              { value: "all", label: "全部" },
              { value: "good", label: "利好" },
              { value: "bad", label: "利空" },
            ] as const
          ).map((item) => (
            <Button
              key={item.value}
              variant="outline"
              size="xs"
              onClick={() => {
                setDirection(item.value)
                setSelectedId(null)
              }}
              className={`text-caption h-6 px-2 ${
                direction === item.value
                  ? item.value === "good"
                    ? "border-up bg-up/10 text-up"
                    : item.value === "bad"
                      ? "border-down bg-down/10 text-down"
                      : "border-brand bg-brand text-brand-foreground"
                  : "border-hairline text-secondary"
              }`}
            >
              {item.label}
            </Button>
          ))}
          <span className="text-tertiary text-caption">幅度≥</span>
          {PCT_OPTIONS.map((pct) => (
            <Button
              key={pct}
              variant="outline"
              size="xs"
              onClick={() => {
                setMinPct(pct)
                setSelectedId(null)
              }}
              className={`text-caption font-number h-6 px-1.5 ${
                minPct === pct
                  ? "border-brand bg-brand text-brand-foreground"
                  : "border-hairline text-secondary"
              }`}
            >
              {pct}%
            </Button>
          ))}
        </div>
      </div>

      <div className="flex-1 overflow-y-auto">
        {/* 指数 */}
        <section className="border-hairline border-b px-4 py-4">
          <div className="mb-1 flex items-baseline justify-between">
            <h3 className="text-caption text-primary font-medium">指数 · 日内</h3>
            <span className="text-tertiary text-caption">{day?.date}</span>
          </div>
          {indexSeries.length > 0 ? (
            <IndexMiniChart series={indexSeries} cursor={cursor} />
          ) : (
            <p className="text-tertiary text-caption py-3">
              该交易日没有保存指数分时（仅保存采集当天的数据）
            </p>
          )}
        </section>

        {/* 板块轮动 */}
        <section className="border-hairline border-b px-4 py-4">
          <div className="mb-2 flex items-baseline justify-between gap-2">
            <h3 className="text-caption text-primary font-medium">板块轮动热力图</h3>
            <div className="flex items-center gap-2">
              <span className="text-tertiary text-caption">
                5 分钟 / 格 · {rotation.rows.length} 个板块
              </span>
              {rotation.rows.length > HEATMAP_ROW_LIMIT && (
                <Button
                  variant="ghost"
                  size="xs"
                  className="text-caption h-5 px-1.5"
                  onClick={() => setShowAllPlates((value) => !value)}
                >
                  {showAllPlates ? "精简" : "全部"}
                </Button>
              )}
            </div>
          </div>
          <RotationHeatmap
            rotation={displayRotation}
            cursor={cursor}
            selectedId={selectedId}
            onSelect={onSelectCell}
          />
          {!showAllPlates && rotation.rows.length > HEATMAP_ROW_LIMIT && (
            <p className="text-tertiary text-caption mt-1">
              按累计异动幅度显示前 {HEATMAP_ROW_LIMIT} 个板块
            </p>
          )}
          <div className="text-tertiary text-caption mt-1 flex items-center gap-3">
            <span className="flex items-center gap-1">
              <span className="bg-up size-1.5 rounded-sm" /> 利好
            </span>
            <span className="flex items-center gap-1">
              <span className="bg-down size-1.5 rounded-sm" /> 利空
            </span>
            <span className="ml-auto">颜色越深 = 异动幅度越大</span>
          </div>
        </section>

        {/* Top 信号 */}
        <section className="border-hairline border-b px-4 py-4">
          <div className="mb-2 flex items-baseline justify-between gap-2">
            <h3 className="text-caption text-primary font-medium">Top 信号</h3>
            <span className="text-tertiary text-caption">幅度 · 广度 · 新鲜度 · 时段</span>
          </div>
          {topSignals.length > 0 ? (
            <div className="space-y-2">
              {topSignals.map((signal) => (
                <SignalRow
                  key={signal.event.id}
                  signal={signal}
                  active={selectedId === signal.event.id}
                  future={signal.minute > cursor}
                  onClick={() => jumpToEvent(signal.event)}
                />
              ))}
            </div>
          ) : (
            <p className="text-tertiary text-caption py-2">没有满足条件的信号</p>
          )}
        </section>

        {/* 选中事件 */}
        {selected && (
          <section className="border-hairline bg-canvas border-b px-4 py-4">
            <EventDetail
              event={selected}
              signal={selectedSignal}
              onClose={() => setSelectedId(null)}
            />
          </section>
        )}

        {/* 全部事件 */}
        <section className="px-4 py-4">
          <div className="mb-2 flex items-baseline justify-between gap-2">
            <h3 className="text-caption text-primary font-medium">全部事件</h3>
            <span className="text-tertiary text-caption">
              {filtered.length} / {events.length}
            </span>
          </div>
          {filtered.length > 0 ? (
            <div className="space-y-2">
              {visibleEvents.map((event) => {
                const good = eventDirection(event) === "good"
                const future = eventTradingMinute(event) > cursor
                return (
                  <Button
                    key={event.id}
                    variant="ghost"
                    aria-pressed={selectedId === event.id}
                    onClick={() => jumpToEvent(event)}
                    className={`bg-surface h-auto w-full cursor-pointer items-center justify-start gap-2 rounded-md border px-3 py-2 text-left font-normal transition-colors ${
                      future ? "opacity-50" : ""
                    } ${
                      selectedId === event.id ? "border-brand" : "border-hairline hover:bg-hover"
                    }`}
                  >
                    <span
                      className={`size-1.5 shrink-0 rounded-full ${good ? "bg-up" : "bg-down"}`}
                    />
                    <span className="text-secondary font-number text-caption w-10 shrink-0">
                      {formatTradingMinute(eventTradingMinute(event))}
                    </span>
                    <span className="text-caption text-primary min-w-0 flex-1 truncate font-medium">
                      {eventName(event)}
                    </span>
                    <span
                      className={`font-number text-caption w-14 shrink-0 text-right font-medium ${
                        good ? "text-up" : "text-down"
                      }`}
                    >
                      {formatPct(eventPcp(event))}
                    </span>
                  </Button>
                )
              })}
              {filtered.length > visibleEvents.length && (
                <Button
                  variant="outline"
                  size="xs"
                  className="border-hairline mt-2 w-full"
                  onClick={() => setVisibleEventCount((count) => count + EVENT_PAGE_SIZE)}
                >
                  显示更多（剩余 {filtered.length - visibleEvents.length} 条）
                </Button>
              )}
            </div>
          ) : (
            <div className="text-secondary text-caption flex flex-col items-center gap-2 py-6">
              <span>当前筛选条件下没有事件</span>
              <Button
                variant="outline"
                size="xs"
                className="border-hairline"
                onClick={() => {
                  setDirection("all")
                  setMinPct(0)
                }}
              >
                清除筛选
              </Button>
            </div>
          )}
        </section>

        <p className="text-tertiary text-caption px-4 pb-6">
          ← → 单步（Shift 加速） · 空格播放/暂停 · 点击热力图或信号跳转
        </p>
      </div>
    </div>
  )
}

function SignalRow({
  signal,
  active,
  future,
  onClick,
}: {
  signal: Signal
  active: boolean
  future: boolean
  onClick: () => void
}) {
  const good = signal.direction === "good"
  return (
    <Button
      variant="ghost"
      aria-pressed={active}
      onClick={onClick}
      className={`bg-surface h-auto w-full cursor-pointer flex-col items-stretch justify-start gap-0 rounded-md border px-3 py-2 text-left font-normal whitespace-normal transition-colors ${
        future ? "opacity-60" : ""
      } ${active ? "border-brand" : "border-hairline hover:bg-hover"}`}
    >
      <div className="flex items-center gap-2">
        <span className="font-number text-caption text-brand w-7 shrink-0 font-semibold">
          {signal.score}
        </span>
        <span className="text-secondary font-number text-caption w-10 shrink-0">
          {formatTradingMinute(signal.minute)}
        </span>
        <span className="text-caption text-primary min-w-0 flex-1 truncate font-semibold">
          {signal.name}
        </span>
        <span
          className={`text-caption shrink-0 rounded-sm px-1.5 py-0.5 ${
            good ? "bg-up/10 text-up" : "bg-down/10 text-down"
          }`}
        >
          {good ? "利好" : "利空"}
        </span>
        <span
          className={`font-number text-caption w-14 shrink-0 text-right ${
            good ? "text-up" : "text-down"
          }`}
        >
          {formatPct(signal.pcp)}
        </span>
      </div>
      <div className="mt-1 flex items-center gap-2">
        <div className="bg-hairline h-0.5 flex-1 overflow-hidden rounded-full">
          <div className="bg-brand h-full rounded-full" style={{ width: `${signal.score}%` }} />
        </div>
        <span className="text-tertiary text-caption shrink-0">
          广度 {Math.round(signal.breadth * 100)}% ·{" "}
          {signal.repeat === 0 ? "首次" : `第 ${signal.repeat + 1} 次`}
        </span>
      </div>
    </Button>
  )
}

function EventDetail({
  event,
  signal,
  onClose,
}: {
  event: ReplayEvent
  signal: Signal | null
  onClose: () => void
}) {
  const good = eventDirection(event) === "good"
  const stocks = event.related_stocks ?? []
  const sameDirection = stocks.filter((stock) => (good ? stock.pcp > 0 : stock.pcp < 0)).length

  return (
    <div>
      <div className="flex items-start gap-2">
        <div className="min-w-0 flex-1">
          <div className="flex min-w-0 items-center gap-2">
            <span
              className="text-body text-primary block min-w-0 flex-1 truncate font-semibold"
              title={eventName(event)}
            >
              {eventName(event)}
            </span>
            <span
              className={`text-caption shrink-0 rounded-sm px-1.5 py-0.5 ${
                good ? "bg-up/10 text-up" : "bg-down/10 text-down"
              }`}
            >
              {good ? "利好" : "利空"}
            </span>
            <span className="text-tertiary text-caption shrink-0">{eventKindLabel(event)}</span>
          </div>
          <div className="font-number text-caption text-secondary mt-1">
            {formatTradingMinute(eventTradingMinute(event))} · {formatPct(eventPcp(event))}
            {event.symbol ? ` · ${event.symbol}` : ""}
          </div>
        </div>
        <Button
          variant="ghost"
          size="icon-xs"
          aria-label="关闭详情"
          onClick={onClose}
          title="关闭详情"
        >
          <X />
        </Button>
      </div>

      <div className="mt-3 grid grid-cols-3 gap-3">
        <Metric label="评分" value={signal ? String(signal.score) : "—"} />
        <Metric
          label="同向广度"
          value={stocks.length > 0 ? `${sameDirection}/${stocks.length}` : "—"}
        />
        <Metric
          label="今日第"
          value={signal ? (signal.repeat === 0 ? "1 次" : `${signal.repeat + 1} 次`) : "—"}
        />
      </div>

      {stocks.length > 0 && (
        <div className="mt-3">
          <div className="text-tertiary text-caption mb-1">关联股票</div>
          <div className="flex flex-wrap gap-1">
            {stocks.slice(0, 12).map((stock) => (
              <span
                key={stock.symbol}
                className="bg-surface border-hairline text-caption flex items-center gap-1 rounded-sm border px-2 py-1"
              >
                <span className="text-primary">{stock.name}</span>
                <span className={`font-number ${stock.pcp >= 0 ? "text-up" : "text-down"}`}>
                  {formatPct(stock.pcp)}
                </span>
              </span>
            ))}
            {stocks.length > 12 && (
              <span className="text-tertiary text-caption self-center">等 {stocks.length} 只</span>
            )}
          </div>
        </div>
      )}
    </div>
  )
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="bg-surface border-hairline rounded-md border px-3 py-2">
      <div className="text-tertiary text-caption">{label}</div>
      <div className="font-number text-body text-primary font-semibold">{value}</div>
    </div>
  )
}
