import { chartColors, withAlpha } from "@/lib/chartColors"
import { formatCompactNumber } from "@/lib/format"
import { getKlineChartStatics, type CrosshairInfo } from "@/lib/klineChartStatics"

import { useRef, useEffect, useCallback, useState } from "react"
import {
  createChart,
  CandlestickSeries,
  HistogramSeries,
  LineSeries,
  LineStyle,
  CrosshairMode,
  ColorType,
  type IChartApi,
  type ISeriesApi,
  type Time,
  type IPriceLine,
} from "lightweight-charts"

interface KlineChartProps {
  data: KlineCandle[]
  period: string
  preclose?: number
  symbol: string
}

export interface KlineCandle {
  time: number
  open?: number | null
  close: number
  high?: number | null
  low?: number | null
  volume?: number | null
  change_rate?: number | null
}

// Shanghai/Shenzhen morning+afternoon sessions (minutes since midnight).
// 区间含 11:30 / 15:00 收盘 bar：选股宝把午盘收盘标在 11:30、全天收盘标在 15:00。
const TRADING_SESSIONS = [
  [570, 690], // 9:30 - 11:30
  [780, 900], // 13:00 - 15:00
]

// ---- 同花顺分时图复刻参数 ----
// 价格轴网格：昨收居中，向上/向下各 `PRICE_GRID_HALF_SEGMENTS` 格，
// 默认 2 格 → 全场 5 条横线（跌停、-半程、昨收、+半程、涨停）。
// 对应同花顺手机端“分时坐标格数 5 格”；改成 3 即为 7 格（8 条线）。
const PRICE_GRID_HALF_SEGMENTS = 2
// 分时主图字号（layout.fontSize 与 y 轴留白共用，单一来源）
const INTRADAY_FONT_SIZE = 12
// 分时 y 轴上下留白（CSS 像素；DPR 无关 —— LWC 内部按 devicePixelRatio 位图缩放坐标）。
// 几何下限 = 轴标签盒高的一半：盒高 = fontSize×17/12（fontSize + 上下各 2.5/12×fontSize
// 的 padding，见官方 price-axis-renderer-options-provider），12px 字号 → 盒高 17px、半高 8.5px。
// ceil(8.5)+1 = 10px：任何 pane 高度 / DPR 下标签盒都不顶、不裁画布边缘，且留白压到最小。
// 用像素而不用 scaleMargins 百分比：留白恒定，不随 pane 高度漂移（同花顺式）。
const INTRADAY_AXIS_MARGIN_PX = Math.ceil((INTRADAY_FONT_SIZE * 17) / 24) + 1
// 网格/昨收线的具体颜色取品牌 token（动态读取，见数据填充 effect），
// 样式固定为同花顺式虚线：普通网格浅色虚线、昨收黄色系虚线。
// 时间轴只标注三个时间（同花顺分时图横轴样式）：09:30 / 11:30 / 15:00。
// 全场分为四格，中间格界（10:30、14:00）只画刻度短线不标文字。
// 注意不能用 11:30 与 13:00 两个独立标签：两格在 242 根网格中相邻
// （120/121），标签会完全重叠 —— 故交界处合并为位置居中的单一标签。
const INTRADAY_TICK_LABELS = ["09:30", "11:30", "15:00"]
// 上述标签在 242 根会话网格中的 index（0/120.5/241）：
// 9:30=0、午休交界=120.5（11:30=120 与 13:00=121 的中点）、15:00=241。
// 全场分为四格：上午 09:30→10:30→11:30、下午 13:00→14:00→15:00；
// 中间格界（10:30=60、14:00=181）只画刻度短线、不标文字
// （INTRADAY_TICK_MARK_INDEX，见渲染层）。
// 标签由 HTML 在固定格点绘制（不依赖 LWC tick 选择），保证像素级对齐。
const INTRADAY_TICK_INDEX = [0, 120.5, 241]
const INTRADAY_TICK_MARK_INDEX = [60, 181]

function isTradingTime(ts: number): boolean {
  const d = new Date(ts * 1000)
  const mins = d.getHours() * 60 + d.getMinutes()
  return TRADING_SESSIONS.some(([s, e]) => mins >= s && mins <= e)
}

function formatTime(ts: number, period: string): string {
  const d = new Date(ts * 1000)
  const pad = (n: number) => String(n).padStart(2, "0")
  const ymd = `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`
  if (period === "minute") return `${ymd} ${pad(d.getHours())}:${pad(d.getMinutes())}`
  return ymd
}

/**
 * K 线（日/周/月）时间轴刻度标签 —— 同花顺复刻：
 * 日K/周K 在月初 bar 显示 `yy/mm`（如 25/01），月K 显示年份。
 * 相邻同月/同年只显示一次（`klineTickKeyRef` 过滤），
 * 十字光标浮窗仍走 `formatTime`（完整日期）。
 */
function formatKlineTick(ts: number, period: string): string {
  const d = new Date(ts * 1000)
  const pad = (n: number) => String(n).padStart(2, "0")
  if (period === "month") return `${d.getFullYear()}`
  return `${String(d.getFullYear() % 100).padStart(2, "0")}/${pad(d.getMonth() + 1)}`
}

