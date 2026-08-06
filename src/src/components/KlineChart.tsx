import { chartColors } from "@/lib/chartColors"

import { useRef, useEffect, useCallback } from "react"
import {
  createChart,
  CandlestickSeries,
  HistogramSeries,
  LineSeries,
  CrosshairMode,
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

// Shanghai/Shenzhen morning+afternoon sessions (minutes since midnight)
const TRADING_SESSIONS = [
  [570, 690], // 9:30 - 11:30
  [780, 900], // 13:00 - 15:00
]

function isTradingTime(ts: number): boolean {
  const d = new Date(ts * 1000)
  const mins = d.getHours() * 60 + d.getMinutes()
  return TRADING_SESSIONS.some(([s, e]) => mins >= s && mins < e)
}

function formatTime(ts: number, period: string): string {
  const d = new Date(ts * 1000)
  const pad = (n: number) => String(n).padStart(2, "0")
  const ymd = `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`
  if (period === "minute") return `${ymd} ${pad(d.getHours())}:${pad(d.getMinutes())}`
  return ymd
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
  if (!TRADING_SESSIONS.some(([s, e]) => mins >= s && mins < e)) return ""
  const pad = (n: number) => String(n).padStart(2, "0")
  return `${pad(d.getHours())}:${pad(d.getMinutes())}`
}

/**
 * Build the full intraday time grid for the local trading day containing
 * `refTime`: every trading minute 9:30–11:29 and 13:00–14:59 (240 points).
 *
 * The grid is pre-populated as whitespace on the chart so the time axis is
 * drawn once for the whole session; stream data then fills values in place
 * instead of re-creating the chart / re-fitting the axis on every tick.
 *
 * Minute convention matches `isTradingTime` (start-of-bar, excludes 11:30 and
 * 15:00), so grid timestamps line up with the stream's `tick_at` values.
 */
function buildSessionGrid(refTime: number): number[] {
  const d = new Date(refTime * 1000)
  const y = d.getFullYear()
  const mo = d.getMonth()
  const da = d.getDate()
  // [startH, startM, endH, endM] — end is exclusive, matching isTradingTime
  const sessions: [number, number, number, number][] = [
    [9, 30, 11, 30],
    [13, 0, 15, 0],
  ]
  const grid: number[] = []
  for (const [sh, sm, eh, em] of sessions) {
    const end = Math.floor(new Date(y, mo, da, eh, em, 0).getTime() / 1000)
    let t = Math.floor(new Date(y, mo, da, sh, sm, 0).getTime() / 1000)
    while (t < end) {
      grid.push(t)
      t += 60
    }
  }
  return grid
}

function formatVolume(v: number): string {
  if (!v) return "-"
  if (v > 1e8) return (v / 1e8).toFixed(2) + "亿"
  if (v > 1e4) return (v / 1e4).toFixed(2) + "万"
  return v.toFixed(0)
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

export interface CrosshairInfo {
  time: string
  open: string
  high: string
  low: string
  close: string
  volume: string
  change: string
  changeNum: number
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
  // Mirrors of props read inside the long-lived crosshair handler / data fill,
  // kept in refs so chart creation does not depend on `data`/`preclose`.
  const dataRef = useRef<KlineCandle[]>(data)
  const precloseRef = useRef<number | undefined>(preclose)
  // Intraday full-session grid + bookkeeping for the pre-drawn axis.
  const gridRef = useRef<number[]>([])
  const gridDayRef = useRef<number>(-1)
  const axisPinnedRef = useRef(false)
  const precloseAppliedRef = useRef<number | null>(null)

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
        background: { type: "solid", color: "transparent" } as any,
        textColor: chartColors.tertiary(),
        fontSize: 12,
        panes: {
          enableResize: false,
          separatorColor: "rgba(255,255,255,0.08)",
          separatorHoverColor: "rgba(255,255,255,0.08)",
        },
      },
      grid: {
        vertLines: { color: "rgba(255,255,255,0.06)" },
        horzLines: { color: "rgba(255,255,255,0.06)" },
      },
      crosshair: { mode: CrosshairMode.Magnet },
      timeScale: {
        timeVisible: true,
        secondsVisible: false,
        borderColor: "rgba(255,255,255,0.06)",
        tickMarkFormatter: (time: Time) =>
          formatIntradayLabel(time as number, timeLabelMapRef.current),
      },
      localization: {
        // Crosshair vertical-line label on the time axis uses timeFormatter,
        // not tickMarkFormatter — keep them consistent for the custom time axis.
        timeFormatter: (time: Time) =>
          formatIntradayLabel(time as number, timeLabelMapRef.current),
      },
      rightPriceScale: {
        borderVisible: false,
        autoScale: false,
        scaleMargins: { top: 0.03, bottom: 0.03 },
      },
      leftPriceScale: {
        visible: true,
        borderVisible: false,
        autoScale: false,
        scaleMargins: { top: 0.03, bottom: 0.03 },
      },
    })

    priceSeriesRef.current = chart.addSeries(LineSeries, {
      color: chartColors.brand(),
      lineWidth: 2,
      priceScaleId: "left",
      crosshairMarkerVisible: true,
      crosshairMarkerRadius: 4,
      lastValueVisible: false,
      priceLineVisible: false,
      priceFormat: { type: "price", precision: 2, minMove: 0.01 },
    })

    avgPriceSeriesRef.current = chart.addSeries(LineSeries, {
      color: chartColors.warning(),
      lineWidth: 1,
      lineStyle: 2,
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
        formatter: (v: number) => `${v >= 0 ? "+" : ""}${v.toFixed(2)}%`,
        minMove: 0.01,
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

    chart.panes()[0].setHeight(320)
    chart.panes()[1].setHeight(100)

    chart.priceScale("right", 1).applyOptions({
      borderVisible: false,
      autoScale: true,
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
        volume: formatVolume(c.volume || 0),
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
        background: { type: "solid", color: "transparent" } as any,
        textColor: chartColors.tertiary(),
        fontSize: 12,
        panes: {
          enableResize: false,
          separatorColor: "rgba(255,255,255,0.08)",
          separatorHoverColor: "rgba(255,255,255,0.08)",
        },
      },
      grid: {
        vertLines: { color: "rgba(255,255,255,0.06)" },
        horzLines: { color: "rgba(255,255,255,0.06)" },
      },
      crosshair: { mode: CrosshairMode.Magnet },
      timeScale: {
        timeVisible: period === "minute",
        secondsVisible: false,
        borderColor: "rgba(255,255,255,0.06)",
        tickMarkFormatter: (time: Time) => formatTime(time as number, period),
      },
      localization: {
        // Crosshair vertical-line label on the time axis uses timeFormatter,
        // not tickMarkFormatter — keep them consistent with the custom axis.
        timeFormatter: (time: Time) => formatTime(time as number, period),
      },
      rightPriceScale: {
        borderColor: "rgba(255,255,255,0.06)",
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

    chart.panes()[0].setHeight(340)
    chart.panes()[1].setHeight(80)

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
        volume: formatVolume(c.volume || 0),
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

      // Volume, colored vs previous close; whitespace for future minutes.
      let prevClose: number | null = null
      volume.setData(
        grid.map((t) => {
          const c = candleMap.get(t)
          if (!c) return { time: t as Time, value: undefined as unknown as number }
          const isUp = prevClose !== null ? c.close >= prevClose : c.close >= (c.open ?? c.close)
          prevClose = c.close
          return {
            time: t as Time,
            value: c.volume || 0,
            color: isUp ? "rgba(255,92,92,0.3)" : "rgba(29,158,117,0.3)",
          }
        })
      )

      // pct anchor: two endpoints spanning the full grid (±涨跌停).
      const limitPct = getLimitPct(symbol)
      pctAnchor.setData([
        { time: grid[0] as Time, value: -limitPct },
        { time: grid[grid.length - 1] as Time, value: limitPct },
      ])

      // Pin the time axis to the full session once per grid build so later
      // stream ticks (which keep the same grid) don't shift it.
      if (!axisPinnedRef.current) {
        chart.timeScale().fitContent()
        axisPinnedRef.current = true
      }

      // Preclose price lines + fixed ±涨跌停 price range — applied once when
      // preclose becomes available (and re-applied if it changes).
      if (preclose && preclose > 0 && precloseAppliedRef.current !== preclose) {
        clearPriceLines()
        priceLinesRef.current.push(
          price.createPriceLine({
            price: preclose,
            color: "rgba(0,0,0,0.20)",
            lineStyle: 2,
            lineWidth: 1,
            axisLabelVisible: true,
          })
        )
        priceLinesRef.current.push(
          pctAnchor.createPriceLine({
            price: 0,
            color: "rgba(0,0,0,0.20)",
            lineStyle: 2,
            lineWidth: 1,
            axisLabelVisible: true,
          })
        )
        const prRange = preclose * (limitPct / 100)
        chart.priceScale("left", 0).setAutoScale(false)
        chart.priceScale("left", 0).setVisibleRange({
          from: preclose - prRange,
          to: preclose + prRange,
        })
        chart.priceScale("right", 0).setVisibleRange({
          from: -limitPct,
          to: limitPct,
        })
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
          color: c.close >= (c.open ?? c.close) ? "rgba(255,92,92,0.3)" : "rgba(29,158,117,0.3)",
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

      clearPriceLines()
      const visible = period === "day" ? 60 : period === "week" ? 60 : 36
      const to = deduped.length - 1
      const from = Math.max(0, to - visible + 1)
      chart.timeScale().setVisibleLogicalRange({ from, to })
    }
  }, [data, period, symbol, preclose, clearPriceLines])

  // Expose crosshair ref + listener for KlineDrawer (in effect to avoid modifying during render)
  useEffect(() => {
    ;(KlineChart as any).__crosshairRef = onCrosshairRef
    ;(KlineChart as any).setCrosshairListener = setCrosshairListener
    ;(KlineChart as any).chartRef = chartRef
  }, [onCrosshairRef, setCrosshairListener, chartRef])

  // Auto-restore external crosshair listener on mount (handles HMR remount)
  useEffect(() => {
    const pending = (KlineChart as any).__pendingCrosshairListener
    if (pending) {
      onCrosshairRef.current = pending
    }
  }, [])

  return <div ref={containerRef} className="w-full" style={{ height: 420 }} />
}
