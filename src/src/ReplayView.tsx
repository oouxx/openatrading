import { useState, useEffect, useRef, useCallback } from 'react'
import {
  createChart,
  CandlestickSeries,
  HistogramSeries,
  type IChartApi,
  type ISeriesApi,
  type Time,
} from 'lightweight-charts'
import { apiFetch } from '@/stores/data'

const REPLAY_SPEEDS = [1, 2, 4, 8]

interface ReplayBar {
  time: number
  open: number
  high: number
  low: number
  close: number
  volume: number
}

export default function ReplayView() {
  const [symbol, setSymbol] = useState('000001.SZ')
  const [symbolInput, setSymbolInput] = useState('000001.SZ')
  const [playing, setPlaying] = useState(false)
  const [speed, setSpeed] = useState(1)
  const [progress, setProgress] = useState(0)
  const [totalBars, setTotalBars] = useState(0)
  const [currentBar, setCurrentBar] = useState(0)

  const chartContainerRef = useRef<HTMLDivElement>(null)
  const chartRef = useRef<IChartApi | null>(null)
  const candleSeriesRef = useRef<ISeriesApi<'Candlestick'> | null>(null)
  const volumeSeriesRef = useRef<ISeriesApi<'Histogram'> | null>(null)
  const barsRef = useRef<ReplayBar[]>([])
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null)

  const loadBars = useCallback(async () => {
    try {
      const resp = await apiFetch(`/api/kline/${symbol}?period=minute&count=2000`)
      if (!resp.ok) return
      const data = await resp.json()
      barsRef.current = data.candles || []
      setTotalBars(barsRef.current.length)
      setCurrentBar(0)
      setProgress(0)
    } catch {}
  }, [symbol])

  useEffect(() => {
    loadBars()
  }, [loadBars])

  useEffect(() => {
    if (!chartContainerRef.current) return

    if (chartRef.current) {
      chartRef.current.remove()
      chartRef.current = null
    }

    const chart = createChart(chartContainerRef.current, {
      width: chartContainerRef.current.clientWidth,
      height: 500,
      layout: { background: { color: '#111124' }, textColor: '#888780', fontSize: 11 },
      grid: { vertLines: { color: 'rgba(255,255,255,0.06)' }, horzLines: { color: 'rgba(255,255,255,0.06)' } },
      timeScale: { borderColor: 'rgba(255,255,255,0.06)', timeVisible: true },
      rightPriceScale: { borderColor: 'rgba(255,255,255,0.06)' },
      crosshair: { mode: 0 },
    })

    const candleSeries = chart.addSeries(CandlestickSeries, {
      upColor: '#FF5C5C',
      downColor: '#1D9E75',
      borderUpColor: '#FF5C5C',
      borderDownColor: '#1D9E75',
      wickUpColor: '#FF5C5C',
      wickDownColor: '#1D9E75',
    })

    const volumeSeries = chart.addSeries(HistogramSeries, {
      priceFormat: { type: 'volume' },
      priceScaleId: 'volume',
    })

    chart.priceScale('volume').applyOptions({
      scaleMargins: { top: 0.85, bottom: 0 },
    })

    chartRef.current = chart
    candleSeriesRef.current = candleSeries
    volumeSeriesRef.current = volumeSeries

    return () => {
      chart.remove()
      chartRef.current = null
    }
  }, [])

  useEffect(() => {
    if (playing && currentBar < totalBars) {
      timerRef.current = setInterval(() => {
        setCurrentBar((prev) => {
          const next = prev + 1
          if (next >= totalBars) {
            setPlaying(false)
            return prev
          }
          setProgress(Math.round((next / totalBars) * 100))
          return next
        })
      }, 200 / speed)
    }
    return () => {
      if (timerRef.current) clearInterval(timerRef.current)
    }
  }, [playing, speed, totalBars])

  useEffect(() => {
    if (!candleSeriesRef.current || !volumeSeriesRef.current || !barsRef.current.length) return

    const visibleBars = barsRef.current.slice(0, currentBar + 1)
    if (!visibleBars.length) return

    const candleData = visibleBars.map((b) => ({
      time: b.time as Time,
      open: b.open,
      high: b.high,
      low: b.low,
      close: b.close,
    }))

    const volumeData = visibleBars.map((b) => ({
      time: b.time as Time,
      value: b.volume,
      color: b.close >= b.open ? 'rgba(255,92,92,0.3)' : 'rgba(29,158,117,0.3)',
    }))

    candleSeriesRef.current.setData(candleData)
    volumeSeriesRef.current.setData(volumeData)

    if (chartRef.current) {
      chartRef.current.timeScale().scrollToRealTime()
    }
  }, [currentBar])

  const togglePlay = useCallback(() => {
    if (currentBar >= totalBars - 1) {
      setCurrentBar(0)
      setProgress(0)
    }
    setPlaying((p) => !p)
  }, [currentBar, totalBars])

  const onProgressChange = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    const pct = parseInt(e.target.value)
    const barIdx = Math.round((pct / 100) * (totalBars - 1))
    setCurrentBar(barIdx)
    setProgress(pct)
  }, [totalBars])

  const applySymbol = useCallback(() => {
    if (symbolInput.trim()) {
      setSymbol(symbolInput.trim())
      setPlaying(false)
    }
  }, [symbolInput])

  return (
    <div className="min-h-screen bg-bg pt-14">
      <div className="max-w-[1200px] mx-auto px-6 py-4">
        <h1 className="text-display-md text-white mb-6">行情回放</h1>

        <div className="bg-bg-surface rounded-card border border-hairline p-card-inner mb-4">
          <div className="flex items-center gap-4 flex-wrap">
            <div className="flex items-center gap-2">
              <input
                value={symbolInput}
                onChange={(e) => setSymbolInput(e.target.value)}
                className="bg-bg border border-hairline rounded-input px-3 py-2 text-caption text-white font-mono outline-none focus:border-ai/50 w-[140px]"
                placeholder="股票代码"
                onKeyDown={(e) => e.key === 'Enter' && applySymbol()}
              />
              <button
                onClick={applySymbol}
                className="px-3 py-2 bg-ai/20 text-ai rounded-button text-caption font-mono hover:bg-ai/30 transition-colors cursor-pointer"
              >
                加载
              </button>
            </div>

            <div className="flex items-center gap-2">
              <button
                onClick={togglePlay}
                className="w-10 h-10 bg-ai/20 text-ai rounded-full flex items-center justify-center hover:bg-ai/30 transition-colors cursor-pointer"
              >
                {playing ? '⏸' : '▶'}
              </button>
              <button
                onClick={() => {
                  setPlaying(false)
                  setCurrentBar(0)
                  setProgress(0)
                }}
                className="w-10 h-10 bg-white/5 text-ink-muted rounded-full flex items-center justify-center hover:bg-white/10 transition-colors cursor-pointer"
              >
                ⏮
              </button>
            </div>

            <div className="flex items-center gap-2">
              <span className="text-caption text-ink-muted">速度:</span>
              {REPLAY_SPEEDS.map((s) => (
                <button
                  key={s}
                  onClick={() => setSpeed(s)}
                  className={`px-2 py-1 text-caption font-mono rounded-button cursor-pointer ${
                    speed === s ? 'bg-ai/20 text-ai' : 'bg-white/5 text-ink-muted hover:text-white'
                  }`}
                >
                  {s}x
                </button>
              ))}
            </div>

            <div className="flex-1 flex items-center gap-3">
              <input
                type="range"
                min={0}
                max={100}
                value={progress}
                onChange={onProgressChange}
                className="flex-1 accent-ai"
              />
              <span className="text-caption text-ink-muted font-mono w-[80px]">
                {currentBar}/{totalBars}
              </span>
            </div>
          </div>
        </div>

        <div className="bg-bg-surface rounded-card border border-hairline overflow-hidden">
          <div ref={chartContainerRef} style={{ height: 500 }} />
        </div>
      </div>
    </div>
  )
}