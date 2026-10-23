import type { ReplayEvent, ReplayIndexTick } from "@/hooks/query/useReplay"

const AM_OPEN = 9 * 60 + 30
const AM_CLOSE = 11 * 60 + 30
const PM_OPEN = 13 * 60
const PM_CLOSE = 15 * 60
const BEIJING_OFFSET = 8 * 3600

/** 09:30–11:30 + 13:00–15:00，共 240 个交易分钟。 */
export const TRADING_MINUTES = PM_CLOSE - AM_OPEN - (PM_OPEN - AM_CLOSE)

export const BUCKET_SIZE = 5
/** 240 个交易分钟按 5 分钟分桶，最后一桶覆盖 14:55–15:00。 */
export const BUCKET_COUNT = Math.ceil(TRADING_MINUTES / BUCKET_SIZE)

export type Direction = "good" | "bad"
export type DirectionFilter = "all" | Direction

export interface RotationCell {
  eventId: number
  pcp: number
  direction: 1 | 2
  count: number
  minute: number
}

export interface RotationRow {
  name: string
  firstBucket: number
  total: number
  cells: (RotationCell | null)[]
}

export interface RotationData {
  rows: RotationRow[]
  maxAbs: number
}

export interface Signal {
  event: ReplayEvent
  name: string
  direction: Direction
  minute: number
  pcp: number
  score: number
  breadth: number
  repeat: number
  amplitude: number
}

export interface IndexPoint {
  minute: number
  pct: number
}

export interface ReplayFilters {
  direction: DirectionFilter
  /** 百分数，例如 2 表示 2%。 */
  minPct: number
}

/** Unix 秒 → 北京时间当天第几分钟。 */
export function beijingMinute(timestamp: number): number {
  const seconds = (((timestamp + BEIJING_OFFSET) % 86400) + 86400) % 86400
  return Math.floor(seconds / 60)
}

export function beijingDate(timestamp: number): string {
  return new Date((timestamp + BEIJING_OFFSET) * 1000).toISOString().slice(0, 10)
}

/** 绝对分钟 → 去掉午休后的交易分钟（0–240）。 */
export function toTradingMinute(minute: number): number {
  if (minute <= AM_CLOSE) return clamp(minute - AM_OPEN, 0, 120)
  if (minute >= PM_OPEN) return clamp(120 + minute - PM_OPEN, 120, TRADING_MINUTES)
  return 120
}

export function fromTradingMinute(tradingMinute: number): number {
  if (tradingMinute <= 120) return AM_OPEN + tradingMinute
  return PM_OPEN + (tradingMinute - 120)
}

export function formatMinute(minute: number): string {
  const normalized = ((minute % 1440) + 1440) % 1440
  return `${String(Math.floor(normalized / 60)).padStart(2, "0")}:${String(
    normalized % 60
  ).padStart(2, "0")}`
}

export function formatTradingMinute(tradingMinute: number): string {
  return formatMinute(fromTradingMinute(tradingMinute))
}

export function formatPct(value: number, digits = 2): string {
  const sign = value >= 0 ? "+" : "-"
  return `${sign}${(Math.abs(value) * 100).toFixed(digits)}%`
}

export function eventName(event: ReplayEvent): string {
  const target = event.target?.trim() ?? ""
  // 上游的 target 对异动事件通常是数字 id，不适合当标题；只有非纯数字才兜底。
  const fallback = target && !/^\d+$/.test(target) ? target : ""
  return event.plate_name || event.stock_name || event.symbol || fallback
}

export function eventPcp(event: ReplayEvent): number {
  return event.pcp ?? 0
}

export function eventTradingMinute(event: ReplayEvent): number {
  return toTradingMinute(beijingMinute(event.event_timestamp))
}

export function eventDirection(event: ReplayEvent): Direction {
  return event.good_or_bad === 1 ? "good" : "bad"
}

export function isPlateEvent(event: ReplayEvent): boolean {
  return Boolean(event.plate_name)
}

/** 当前上游的 11000/11001 都带板块数据，按载荷而不是内部 type code 展示。 */
export function eventKindLabel(event: ReplayEvent): string {
  if (event.plate_name) return "板块异动"
  if (event.stock_name) return "个股异动"
  return "异动"
}

export function filterReplayEvents(events: ReplayEvent[], filters: ReplayFilters): ReplayEvent[] {
  return events.filter((event) => {
    if (!eventName(event)) return false
    if (filters.direction !== "all" && eventDirection(event) !== filters.direction) return false
    return Math.abs(eventPcp(event)) * 100 >= filters.minPct
  })
}

