import { useRef, useEffect } from 'react'
import { createChart, LineSeries, type IChartApi, type Time } from 'lightweight-charts'

interface SentimentHistoryProps {
  data?: { date: string; value: number }[]
}

export default function SentimentHistory({ data }: SentimentHistoryProps) {
  const chartContainerRef = useRef<HTMLDivElement>(null)
  const chartRef = useRef<IChartApi | null>(null)
  const interpretationRef = useRef('')

  useEffect(() => {
    if (!chartContainerRef.current || !data?.length) return

    if (chartRef.current) {
      chartRef.current.remove()
      chartRef.current = null
    }

    const chart = createChart(chartContainerRef.current, {
      width: chartContainerRef.current.clientWidth,
      height: 200,
      layout: { background: { color: '#111124' }, textColor: '#888780', fontSize: 10 },
      grid: { vertLines: { color: 'rgba(255,255,255,0.06)' }, horzLines: { color: 'rgba(255,255,255,0.06)' } },
      timeScale: { borderColor: 'rgba(255,255,255,0.06)', timeVisible: false },
      rightPriceScale: { borderColor: 'rgba(255,255,255,0.06)', autoScale: true },
      crosshair: { vertLine: { color: 'rgba(255,255,255,0.1)' }, horzLine: { color: 'rgba(255,255,255,0.1)' } },
    })

    const series = chart.addSeries(LineSeries, {
      color: '#7F77DD',
      lineWidth: 2,
      crosshairMarkerVisible: true,
      crosshairMarkerRadius: 4,
      lastValueVisible: true,
      priceLineVisible: false,
      priceFormat: { type: 'custom', formatter: (p: number) => p.toFixed(0) },
    })

    series.setData(data.map((d) => ({ time: d.date as Time, value: d.value })))

    const last = data[data.length - 1]
    if (last) {
      series.createPriceLine({
        price: last.value,
        color: '#7F77DD',
        lineStyle: 2,
        lineWidth: 1,
        axisLabelVisible: true,
        title: `今日 ${last.value}`,
      })
    }

    const recent = data.slice(-3)
    if (recent.length >= 3 && recent.every((d) => d.value >= 75)) {
      interpretationRef.current = '连续3日偏热区间，历史规律显示此后2~3日有回调风险，注意控仓'
    } else if (recent.length >= 3 && recent.every((d) => d.value >= 50)) {
      interpretationRef.current = '情绪正常偏暖，短线操作环境尚可'
    } else if (data.length > 0 && data[data.length - 1].value < 50) {
      interpretationRef.current = '情绪偏冷，等待回暖信号'
    } else {
      interpretationRef.current = ''
    }

    chart.timeScale().fitContent()
    chartRef.current = chart

    return () => {
      chart.remove()
      chartRef.current = null
    }
  }, [data])

  return (
    <div className="bg-bg-surface rounded-card border border-hairline p-card-inner">
      <h3 className="text-caption-strong text-ink-muted mb-4 uppercase tracking-wider">情绪历史走势（近10日）</h3>
      {!data?.length ? (
        <div className="text-center py-8 text-caption text-ink-muted">暂无情绪历史数据</div>
      ) : (
        <>
          <div ref={chartContainerRef} style={{ height: 200 }} />
          {interpretationRef.current && (
            <p className="text-caption text-ink-muted mt-3">{interpretationRef.current}</p>
          )}
        </>
      )}
    </div>
  )
}