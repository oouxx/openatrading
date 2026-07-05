import { useRef, useEffect, useMemo } from "react"
import { createChart, type IChartApi, type Time, LineSeries } from "lightweight-charts"
import { useSentimentHistory } from "@/hooks/query/useSentimentHistory"

export default function SentimentHistory() {
  const containerRef = useRef<HTMLDivElement>(null)
  const chartRef = useRef<IChartApi | null>(null)
  const { data: raw = [] } = useSentimentHistory()

  const chartData = useMemo(() => {
    const rows = (raw as Array<{ date: string; sentiment: number }>).slice(-10)
    return rows.map((d) => ({ time: d.date as Time, value: d.sentiment }))
  }, [raw])

  const interpretation = useMemo(() => {
    const recent = (raw as Array<{ date: string; sentiment: number }>).slice(-3)
    if (recent.length >= 3 && recent.every((d) => d.sentiment >= 75))
      return "连续3日偏热区间，历史规律显示此后2~3日有回调风险，注意控仓"
    if (recent.length >= 3 && recent.every((d) => d.sentiment >= 50))
      return "情绪正常偏暖，短线操作环境尚可"
    if (recent.length > 0 && recent[recent.length - 1].sentiment < 50)
      return "情绪偏冷，等待回暖信号"
    return ""
  }, [raw])

  useEffect(() => {
    if (!containerRef.current || !chartData.length) return

    chartRef.current?.remove()

    const chart = createChart(containerRef.current, {
      width: containerRef.current.clientWidth,
      height: 200,
      layout: {
        attributionLogo: false,
        background: { type: "solid", color: "transparent" } as any,
        textColor: "#888780",
        fontSize: 10,
      },
      grid: {
        vertLines: { color: "rgba(255,255,255,0.06)" },
        horzLines: { color: "rgba(255,255,255,0.06)" },
      },
      timeScale: { borderColor: "rgba(255,255,255,0.06)", timeVisible: false },
      rightPriceScale: { borderColor: "rgba(255,255,255,0.06)", autoScale: true },
      crosshair: {
        vertLine: { color: "rgba(255,255,255,0.1)" },
        horzLine: { color: "rgba(255,255,255,0.1)" },
      },
    })

    const series = chart.addSeries(LineSeries, {
      color: "#7F77DD",
      lineWidth: 2,
      crosshairMarkerVisible: true,
      crosshairMarkerRadius: 4,
      lastValueVisible: true,
      priceLineVisible: false,
    })

    series.setData(chartData)

    const last = chartData[chartData.length - 1]
    if (last) {
      series.createPriceLine({
        price: last.value,
        color: "#7F77DD",
        lineStyle: 2 as const,
        lineWidth: 1,
        axisLabelVisible: true,
        // title: `今日 ${last.value.toFixed(1)}`,
      })
    }

    chart.timeScale().fitContent()
    chartRef.current = chart

    return () => chart.remove()
  }, [chartData])

  return (
    <div className="rounded-card border-hairline bg-bg-surface p-card-inner">
      <h3 className="text-caption-strong text-ink-muted mb-4 tracking-wider uppercase">
        情绪历史走势（近10日）
      </h3>
      {!chartData.length ? (
        <div className="text-caption text-ink-muted py-8 text-center">暂无情绪历史数据</div>
      ) : (
        <div ref={containerRef} style={{ height: 200 }} />
      )}
      {interpretation && <p className="text-caption text-ink-muted mt-3">{interpretation}</p>}
    </div>
  )
}