/**
 * 5 分钟 × 板块的轮动矩阵。
 *
 * 同一板块在同一个桶内只保留幅度最大的一条事件，避免重复异动把颜色刷爆；
 * `count` 记录该桶内实际事件数，用来说明“持续异动”还是“一次性脉冲”。
 */
export function buildRotation(events: ReplayEvent[]): RotationData {
  const rows = new Map<
    string,
    { firstBucket: number; total: number; cells: (RotationCell | null)[] }
  >()

  for (const event of events) {
    if (!isPlateEvent(event)) continue
    const name = eventName(event)
    if (!name || event.pcp == null) continue
    const minute = eventTradingMinute(event)
    const bucket = clamp(Math.floor(minute / BUCKET_SIZE), 0, BUCKET_COUNT - 1)
    const pcp = Math.abs(event.pcp)
    const row = rows.get(name) ?? {
      firstBucket: bucket,
      total: 0,
      cells: Array.from({ length: BUCKET_COUNT }, () => null),
    }
    row.firstBucket = Math.min(row.firstBucket, bucket)
    row.total += pcp
    const existing = row.cells[bucket]
    if (!existing) {
      row.cells[bucket] = {
        eventId: event.id,
        pcp: event.pcp,
        direction: event.good_or_bad === 1 ? 1 : 2,
        count: 1,
        minute,
      }
    } else {
      existing.count += 1
      if (pcp > Math.abs(existing.pcp)) {
        existing.pcp = event.pcp
        existing.direction = event.good_or_bad === 1 ? 1 : 2
        existing.minute = minute
        existing.eventId = event.id
      }
    }
    rows.set(name, row)
  }

  let maxAbs = 0.005
  const list: RotationRow[] = [...rows.entries()].map(([name, row]) => {
    for (const cell of row.cells) {
      if (cell) maxAbs = Math.max(maxAbs, Math.abs(cell.pcp))
    }
    return { name, ...row }
  })
  list.sort((a, b) => a.firstBucket - b.firstBucket || b.total - a.total)
  return { rows: list, maxAbs }
}

/**
 * 事件质量评分：幅度、广度、新鲜度、时段共同决定排序。
 *
 * 权重只是可解释的起点，后续可以根据“事件后收益”做校准，不要把它当成
 * 已验证的 alpha 模型。
 */
export function buildSignals(events: ReplayEvent[]): Signal[] {
  const sorted = [...events].sort((a, b) => a.event_timestamp - b.event_timestamp)
  const repeats = new Map<string, number>()
  const signals: Signal[] = []

  for (const event of sorted) {
    const name = eventName(event)
    if (!name || event.pcp == null) continue
    const repeat = repeats.get(name) ?? 0
    repeats.set(name, repeat + 1)

    const direction = eventDirection(event)
    const stocks = event.related_stocks ?? []
    const sameDirection = stocks.filter((stock) =>
      direction === "good" ? stock.pcp > 0 : stock.pcp < 0
    ).length
    const breadth = stocks.length > 0 ? sameDirection / stocks.length : 0.5
    const amplitude = Math.min(1, Math.abs(event.pcp) / 0.05)
    const novelty = repeat === 0 ? 1 : 1 / (1 + repeat * 0.6)
    const minute = eventTradingMinute(event)
    const timeWeight = 1 - (minute / TRADING_MINUTES) * 0.35
    const score = 100 * (0.35 * amplitude + 0.25 * breadth + 0.2 * novelty + 0.2 * timeWeight)

    signals.push({
      event,
      name,
      direction,
      minute,
      pcp: event.pcp,
      score: Math.round(score),
      breadth,
      repeat,
      amplitude,
    })
  }

  return signals.sort((a, b) => b.score - a.score || a.minute - b.minute)
}

/** 指数分时归一化为“相对开盘涨跌幅”，并只保留所选交易日。 */
export function normalizeIndex(ticks: ReplayIndexTick[], date: string): IndexPoint[] {
  const source = ticks.filter((tick) => beijingDate(tick.tick_at) === date)
  if (source.length === 0) return []
  const base = source[0].close_px
  if (!base) return []

  const byMinute = new Map<number, number>()
  for (const tick of source) {
    const minute = toTradingMinute(beijingMinute(tick.tick_at))
    byMinute.set(minute, (tick.close_px / base - 1) * 100)
  }
  return [...byMinute.entries()]
    .map(([minute, pct]) => ({ minute, pct }))
    .sort((a, b) => a.minute - b.minute)
}

export function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value))
}