function buildTimeLabels(candles: KlineCandle[]): Map<number, string> {
  const map = new Map<number, string>()
  let prevDay = -1
  candles.forEach((c) => {
    const d = new Date(c.time * 1000)
    const pad = (n: number) => String(n).padStart(2, "0")
    const day = Math.floor(c.time / 86400)
    const timeStr = `${pad(d.getHours())}:${pad(d.getMinutes())}`
    if (day !== prevDay) {
      // First tick of the day — include date for multi-day context
      map.set(c.time, `${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${timeStr}`)
      prevDay = day
    } else {
      map.set(c.time, timeStr)
    }
  })
  return map
}

/**
 * Format an intraday timestamp for the time axis / crosshair label.
 * Reused by both `tickMarkFormatter` (fixed axis ticks) and
 * `localization.timeFormatter` (the floating crosshair label), so the
 * crosshair label stays consistent with the custom time axis.
 */
function formatIntradayLabel(ts: number, labelMap: Map<number, string>): string {
  // Prefer label from map (has date prefix for multi-day context)
  const label = labelMap.get(ts)
  if (label) return label
  // Gap / whitespace point — only render if within trading hours
  const d = new Date(ts * 1000)
  const mins = d.getHours() * 60 + d.getMinutes()
  if (!TRADING_SESSIONS.some(([s, e]) => mins >= s && mins <= e)) return ""
  const pad = (n: number) => String(n).padStart(2, "0")
  return `${pad(d.getHours())}:${pad(d.getMinutes())}`
}

/**
 * Build the full intraday time grid for the local trading day containing
 * `refTime`: every trading minute 9:30–11:30 and 13:00–15:00 (242 points).
 *
 * The grid is pre-populated as whitespace on the chart so the time axis is
 * drawn once for the whole session; stream data then fills values in place
 * instead of re-creating the chart / re-fitting the axis on every tick.
 *
 * Minute convention matches `isTradingTime` — the session is inclusive of the
 * 11:30 / 15:00 close bars, since the upstream feed stamps the last bar of
 * each session at those times (not at 11:29 / 14:59).
 */
function buildSessionGrid(refTime: number): number[] {
  const d = new Date(refTime * 1000)
  const y = d.getFullYear()
  const mo = d.getMonth()
  const da = d.getDate()
  // [startH, startM, endH, endM] — end inclusive, matching isTradingTime
  const sessions: [number, number, number, number][] = [
    [9, 30, 11, 30],
    [13, 0, 15, 0],
  ]
  const grid: number[] = []
  for (const [sh, sm, eh, em] of sessions) {
    const end = Math.floor(new Date(y, mo, da, eh, em, 0).getTime() / 1000)
    let t = Math.floor(new Date(y, mo, da, sh, sm, 0).getTime() / 1000)
    while (t <= end) {
      grid.push(t)
      t += 60
    }
  }
  return grid
}

function getLimitPct(symbol: string): number {
  const raw = symbol.trim()
  // 去掉后缀 (.SZ/.sh/.BJ 等，大小写都要匹配)
  let code = raw.replace(/\.[A-Za-z]+$/, "")
  // 去掉前缀 (sz/sh/bj，大小写都要匹配)
  code = code.replace(/^(sz|sh|bj)/i, "")

  if (!/^\d{6}$/.test(code)) {
    // 兜底：格式不对时不应该静默返回10%
    return 10
  }

  if (/^(83|87|88|92)\d{4}$/.test(code)) return 30
  if (/^68\d{4}$/.test(code)) return 20
  if (/^30\d{4}$/.test(code)) return 20
  if (/^[46]0\d{4}$/.test(code)) return 10
  if (/^00\d{4}$/.test(code)) return 10
  return 10
}

/** Round to the exchange price tick (A 股 0.01 元，四舍五入）。 */
const roundTick = (p: number) => Math.round(p * 100) / 100

/** Actual limit-up/down prices at `preclose`（交易所把 ±限幅取整到 0.01 —
 *  例如昨收 15.16 → 涨停 16.68 = +10.03%，并非精确 ±10%）。
 *  分时网格顶部/底部线必须锚在取整后的涨跌停价上，否则封板时
 *  现价线与 ±10% 网格线相差 0.004 元，视觉上“没贴到 10% 上”。 */
function getLimitPrices(
  preclose: number,
  limitPct: number
): { up: number; down: number; upPct: number; downPct: number } {
  const up = roundTick(preclose * (1 + limitPct / 100))
  const down = roundTick(preclose * (1 - limitPct / 100))
  return {
    up,
    down,
    upPct: (up / preclose - 1) * 100,
    downPct: (down / preclose - 1) * 100,
  }
}

