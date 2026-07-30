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
  candles.forEach((c) => {
    const d = new Date(c.time * 1000)
    const pad = (n: number) => String(n).padStart(2, "0")
    map.set(c.time, `${pad(d.getHours())}:${pad(d.getMinutes())}`)
  })
  return map
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
        textColor: "#888780",
        fontSize: 10,
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
        tickMarkFormatter: (time: Time) => timeLabelMapRef.current.get(time as number) || "",
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
      color: "#7F77DD",
      lineWidth: 2,
      priceScaleId: "left",
      crosshairMarkerVisible: true,
      crosshairMarkerRadius: 4,
      lastValueVisible: false,
      priceLineVisible: false,
      priceFormat: { type: "price", precision: 2, minMove: 0.01 },
    })

    avgPriceSeriesRef.current = chart.addSeries(LineSeries, {
      color: "#f5a623",
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
      const c = data.find((d) => d.time === ts)
      if (!c || !onCrosshairRef.current) return
      const chgRate = preclose ? ((c.close - preclose) / preclose) * 100 : (c.change_rate ?? 0)
      const label = timeLabelMapRef.current.get(ts) || ""
      onCrosshairRef.current({
        time: label,
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
  }, [data, preclose])

  const initKlineChart = useCallback(() => {
    if (!containerRef.current) return
    const el = containerRef.current

    const chart = createChart(el, {
      width: el.clientWidth,
      height: 420,
      layout: {
        attributionLogo: false,
        background: { type: "solid", color: "transparent" } as any,
        textColor: "#888780",
        fontSize: 10,
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
      upColor: "#FF5C5C",
      downColor: "#1D9E75",
      borderUpColor: "#FF5C5C",
      borderDownColor: "#1D9E75",
      wickUpColor: "#FF5C5C",
      wickDownColor: "#1D9E75",
      priceScaleId: "right",
      priceFormat: { type: "price", precision: 2, minMove: 0.01 },
    })

    // Moving averages
    const maConfig = [
      { ref: ma5SeriesRef, color: "#F5A623", label: "MA5" },
      { ref: ma10SeriesRef, color: "#4A90D9", label: "MA10" },
      { ref: ma20SeriesRef, color: "#D0021B", label: "MA20" },
      { ref: ma30SeriesRef, color: "#7ED321", label: "MA30" },
    ] as const
    seriesRefs.current = maConfig.map(({ ref, color, label }) => {
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
      const c = data.find((d) => d.time === ts)
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
  }, [data, period])

  // Render chart when data/period changes
  useEffect(() => {
    if (!containerRef.current || data.length === 0) return

    // Cleanup previous chart
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

    // Insert whitespace data points between trading days so lightweight-charts
    // doesn't connect yesterday's close to today's open.
    // Place whitespace right after the last candle of the previous day,
    // not at midnight, to avoid extra ticks on the time axis.
    const insertDayGaps = <T extends { time: number }>(
      items: T[]
    ): (T | { time: number })[] => {
      const result: (T | { time: number })[] = []
      for (let i = 0; i < items.length; i++) {
        if (i > 0) {
          const prevDay = Math.floor(items[i - 1].time / 86400)
          const currDay = Math.floor(items[i].time / 86400)
          if (currDay !== prevDay) {
            result.push({ time: items[i - 1].time + 60 })
          }
        }
        result.push(items[i])
      }
      return result
    }

    const filtered = period === "minute" ? data.filter((c) => isTradingTime(c.time)) : data

    if (filtered.length === 0) return

    if (period === "minute") {
      timeLabelMapRef.current = buildTimeLabels(filtered)
      initIntradayChart()
      const price = priceSeriesRef.current
      const avgPrice = avgPriceSeriesRef.current
      const volume = volumeSeriesRef.current
      const pctAnchor = pctAnchorSeriesRef.current
      if (!price || !avgPrice || !volume || !pctAnchor || !chartRef.current) return

      if (preclose && preclose > 0) {
        const limitPct = getLimitPct(symbol)
        const priceRange = preclose * (limitPct / 100)

        chartRef.current.priceScale("left", 0).setAutoScale(false)
        chartRef.current.priceScale("left", 0).setVisibleRange({
          from: preclose - priceRange,
          to: preclose + priceRange,
        })

        chartRef.current.priceScale("right", 0).setAutoScale(false)
        chartRef.current.priceScale("right", 0).setVisibleRange({ from: -limitPct, to: limitPct })
      }

      // Price line — with day gaps
      const gapped = insertDayGaps(filtered)
      price.setData(gapped.map((c) => ({ time: (c as any).time as Time, value: (c as KlineCandle).close })))

      // VWAP line — with day gaps
      let cumValue = 0
      let cumVolume = 0
      const vwapData = gapped.map((c) => {
        if (!("close" in c)) return { time: c.time as Time }
        cumValue += (c as KlineCandle).close * ((c as KlineCandle).volume || 0)
        cumVolume += (c as KlineCandle).volume || 0
        return {
          time: c.time as Time,
          value: cumVolume > 0 ? cumValue / cumVolume : (c as KlineCandle).close,
        }
      })
      avgPrice.setData(vwapData)

      // Intraday volume — with day gaps
      volume.setData(
        gapped.map((c) => {
          if (!("close" in c)) return { time: c.time as Time }
          const kc = c as KlineCandle
          return {
            time: kc.time as Time,
            value: kc.volume || 0,
            color:
              kc.close >= (kc.open ?? preclose ?? kc.close)
                ? "rgba(255,92,92,0.3)"
                : "rgba(29,158,117,0.3)",
          }
        })
      )

      if (preclose && preclose > 0 && chartRef.current && filtered.length > 0) {
        const limitPct = getLimitPct(symbol)
        const firstTime = filtered[0]?.time as Time
        const lastTime = filtered[filtered.length - 1]?.time as Time

        pctAnchor.setData([
          { time: firstTime, value: -limitPct },
          { time: lastTime, value: limitPct },
        ])

        clearPriceLines()
        priceLinesRef.current.push(
          price.createPriceLine({
            price: preclose,
            color: "rgba(0,0,0,0.20)",
            lineStyle: 2,
            lineWidth: 1,
            axisLabelVisible: true,
            // title: `${preclose.toFixed(2)}`,
          })
        )
        priceLinesRef.current.push(
          pctAnchor.createPriceLine({
            price: 0,
            color: "rgba(0,0,0,0.20)",
            lineStyle: 2,
            lineWidth: 1,
            axisLabelVisible: true,
            // title: "0.00%",
          })
        )

        chartRef.current.timeScale().fitContent()

        const prRange = preclose * (limitPct / 100)
        chartRef.current.priceScale("left", 0).setAutoScale(false)
        chartRef.current.priceScale("left", 0).setVisibleRange({
          from: preclose - prRange,
          to: preclose + prRange,
        })
        chartRef.current.priceScale("right", 0).setVisibleRange({
          from: -limitPct,
          to: limitPct,
        })
      }
    } else {
      // Kline chart (day/week/month)
      initKlineChart()

      const candle = candleSeriesRef.current
      const volume = volumeSeriesRef.current
      if (!candle || !volume || !chartRef.current) return

      candle.setData(
        filtered.map((c) => ({
          time: c.time as Time,
          open: c.open ?? c.close,
          high: c.high ?? c.close,
          low: c.low ?? c.close,
          close: c.close,
        }))
      )

      volume.setData(
        filtered.map((c) => ({
          time: c.time as Time,
          value: c.volume || 0,
          color: c.close >= (c.open ?? c.close) ? "rgba(255,92,92,0.3)" : "rgba(29,158,117,0.3)",
        }))
      )

      // Moving averages (MA5, MA10, MA20, MA30)
      const closes = filtered.map((c) => c.close)
      const ma = (period: number, i: number) => {
        if (i < period - 1) return null
        let sum = 0
        for (let j = i - period + 1; j <= i; j++) sum += closes[j]
        return sum / period
      }
      const maPeriods = [5, 10, 20, 30]
      maPeriods.forEach((p, idx) => {
        const series = seriesRefs.current[idx]
        if (!series) return
        const data = filtered
          .map((c, i) => {
            const v = ma(p, i)
            return v !== null ? { time: c.time as Time, value: v } : null
          })
          .filter(Boolean) as { time: Time; value: number }[]
        if (data.length > 0) series.setData(data)
      })

      clearPriceLines()

      const visible = (() => {
        if (period === "day") return 60
        if (period === "week") return 60
        return 36
      })()
      const to = filtered.length - 1
      const from = Math.max(0, to - visible + 1)
      chartRef.current.timeScale().setVisibleLogicalRange({ from, to })
    }
  }, [data, period, preclose, symbol, initIntradayChart, initKlineChart, clearPriceLines])

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      clearPriceLines()
      chartRef.current?.remove()
      chartRef.current = null
    }
  }, [clearPriceLines])

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
