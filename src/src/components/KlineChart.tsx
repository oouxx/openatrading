import { useRef, useEffect, useCallback } from "react"
import {
  createChart,
  CandlestickSeries,
  HistogramSeries,
  LineSeries,
  CrosshairMode,
  type IChartApi,
  type ISeriesApi,
  type CandlestickData,
  type HistogramData,
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
  const code = symbol.replace(/\.[A-Z]+$/, "")
  if (/^[48]\d{5}$/.test(code)) return 30
  if (/^68\d{4}$/.test(code)) return 20
  if (/^30\d{4}$/.test(code)) return 20
  if (/^60\d{4}$/.test(code)) return 10
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

export default function KlineChart({
  data,
  period,
  preclose,
  symbol,
}: KlineChartProps) {
  const containerRef = useRef<HTMLDivElement>(null)
  const chartRef = useRef<IChartApi | null>(null)
  const candleSeriesRef = useRef<ISeriesApi<"Candlestick"> | null>(null)
  const priceSeriesRef = useRef<ISeriesApi<"Line"> | null>(null)
  const avgPriceSeriesRef = useRef<ISeriesApi<"Line"> | null>(null)
  const pctAnchorSeriesRef = useRef<ISeriesApi<"Line"> | null>(null)
  const volumeSeriesRef = useRef<ISeriesApi<"Histogram"> | null>(null)
  const anchorSeriesRef = useRef<ISeriesApi<"Line"> | null>(null)
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
        background: { color: "#111124" },
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
      crosshair: { mode: CrosshairMode.Normal },
      timeScale: {
        timeVisible: true,
        secondsVisible: false,
        borderColor: "rgba(255,255,255,0.06)",
        tickMarkFormatter: (time: Time) =>
          timeLabelMapRef.current.get(time as number) || "",
      },
      rightPriceScale: {
        borderVisible: false,
        autoScale: false,
        scaleMargins: { top: 0, bottom: 0 },
      },
      leftPriceScale: {
        visible: true,
        borderVisible: false,
        autoScale: false,
        scaleMargins: { top: 0, bottom: 0 },
      },
    })

    priceSeriesRef.current = chart.addSeries(LineSeries, {
      color: "#ffffff",
      lineWidth: 1,
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
        formatter: (v: number) =>
          `${v >= 0 ? "+" : ""}${v.toFixed(2)}%`,
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
      1,
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
      const chgRate = preclose
        ? ((c.close - preclose) / preclose) * 100
        : c.change_rate ?? 0
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
        background: { color: "#111124" },
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
      crosshair: { mode: CrosshairMode.Normal },
      timeScale: {
        timeVisible: period === "minute",
        secondsVisible: false,
        borderColor: "rgba(255,255,255,0.06)",
        tickMarkFormatter: (time: Time) => formatTime(time as number, period),
      },
      rightPriceScale: {
        borderColor: "rgba(255,255,255,0.06)",
        autoScale: true,
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
      1,
    )

    chart.panes()[0].setHeight(340)
    chart.panes()[1].setHeight(80)

    chart.priceScale("right", 1).applyOptions({
      borderVisible: false,
      autoScale: true,
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
    anchorSeriesRef.current = null
    timeLabelMapRef.current = new Map<number, string>()

    const filtered = period === "minute" ? data.filter((c) => isTradingTime(c.time)) : data

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
        chartRef.current
          .priceScale("left", 0)
          .setVisibleRange({
            from: preclose - priceRange,
            to: preclose + priceRange,
          })

        chartRef.current.priceScale("right", 0).setAutoScale(false)
        chartRef.current
          .priceScale("right", 0)
          .setVisibleRange({ from: -limitPct, to: limitPct })
      }

      // Price line
      price.setData(
        filtered.map((c) => ({ time: c.time as Time, value: c.close })),
      )

      // VWAP line
      let cumValue = 0
      let cumVolume = 0
      avgPrice.setData(
        filtered.map((c) => {
          cumValue += c.close * (c.volume || 0)
          cumVolume += c.volume || 0
          return {
            time: c.time as Time,
            value: cumVolume > 0 ? cumValue / cumVolume : c.close,
          }
        }),
      )

      // Intraday volume
      volume.setData(
        filtered.map((c) => ({
          time: c.time as Time,
          value: c.volume || 0,
          color:
            c.close >= (c.open ?? preclose ?? c.close)
              ? "rgba(255,92,92,0.3)"
              : "rgba(29,158,117,0.3)",
        })),
      )

      if (preclose && preclose > 0 && chartRef.current) {
        const limitPct = getLimitPct(symbol)
        const firstTime = filtered[0].time as Time
        const lastTime = filtered[filtered.length - 1].time as Time

        pctAnchor.setData([
          { time: firstTime, value: -limitPct },
          { time: lastTime, value: limitPct },
        ])

        clearPriceLines()
        priceLinesRef.current.push(
          price.createPriceLine({
            price: preclose,
            color: "rgba(255,255,255,0.20)",
            lineStyle: 2,
            lineWidth: 1,
            axisLabelVisible: true,
            title: `昨收 ${preclose.toFixed(2)}`,
          }),
        )
        priceLinesRef.current.push(
          pctAnchor.createPriceLine({
            price: 0,
            color: "rgba(255,255,255,0.20)",
            lineStyle: 2,
            lineWidth: 1,
            axisLabelVisible: true,
            title: "0.00%",
          }),
        )

        chartRef.current.timeScale().fitContent()

        chartRef.current.priceScale("left", 0).setAutoScale(false)
        chartRef.current
          .priceScale("left", 0)
          .setVisibleRange({
            from: preclose - priceRange,
            to: preclose + priceRange,
          })
        chartRef.current.priceScale("right", 0).setVisibleRange({
          from: -limitPct,
          to: limitPct,
        })
      }
    } else {
      // Kline chart
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
        })),
      )

      volume.setData(
        filtered.map((c) => ({
          time: c.time as Time,
          value: c.volume || 0,
          color:
            c.close >= (c.open ?? c.close)
              ? "rgba(255,92,92,0.3)"
              : "rgba(29,158,117,0.3)",
        })),
      )

      // Anchor series
      if (preclose && preclose > 0) {
        const anchor = chartRef.current.addSeries(LineSeries, {
          color: "transparent",
          priceScaleId: "right",
          lastValueVisible: false,
          priceLineVisible: false,
          crosshairMarkerVisible: false,
        })
        const firstTime = filtered[0].time as Time
        const lastTime = filtered[filtered.length - 1].time as Time
        let maxUpper = 0
        let maxLower = 0
        for (const c of filtered) {
          if (c.high && c.high > preclose) maxUpper = Math.max(maxUpper, c.high - preclose)
          if (c.low && c.low < preclose) maxLower = Math.max(maxLower, preclose - c.low)
        }
        const range = Math.max(maxUpper, maxLower) * 1.15 || preclose * 0.02
        anchor.setData([
          { time: firstTime, value: preclose - range },
          { time: lastTime, value: preclose + range },
        ])
        anchorSeriesRef.current = anchor
      }

      const visible = (() => {
        if (period === "day") return 60
        if (period === "week") return 52
        return 24
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
    }
  }, [clearPriceLines])

  // Wrap ref for reuse in KlineDrawer
  ;(KlineChart as any).setCrosshairListener = setCrosshairListener
  ;(KlineChart as any).chartRef = chartRef

  return <div ref={containerRef} className="w-full" style={{ height: 420 }} />
}