export default function KlineChart({ data, period, preclose, symbol }: KlineChartProps) {
  const containerRef = useRef<HTMLDivElement>(null)
  const chartRef = useRef<IChartApi | null>(null)
  const candleSeriesRef = useRef<ISeriesApi<"Candlestick"> | null>(null)
  const priceSeriesRef = useRef<ISeriesApi<"Line"> | null>(null)
  const avgPriceSeriesRef = useRef<ISeriesApi<"Line"> | null>(null)
  const pctAnchorSeriesRef = useRef<ISeriesApi<"Line"> | null>(null)
  const volumeSeriesRef = useRef<ISeriesApi<"Histogram"> | null>(null)
  const ma5SeriesRef = useRef<ISeriesApi<"Line"> | null>(null)
  const ma10SeriesRef = useRef<ISeriesApi<"Line"> | null>(null)
  const ma20SeriesRef = useRef<ISeriesApi<"Line"> | null>(null)
  const ma30SeriesRef = useRef<ISeriesApi<"Line"> | null>(null)
  const seriesRefs = useRef<ISeriesApi<"Line">[]>([])
  const priceLinesRef = useRef<IPriceLine[]>([])
  const timeLabelMapRef = useRef<Map<number, string>>(new Map())
  const onCrosshairRef = useRef<((info: CrosshairInfo) => void) | null>(null)
  // K 线时间轴刻度：记录上一个已显示的月份/年份，避免重复标签
  const klineTickKeyRef = useRef("")
  // K 线最新价虚线（同花顺右轴黄色虚线 + 价格标签）
  const lastPriceLineRef = useRef<IPriceLine | null>(null)
  // K 线主图左上角 MA 图例（同花顺 MA5/MA10/MA20/MA30）
  const [maLegend, setMaLegend] = useState<
    { label: string; color: string; value: string }[] | null
  >(null)
  // 分时横轴布局（HTML 标签定位用）：左/右价格轴宽 + 容器宽
  const [tickLayout, setTickLayout] = useState<{
    leftW: number
    rightW: number
    width: number
  } | null>(null)
  // 会话网格长度（渲染期读取 ref 被 React 19 禁止，故镜像为 state）
  const [gridLen, setGridLen] = useState(0)
  // Mirrors of props read inside the long-lived crosshair handler / data fill,
  // kept in refs so chart creation does not depend on `data`/`preclose`.
  const dataRef = useRef<KlineCandle[]>(data)
  const precloseRef = useRef<number | undefined>(preclose)
  // Intraday full-session grid + bookkeeping for the pre-drawn axis.
  const gridRef = useRef<number[]>([])
  const gridDayRef = useRef<number>(-1)
  const axisPinnedRef = useRef(false)
  const precloseAppliedRef = useRef<number | null>(null)
  // 分时量程锚点（数据填充 effect 写入，series 的 autoscaleInfoProvider 读取）：
  // 左轴 = 取整后的真实涨跌停价，右轴 = 对应涨跌停百分比。量程钉死 + 像素留白。
  const intradayRangeRef = useRef<{
    left: { minValue: number; maxValue: number }
    right: { minValue: number; maxValue: number }
  } | null>(null)

  const clearPriceLines = useCallback(() => {
    for (const pl of priceLinesRef.current) {
      try {
        candleSeriesRef.current?.removePriceLine(pl)
        priceSeriesRef.current?.removePriceLine(pl)
        pctAnchorSeriesRef.current?.removePriceLine(pl)
      } catch {
        /* ignore */
      }
    }
    priceLinesRef.current = []
  }, [])

  // Expose crosshair listener via imperative handle
  const setCrosshairListener = useCallback((fn: (info: CrosshairInfo) => void) => {
    onCrosshairRef.current = fn
  }, [])

  const initIntradayChart = useCallback(() => {
    if (!containerRef.current) return
    const el = containerRef.current

    const chart = createChart(el, {
      width: el.clientWidth,
      height: 420,
      layout: {
        attributionLogo: false,
        background: { type: ColorType.Solid, color: "transparent" },
        textColor: chartColors.tertiary(),
        fontSize: INTRADAY_FONT_SIZE,
        panes: {
          enableResize: false,
          separatorColor: withAlpha(chartColors.tertiary(), 0.16),
          separatorHoverColor: withAlpha(chartColors.tertiary(), 0.16),
        },
      },
      grid: {
        // 品牌配色：竖网格线用 tertiary 淡化（同花顺样式）
        vertLines: { color: withAlpha(chartColors.tertiary(), 0.16) },
        // 横向网格线关掉：同花顺分时横线固定在 -涨停/-半程/昨收/+半程/+涨停
        // 位置，由 5 条 priceLine 自绘（见数据填充 effect），LWC 自动刻度不保证对齐。
        horzLines: { color: "transparent" },
      },
      crosshair: { mode: CrosshairMode.Magnet },
      // 同花顺分时横轴固定整段交易时段，不支持拖动/缩放（K 线模式不受影响）
      handleScroll: false,
      handleScale: false,
      timeScale: {
        // 分时横轴完全由 HTML 标签接管（见渲染层 INTRADAY_TICK_INDEX），
        // 隐藏 LWC 时间轴，避免自动 tick 位置/刻度短线与同花顺不一致。
        visible: false,
        timeVisible: true,
        secondsVisible: false,
        borderColor: withAlpha(chartColors.tertiary(), 0.16),
        // 边缘 tick 标签内移对齐：9:30（最左）左对齐、15:00（最右）右对齐，
        // 避免标签被 canvas 边缘裁掉（如 15:00 只显示 “15:”）。
        fixLeftEdge: true,
        fixRightEdge: true,
      },
      localization: {
        // Crosshair vertical-line label on the time axis uses timeFormatter,
        // not tickMarkFormatter — keep them consistent for the custom time axis.
        timeFormatter: (time: Time) => formatIntradayLabel(time as number, timeLabelMapRef.current),
        // 隐藏 LWC 自动价格刻度（位置由 nice-tick 决定，不保证落在网格线上）：
        // 网格线位置的价格/百分比标签由 priceLine 的 axisLabel 提供。
        tickmarksPriceFormatter: (prices: readonly number[]) => prices.map(() => " "),
        tickmarksPercentageFormatter: (prices: readonly number[]) => prices.map(() => " "),
      },
      rightPriceScale: {
        borderVisible: false,
        // 像素 margins 只在 autoScale 重算路径生效，量程由 series 的 autoscaleInfoProvider
        // 钉死（见 pctAnchor series 创建处），必须 autoScale: true（handleScale 已禁，拖不坏）
        autoScale: true,
        // 百分比 scaleMargins 归零：留白全部交给 autoscaleInfoProvider 的像素 margins，
        // 恒定不随 pane 高度漂移（旧 0.08/0.05 → 300px pane = 24px/15px 且随高度变化；
        // 更早的 0.03 = 9px 时标签被官方钳位推到贴死画布边，视觉上像被裁）
        scaleMargins: { top: 0, bottom: 0 },
      },
      leftPriceScale: {
        visible: true,
        borderVisible: false,
        autoScale: true,
        scaleMargins: { top: 0, bottom: 0 },
      },
    })

    // 同花顺复刻：现价白线（1px），十字光标无圆点 marker（同花顺分时是纯十字线）
    priceSeriesRef.current = chart.addSeries(LineSeries, {
      color: chartColors.primary(),
      lineWidth: 1,
      priceScaleId: "left",
      crosshairMarkerVisible: false,
      lastValueVisible: false,
      priceLineVisible: false,
      priceFormat: { type: "price", precision: 2, minMove: 0.01 },
    })

    // 同花顺复刻：均价线黄色实线（VWAP）
    avgPriceSeriesRef.current = chart.addSeries(LineSeries, {
      color: chartColors.warning(),
      lineWidth: 1,
      priceScaleId: "left",
      crosshairMarkerVisible: false,
      lastValueVisible: false,
      priceLineVisible: false,
      priceFormat: { type: "price", precision: 2, minMove: 0.01 },
    })

    pctAnchorSeriesRef.current = chart.addSeries(LineSeries, {
      color: "transparent",
      priceScaleId: "right",
      lastValueVisible: false,
      priceLineVisible: false,
      crosshairMarkerVisible: false,
      priceFormat: {
        type: "custom",
        // 同花顺百分比轴：正数带 “+”，0 不带（同花顺昨收处显示 0.00%）
        formatter: (v: number) => `${v > 0 ? "+" : ""}${v.toFixed(2)}%`,
        minMove: 0.01,
      },
    })

    // 分时量程 + 像素留白：价格轴（左）/百分比轴（右）量程由 intradayRangeRef 钉死为
    // 取整后的 [跌停, 涨停]，上下各留 INTRADAY_AXIS_MARGIN_PX —— 恰好容纳一个轴标签盒，
    // 恒定不随 pane 高度/分辨率漂移（scaleMargins 百分比做不到）。官方机制：
    // autoscaleInfoProvider 返回的 margins 单位是像素，叠加在 scaleMargins 之上
    // （见 price-scale.ts 的 _topMarginPx = scaleMargins.top×height + marginAbove）。
    const INTRADAY_AUTOSCALE_MARGINS = {
      above: INTRADAY_AXIS_MARGIN_PX,
      below: INTRADAY_AXIS_MARGIN_PX,
    }
    priceSeriesRef.current.applyOptions({
      autoscaleInfoProvider: () => {
        const r = intradayRangeRef.current?.left
        return r ? { priceRange: r, margins: INTRADAY_AUTOSCALE_MARGINS } : null
      },
    })
    pctAnchorSeriesRef.current.applyOptions({
      autoscaleInfoProvider: () => {
        const r = intradayRangeRef.current?.right
        return r ? { priceRange: r, margins: INTRADAY_AUTOSCALE_MARGINS } : null
      },
    })

    chart.addPane(true)

    volumeSeriesRef.current = chart.addSeries(
      HistogramSeries,
      {
        priceScaleId: "right",
        priceFormat: {
          type: "custom",
          formatter: (v: number) => {
            if (v >= 1e8) return (v / 1e8).toFixed(1) + "亿"
            if (v >= 1e4) return (v / 1e4).toFixed(0) + "万"
            return v.toFixed(0)
          },
          minMove: 1,
        },
      },
      1
    )

    // 同花顺分时主图 : 成交量副图 ≈ 2.5 : 1
    chart.panes()[0].setHeight(300)
    chart.panes()[1].setHeight(120)

    chart.priceScale("right", 1).applyOptions({
      borderVisible: false,
      autoScale: true,
    })

    // 成交量副图基线（0 线），同花顺副图底部淡线（品牌 tertiary 淡化）
    volumeSeriesRef.current.createPriceLine({
      price: 0,
      color: withAlpha(chartColors.tertiary(), 0.35),
      lineStyle: LineStyle.Dashed,
      lineWidth: 1,
      axisLabelVisible: false,
    })

    chart.subscribeCrosshairMove((param) => {
      if (!param.point || param.time === undefined || param.seriesData.size === 0) return
      const ts = param.time as number
      const c = dataRef.current.find((d) => d.time === ts)
      if (!c || !onCrosshairRef.current) return
      const pc = precloseRef.current
      const chgRate = pc ? ((c.close - pc) / pc) * 100 : (c.change_rate ?? 0)
      onCrosshairRef.current({
        time: formatIntradayLabel(ts, timeLabelMapRef.current),
        open: c.open?.toFixed(2) || "-",
        high: c.high?.toFixed(2) || "-",
        low: c.low?.toFixed(2) || "-",
        close: c.close?.toFixed(2) || "-",
        volume: formatCompactNumber(c.volume || 0),
        change: chgRate >= 0 ? `+${chgRate.toFixed(2)}%` : `${chgRate.toFixed(2)}%`,
        changeNum: chgRate,
      })
    })

    chartRef.current = chart
  }, [])

  const initKlineChart = useCallback(() => {
    if (!containerRef.current) return
    const el = containerRef.current

    const chart = createChart(el, {
      width: el.clientWidth,
      height: 420,
      layout: {
        attributionLogo: false,
        background: { type: ColorType.Solid, color: "transparent" },
        textColor: chartColors.tertiary(),
        fontSize: 12,
        panes: {
          enableResize: false,
          separatorColor: withAlpha(chartColors.tertiary(), 0.16),
          separatorHoverColor: withAlpha(chartColors.tertiary(), 0.16),
        },
      },
      grid: {
        // 品牌配色：网格/边框用 tertiary 淡化（同花顺样式）
        vertLines: { color: withAlpha(chartColors.tertiary(), 0.12) },
        horzLines: { color: withAlpha(chartColors.tertiary(), 0.12) },
      },
      crosshair: { mode: CrosshairMode.Magnet },
      timeScale: {
        timeVisible: period === "minute",
        secondsVisible: false,
        borderColor: withAlpha(chartColors.tertiary(), 0.16),
        // 同花顺复刻：日K/周K 标 yy/mm，月K 标年份，同月只显示一次
        tickMarkFormatter: (time: Time) => {
          const key = formatKlineTick(time as number, period)
          if (key === klineTickKeyRef.current) return " "
          klineTickKeyRef.current = key
          return key
        },
      },
      localization: {
        // Crosshair vertical-line label on the time axis uses timeFormatter,
        // not tickMarkFormatter — keep them consistent with the custom axis.
        timeFormatter: (time: Time) => formatTime(time as number, period),
      },
      rightPriceScale: {
        borderColor: withAlpha(chartColors.tertiary(), 0.16),
        autoScale: true,
        scaleMargins: {
          top: 0.08,
          bottom: 0.08,
        },
      },
      leftPriceScale: {
        visible: false,
      },
    })

    candleSeriesRef.current = chart.addSeries(CandlestickSeries, {
      upColor: chartColors.up(),
      downColor: chartColors.down(),
      borderUpColor: chartColors.up(),
      borderDownColor: chartColors.down(),
      wickUpColor: chartColors.up(),
      wickDownColor: chartColors.down(),
      priceScaleId: "right",
      priceFormat: { type: "price", precision: 2, minMove: 0.01 },
    })

    // Moving averages
    const maConfig = [
      { ref: ma5SeriesRef, color: chartColors.ma5, label: "MA5" },
      { ref: ma10SeriesRef, color: chartColors.ma10, label: "MA10" },
      { ref: ma20SeriesRef, color: chartColors.ma20, label: "MA20" },
      { ref: ma30SeriesRef, color: chartColors.ma30, label: "MA30" },
    ] as const
    seriesRefs.current = maConfig.map(({ ref, color }) => {
      const series = chart.addSeries(LineSeries, {
        color,
        lineWidth: 1,
        priceScaleId: "right",
        lastValueVisible: false,
        priceLineVisible: false,
        crosshairMarkerVisible: false,
        priceFormat: { type: "price", precision: 2, minMove: 0.01 },
      })
      ref.current = series
      return series
    })

    chart.addPane(true)

    volumeSeriesRef.current = chart.addSeries(
      HistogramSeries,
      {
        priceScaleId: "right",
        priceFormat: {
          type: "custom",
          formatter: (v: number) => {
            if (v >= 1e8) return (v / 1e8).toFixed(1) + "亿"
            if (v >= 1e4) return (v / 1e4).toFixed(0) + "万"
            return v.toFixed(0)
          },
          minMove: 1,
        },
      },
      1
    )

    // 同花顺 K 线主图 : VOL 副图 ≈ 3.2 : 1
    chart.panes()[0].setHeight(320)
    chart.panes()[1].setHeight(100)

    chart.priceScale("right", 1).applyOptions({
      borderVisible: false,
      autoScale: true,
      scaleMargins: {
        top: 0.08,
        bottom: 0.08,
      },
    })

    chart.subscribeCrosshairMove((param) => {
      if (!param.point || param.time === undefined || param.seriesData.size === 0) return
      const ts = param.time as number
      const c = dataRef.current.find((d) => d.time === ts)
      if (!c || !onCrosshairRef.current) return
      const chgRate = c.change_rate ?? 0
      onCrosshairRef.current({
        time: formatTime(ts, period),
        open: c.open?.toFixed(2) || "-",
        high: c.high?.toFixed(2) || "-",
        low: c.low?.toFixed(2) || "-",
        close: c.close?.toFixed(2) || "-",
        volume: formatCompactNumber(c.volume || 0),
        change: chgRate >= 0 ? `+${chgRate.toFixed(2)}%` : `${chgRate.toFixed(2)}%`,
        changeNum: chgRate,
      })
    })

    chartRef.current = chart
  }, [period])

  // Chart lifecycle: create the chart + series once per (period, symbol).
  // Deliberately independent of `data` so streaming ticks don't re-create the
  // chart or re-fit the axis — the intraday time axis is pre-drawn and stream
  // data fills values in place (see the data-fill effect below).
  useEffect(() => {
    if (!containerRef.current) return

    // Reset per-(period, symbol) state for a fresh chart.
    clearPriceLines()
    chartRef.current?.remove()
    chartRef.current = null
    candleSeriesRef.current = null
    priceSeriesRef.current = null
    avgPriceSeriesRef.current = null
    pctAnchorSeriesRef.current = null
    volumeSeriesRef.current = null
    ma5SeriesRef.current = null
    ma10SeriesRef.current = null
    ma20SeriesRef.current = null
    ma30SeriesRef.current = null
    seriesRefs.current = []
    timeLabelMapRef.current = new Map<number, string>()
    gridRef.current = []
    gridDayRef.current = -1
    axisPinnedRef.current = false
    precloseAppliedRef.current = null
    klineTickKeyRef.current = ""
    lastPriceLineRef.current = null
    setMaLegend(null)

    if (period === "minute") {
      initIntradayChart()
    } else {
      initKlineChart()
    }

    return () => {
      clearPriceLines()
      chartRef.current?.remove()
      chartRef.current = null
      candleSeriesRef.current = null
      priceSeriesRef.current = null
      avgPriceSeriesRef.current = null
      pctAnchorSeriesRef.current = null
      volumeSeriesRef.current = null
      ma5SeriesRef.current = null
      ma10SeriesRef.current = null
      ma20SeriesRef.current = null
      ma30SeriesRef.current = null
      seriesRefs.current = []
    }
  }, [period, symbol, initIntradayChart, initKlineChart, clearPriceLines])

  // Keep prop mirrors in sync for the long-lived crosshair handler (the chart
  // is created once and must not depend on `data`/`preclose`).
  useEffect(() => {
    dataRef.current = data
    precloseRef.current = preclose
  }, [data, preclose])

  // 分时横轴 HTML 标签定位：监听容器尺寸变化（图表重建后重新绑定），
  // 同步 LWC 宽度并读取左右价格轴宽，供渲染层计算 6 个标签的 x 坐标。
  useEffect(() => {
    const el = containerRef.current
    if (!el) return
    const updateLayout = () => {
      const chart = chartRef.current
      if (!chart) return
      setTickLayout({
        leftW: chart.priceScale("left", 0).width(),
        rightW: chart.priceScale("right", 0).width(),
        width: el.clientWidth,
      })
    }
    const ro = new ResizeObserver(() => {
      chartRef.current?.applyOptions({ width: el.clientWidth })
      updateLayout()
    })
    ro.observe(el)
    updateLayout()
    return () => ro.disconnect()
  }, [period, symbol])

  // Data fill: intraday pre-draws the full-session grid and fills values as the
  // stream arrives; kline applies a full snapshot each tick. Never re-creates
  // the chart, so the time axis stays stable across stream ticks.
  useEffect(() => {
    const chart = chartRef.current
    if (!chart) return
    if (data.length === 0) return

    // Deduplicate by time — lightweight-charts requires strictly ascending timestamps
    const deduped = (() => {
      const seen = new Map<number, KlineCandle>()
      for (const c of data) seen.set(c.time, c)
      return [...seen.values()].sort((a, b) => a.time - b.time)
    })()
    if (deduped.length === 0) return

    if (period === "minute") {
      // ---- Intraday: pre-drawn full-session axis, fill on stream ----
      const price = priceSeriesRef.current
      const avgPrice = avgPriceSeriesRef.current
      const volume = volumeSeriesRef.current
      const pctAnchor = pctAnchorSeriesRef.current
      if (!price || !avgPrice || !volume || !pctAnchor) return

      // Only trading-hour candles, single day = the last candle's day.
      const trading = deduped.filter((c) => isTradingTime(c.time))
      if (trading.length === 0) return
      const lastDay = Math.floor(trading[trading.length - 1].time / 86400)
      const dayCandles = trading.filter((c) => Math.floor(c.time / 86400) === lastDay)
      const candleMap = new Map<number, KlineCandle>(dayCandles.map((c) => [c.time, c]))

      // (Re)build the full-session grid when the trading day changes / first time.
      if (gridDayRef.current !== lastDay) {
        gridRef.current = buildSessionGrid(dayCandles[dayCandles.length - 1].time)
        gridDayRef.current = lastDay
        setGridLen(gridRef.current.length)
        timeLabelMapRef.current = buildTimeLabels(
          gridRef.current.map((t) => ({ time: t, close: 0 }))
        )
        axisPinnedRef.current = false
        precloseAppliedRef.current = null
      }
      const grid = gridRef.current
      if (grid.length === 0) return

      // Price line over the full grid — whitespace where no candle has arrived.
      price.setData(
        grid.map((t) => {
          const c = candleMap.get(t)
          return { time: t as Time, value: c ? c.close : (undefined as unknown as number) }
        })
      )

      // VWAP (cumulative), reset at day boundaries; whitespace for future minutes.
      let cumValue = 0
      let cumVolume = 0
      let prevDay = -1
      avgPrice.setData(
        grid.map((t) => {
          const c = candleMap.get(t)
          if (!c) return { time: t as Time, value: undefined as unknown as number }
          const currDay = Math.floor(c.time / 86400)
          if (currDay !== prevDay) {
            cumValue = 0
            cumVolume = 0
            prevDay = currDay
          }
          cumValue += c.close * (c.volume || 0)
          cumVolume += c.volume || 0
          return {
            time: t as Time,
            value: cumVolume > 0 ? cumValue / cumVolume : c.close,
          }
        })
      )

      // Volume, colored vs previous close (同花顺：红涨绿跌，对比昨收)；
      // whitespace for future minutes.
      const pc = precloseRef.current
      volume.setData(
        grid.map((t) => {
          const c = candleMap.get(t)
          if (!c) return { time: t as Time, value: undefined as unknown as number }
          const isUp = pc ? c.close >= pc : c.close >= (c.open ?? c.close)
          return {
            time: t as Time,
            value: c.volume || 0,
            // 同花顺量柱比 K 线副图更实（不透明度更高）
            color: isUp ? withAlpha(chartColors.up(), 0.55) : withAlpha(chartColors.down(), 0.55),
          }
        })
      )

      // pct anchor: two endpoints spanning the full grid (±涨跌停).
      // 有昨收时用取整后的真实涨跌停幅度（如 +10.03%），与价格轴严格对齐。
      {
        const limitPct = getLimitPct(symbol)
        const pc = precloseRef.current
        const lim = pc && pc > 0 ? getLimitPrices(pc, limitPct) : null
        const upPct = lim ? lim.upPct : limitPct
        const downPct = lim ? lim.downPct : -limitPct
        pctAnchor.setData([
          { time: grid[0] as Time, value: downPct },
          { time: grid[grid.length - 1] as Time, value: upPct },
        ])
      }

      // Pin the time axis to the full session once per grid build so later
      // stream ticks (which keep the same grid) don't shift it.
      if (!axisPinnedRef.current) {
        chart.timeScale().fitContent()
        axisPinnedRef.current = true
      }

      // 同花顺复刻网格：以昨收为中线、涨跌停对称等分（默认上下各 2 格 → 5 条横线）。
      // 顶部/底部线锚在取整后的真实涨跌停价（如 16.68 = +10.03%），封板时
      // 现价线恰好落在顶线上，而不是悬在未取整的 ±10% 下方/上方。
      // 每格位置同时画两条 priceLine：左轴（价格 series）显示价格标签，
      // 右轴（百分比 series）显示百分比标签；昨收（0%）用品牌 warning 色虚线。
      // 网格线 priceLine 挂在 series 上，宽度贯穿整个主图 pane，
      // 而 LWC 自动刻度文本已被 `tickmarksPriceFormatter` 隐藏。
      if (preclose && preclose > 0 && precloseAppliedRef.current !== preclose) {
        clearPriceLines()
        const limitPct = getLimitPct(symbol)
        const lim = getLimitPrices(preclose, limitPct)
        const { upPct, downPct } = lim
        // 品牌配色：网格线 = tertiary 淡化，昨收线 = warning（与均价线同色系）
        const gridColor = withAlpha(chartColors.tertiary(), 0.35)
        const preCloseColor = chartColors.warning()
        const n = PRICE_GRID_HALF_SEGMENTS * 2 // 全场格数（默认 4 → 5 条线）
        for (let i = 0; i <= n; i++) {
          const pct = downPct + ((upPct - downPct) * i) / n
          const isPreClose = i === n / 2
          const color = isPreClose ? preCloseColor : gridColor
          priceLinesRef.current.push(
            price.createPriceLine({
              price: preclose + (pct / 100) * preclose,
              color,
              lineStyle: LineStyle.Dashed,
              lineWidth: 1,
              axisLabelVisible: true,
            })
          )
          priceLinesRef.current.push(
            pctAnchor.createPriceLine({
              price: pct,
              color,
              lineStyle: LineStyle.Dashed,
              lineWidth: 1,
              axisLabelVisible: true,
            })
          )
        }
        // 量程钉死改走 series 的 autoscaleInfoProvider（见 series 创建处）：
        // - 像素 margins 只在 autoScale 重算路径生效，setVisibleRange 的 custom range
        //   会绕过该路径（price-scale.ts: isCustomPriceRange && !isAutoScale → return），故弃用
        // - 同一次 effect 里上方的 setData 已让 LWC 排队重算，provider 下一帧即读到本 ref
        intradayRangeRef.current = {
          left: { minValue: lim.down, maxValue: lim.up },
          right: { minValue: downPct, maxValue: upPct },
        }
        precloseAppliedRef.current = preclose
      }
    } else {
      // ---- Kline (day/week/month): full snapshot each tick ----
      const candle = candleSeriesRef.current
      const volume = volumeSeriesRef.current
      if (!candle || !volume) return

      candle.setData(
        deduped.map((c) => ({
          time: c.time as Time,
          open: c.open ?? c.close,
          high: c.high ?? c.close,
          low: c.low ?? c.close,
          close: c.close,
        }))
      )
      volume.setData(
        deduped.map((c) => ({
          time: c.time as Time,
          value: c.volume || 0,
          color:
            c.close >= (c.open ?? c.close)
              ? withAlpha(chartColors.up(), 0.3)
              : withAlpha(chartColors.down(), 0.3),
        }))
      )

      // Moving averages (MA5, MA10, MA20, MA30)
      const closes = deduped.map((c) => c.close)
      const ma = (p: number, i: number) => {
        if (i < p - 1) return null
        let sum = 0
        for (let j = i - p + 1; j <= i; j++) sum += closes[j]
        return sum / p
      }
      ;[5, 10, 20, 30].forEach((p, idx) => {
        const series = seriesRefs.current[idx]
        if (!series) return
        const maData = deduped
          .map((c, i) => {
            const v = ma(p, i)
            return v !== null ? { time: c.time as Time, value: v } : null
          })
          .filter(Boolean) as { time: Time; value: number }[]
        if (maData.length > 0) series.setData(maData)
      })

      // 同花顺最新价虚线：右轴黄色虚线 + 价格标签，跟随最新 bar 更新
      const lastCandle = deduped[deduped.length - 1]
      if (lastCandle) {
        if (!lastPriceLineRef.current) {
          lastPriceLineRef.current = candle.createPriceLine({
            price: lastCandle.close,
            color: chartColors.warning(),
            lineStyle: LineStyle.Dashed,
            lineWidth: 1,
            axisLabelVisible: true,
          })
        } else {
          lastPriceLineRef.current.applyOptions({ price: lastCandle.close })
        }
      }

      // 同花顺主图左上角 MA 图例：MA5/MA10/MA20/MA30 最新值（品牌 MA 调色板）
      const legendColors = [chartColors.ma5, chartColors.ma10, chartColors.ma20, chartColors.ma30]
      const n = closes.length
      setMaLegend(
        [5, 10, 20, 30].map((p, i) => {
          let sum = 0
          for (let j = Math.max(0, n - p); j < n; j++) sum += closes[j]
          const v = n >= p ? sum / p : null
          return {
            label: `MA${p}`,
            color: legendColors[i],
            value: v !== null ? v.toFixed(2) : "-",
          }
        })
      )

      clearPriceLines()
      // 同花顺默认可见密度：日K≈半年、周K≈2~3 年、月K≈5 年
      const visible = period === "day" ? 120 : period === "week" ? 120 : 60
      const to = deduped.length - 1
      const from = Math.max(0, to - visible + 1)
      chart.timeScale().setVisibleLogicalRange({ from, to })
    }
  }, [data, period, symbol, preclose, clearPriceLines])

  // Expose crosshair ref + listener for KlineDrawer (in effect to avoid modifying during render)
  useEffect(() => {
    const statics = getKlineChartStatics()
    statics.__crosshairRef = onCrosshairRef
    statics.setCrosshairListener = setCrosshairListener
    statics.chartRef = chartRef
  }, [onCrosshairRef, setCrosshairListener, chartRef])

  // Auto-restore external crosshair listener on mount (handles HMR remount)
  useEffect(() => {
    const pending = getKlineChartStatics().__pendingCrosshairListener
    if (pending) {
      onCrosshairRef.current = pending
    }
  }, [])

  return (
    <div className="relative w-full" style={{ height: period === "minute" ? 436 : 420 }}>
      {/* K 线主图左上角 MA 图例（同花顺样式：整行与均线同色；分时图不显示） */}
      {period !== "minute" && maLegend && (
        <div className="font-number text-caption absolute top-1.5 left-2 z-10 flex flex-wrap items-center gap-x-3 gap-y-0.5">
          {maLegend.map((m) => (
            <span key={m.label} className="flex items-center gap-1" style={{ color: m.color }}>
              {m.label}: <span className="font-bold">{m.value}</span>
            </span>
          ))}
        </div>
      )}
      {/* K 线 VOL 副图左上角标注（同花顺样式） */}
      {period !== "minute" && (
        <span
          className="font-number text-secondary text-caption absolute left-1 z-10"
          style={{ top: 322 }}
        >
          VOL
        </span>
      )}
      {/* 分时横轴：同花顺四格刻度 + 3 个整点标签，按 242 根网格固定格点
          像素级定位（标签 index 0/120.5/241 → 9:30/11:30/15:00；
          四格中间格界 60/181 → 10:30/14:00 只画刻度短线）。
          首尾贴边（左对齐/右对齐），中间居中；LWC 时间轴已隐藏。 */}
      {period === "minute" && tickLayout && gridLen > 1 && (
        <div className="pointer-events-none absolute bottom-0 left-0 z-10 h-4 w-full">
          {/* 四格中间格界的刻度短线（10:30 / 14:00，无文字） */}
          {INTRADAY_TICK_MARK_INDEX.map((idx) => {
            const plotW = Math.max(1, tickLayout.width - tickLayout.leftW - tickLayout.rightW)
            const x = tickLayout.leftW + ((idx + 0.5) / gridLen) * plotW
            return (
              <span
                key={`mark-${idx}`}
                className="bg-tertiary/50 absolute bottom-0 h-1.5 w-px"
                style={{ left: x }}
              />
            )
          })}
          {INTRADAY_TICK_INDEX.map((idx, k) => {
            // bar 中心 x：fitContent 后 242 根等宽铺满绘图区，
            // 第 idx 根中心 = ((idx + 0.5) / gridLen) * plotW
            const plotW = Math.max(1, tickLayout.width - tickLayout.leftW - tickLayout.rightW)
            const x = tickLayout.leftW + ((idx + 0.5) / gridLen) * plotW
            return (
              <span
                key={idx}
                className="font-number text-secondary text-caption absolute top-0"
                style={{
                  left: x,
                  transform:
                    k === 0
                      ? "none"
                      : k === INTRADAY_TICK_INDEX.length - 1
                        ? "translateX(-100%)"
                        : "translateX(-50%)",
                }}
              >
                {INTRADAY_TICK_LABELS[k]}
              </span>
            )
          })}
        </div>
      )}
      <div ref={containerRef} className="w-full" style={{ height: 420 }} />
    </div>
  )
}
