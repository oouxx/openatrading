import { useEffect, useRef } from "react"
import {
  CandlestickSeries,
  ColorType,
  CrosshairMode,
  HistogramSeries,
  LineStyle,
  createChart,
  type IChartApi,
  type IPriceLine,
  type ISeriesApi,
  type Time,
} from "lightweight-charts"
import { chartColors, withAlpha } from "@/lib/chartColors"
import type { KlineResponse } from "@/stores/data"

type Candle = KlineResponse["candles"][number]

/**
 * 下单弹窗里的迷你走势图。
 *
 * 不直接复用 `KlineChart`：那个组件是按整页高度（420px）设计、带 MA 图例、
 * 涨跌幅双轴、同花顺时间格线的完整图表，塞进 448px 弹窗既挤又会把弹窗撑高。
 * 这里只留「下单前看一眼走势」需要的东西：蜡烛 + 量 + 昨收虚线，坐标轴隐藏。
 */
export function MiniKlineChart({
  candles,
  prevClose,
  height = 132,
}: {
  candles: Candle[]
  prevClose?: number
  height?: number
}) {
  const containerRef = useRef<HTMLDivElement | null>(null)
  const chartRef = useRef<IChartApi | null>(null)
  const candleRef = useRef<ISeriesApi<"Candlestick"> | null>(null)
  const volumeRef = useRef<ISeriesApi<"Histogram"> | null>(null)
  const precloseLineRef = useRef<IPriceLine | null>(null)

  // 图表实例只建一次；数据变化走下面的 setData，避免每帧重建 canvas。
  useEffect(() => {
    const el = containerRef.current
    if (!el) return
    const chart = createChart(el, {
      width: el.clientWidth,
      height,
      attributionLogo: false,
      layout: {
        background: { type: ColorType.Solid, color: "transparent" },
        textColor: chartColors.tertiary(),
        fontSize: 10,
      },
      grid: {
        vertLines: { visible: false },
        horzLines: { color: withAlpha(chartColors.tertiary(), 0.15) },
      },
      rightPriceScale: { borderVisible: false, scaleMargins: { top: 0.08, bottom: 0.26 } },
      timeScale: { visible: false, borderVisible: false, rightOffset: 1 },
      crosshair: { mode: CrosshairMode.Magnet },
      handleScroll: false,
      handleScale: false,
    } as Parameters<typeof createChart>[1])

    const candleSeries = chart.addSeries(CandlestickSeries, {
      upColor: chartColors.up(),
      downColor: chartColors.down(),
      borderVisible: false,
      wickUpColor: chartColors.up(),
      wickDownColor: chartColors.down(),
    })
    const volumeSeries = chart.addSeries(HistogramSeries, {
      priceScaleId: "",
      priceFormat: { type: "volume" },
      lastValueVisible: false,
      priceLineVisible: false,
    })
    chart.priceScale("").applyOptions({ scaleMargins: { top: 0.78, bottom: 0 } })

    chartRef.current = chart
    candleRef.current = candleSeries
    volumeRef.current = volumeSeries
    precloseLineRef.current = null

    const observer = new ResizeObserver(() => {
      chart.applyOptions({ width: el.clientWidth })
    })
    observer.observe(el)

    return () => {
      observer.disconnect()
      chart.remove()
      chartRef.current = null
      candleRef.current = null
      volumeRef.current = null
      precloseLineRef.current = null
    }
  }, [height])

  useEffect(() => {
    const candleSeries = candleRef.current
    const volumeSeries = volumeRef.current
    const chart = chartRef.current
    if (!candleSeries || !volumeSeries || !chart) return

    // 时间升序 + 去重：Lightweight-Charts 要求严格递增，否则 setData 抛错。
    const byTime = new Map<number, Candle>()
    for (const c of candles) {
      if (Number.isFinite(c?.time) && Number.isFinite(c?.close)) byTime.set(c.time, c)
    }
    const ordered = [...byTime.values()].sort((a, b) => a.time - b.time)

    candleSeries.setData(
      ordered.map((c) => ({
        time: c.time as Time,
        open: c.open ?? c.close,
        high: c.high ?? c.close,
        low: c.low ?? c.close,
        close: c.close,
      }))
    )
    volumeSeries.setData(
      ordered.map((c) => ({
        time: c.time as Time,
        value: c.volume ?? 0,
        color: withAlpha(
          c.close >= (c.open ?? c.close) ? chartColors.up() : chartColors.down(),
          0.35
        ),
      }))
    )

    if (precloseLineRef.current && prevClose) {
      precloseLineRef.current.applyOptions({ price: prevClose })
    } else if (!precloseLineRef.current && prevClose) {
      precloseLineRef.current = candleSeries.createPriceLine({
        price: prevClose,
        color: withAlpha(chartColors.warning(), 0.8),
        lineWidth: 1,
        lineStyle: LineStyle.Dashed,
        axisLabelVisible: false,
      })
    }

    if (ordered.length > 0) {
      chart.timeScale().setVisibleLogicalRange({ from: 0, to: ordered.length - 1 })
    }
  }, [candles, prevClose])

  return (
    <div className="relative">
      <div ref={containerRef} className="w-full" style={{ height }} />
      {candles.length === 0 && (
        <div className="text-tertiary text-caption absolute inset-0 grid place-items-center">
          走势加载中…
        </div>
      )}
    </div>
  )
}
