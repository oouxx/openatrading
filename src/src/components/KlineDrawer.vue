<template>
  <Teleport to="body">
    <div @click="$emit('close')" class="fixed inset-0 z-40" style="background: rgba(0,0,0,0.4)"></div>

    <div class="fixed top-0 right-0 h-full z-50 overflow-y-auto bg-surface-dark border-l border-hairline-dark" style="width: 520px">
      <!-- Header -->
      <div class="sticky top-0 z-10 p-4 bg-surface-dark border-b border-hairline-dark">
        <div class="flex items-center justify-between mb-2">
          <div class="flex items-center gap-2">
            <span class="text-sm font-bold text-white">{{ name }}</span>
            <span class="text-xs font-mono text-white/40">{{ symbol }}</span>
          </div>
          <button @click="$emit('close')" class="text-lg leading-none p-1 text-white/40 hover:text-white/60">&times;</button>
        </div>
        <div v-if="quote" class="flex items-center gap-3">
          <span class="text-xl font-bold font-mono" :class="quote.px_change_rate >= 0 ? 'text-up' : 'text-down'">{{ quote.last_px?.toFixed(2) }}</span>
          <span class="text-sm font-mono" :class="quote.px_change_rate >= 0 ? 'text-up' : 'text-down'">{{ quote.px_change_rate >= 0 ? '+' : '' }}{{ quote.px_change_rate?.toFixed(2) }}%</span>
          <span class="text-xs text-white/40">成交 {{ formatVolume(quote.turnover_value) }}</span>
        </div>
      </div>

      <!-- Period Toggle -->
      <div class="flex gap-1 px-4 py-2 sticky z-10 bg-surface-dark border-b border-hairline-dark top-[97px]">
        <button v-for="p in periods" :key="p.value" @click="period = p.value"
          :class="['px-3 py-1 text-xs font-mono transition rounded-button', period === p.value ? 'bg-ai text-white' : 'bg-transparent text-white/40']">
          {{ p.label }}
        </button>
      </div>

      <!-- Crosshair Info Bar -->
      <div class="flex items-center justify-between px-4 py-1 font-mono bg-surface-dark-2 border-b border-hairline-dark min-h-[30px] text-caption">
        <span class="text-white">{{ crosshair.time }}</span>
        <div class="flex items-center gap-2 text-white/40">
          <span>开 <span class="text-white">{{ crosshair.open }}</span></span>
          <span>高 <span class="text-white">{{ crosshair.high }}</span></span>
          <span>低 <span class="text-white">{{ crosshair.low }}</span></span>
          <span>{{ period === 'minute' ? '均' : '收' }} <span class="text-white">{{ crosshair.close }}</span></span>
          <span>量 <span class="text-white">{{ crosshair.volume }}</span></span>
          <span :class="crosshair.changeNum >= 0 ? 'text-up' : 'text-down'">{{ crosshair.change }}</span>
        </div>
      </div>

      <!-- Chart -->
      <div ref="chartContainer" class="w-full" style="height: 420px"></div>

      <!-- Metrics -->
      <div v-if="quote" class="p-4 grid grid-cols-3 gap-2 text-xs">
        <div v-for="m in metrics" :key="m.label" class="bg-surface-dark rounded-card border border-hairline-dark p-3 text-center">
          <p class="text-white/40 text-[11px]">{{ m.label }}</p>
          <p class="font-mono mt-1 text-white text-xs">{{ m.value }}</p>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { ref, watch, onMounted, onUnmounted, computed, nextTick } from 'vue'
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
} from 'lightweight-charts'

const props = defineProps<{ symbol: string; name: string }>()
defineEmits<{ close: [] }>()

const periods = [
  { label: '分时', value: 'minute' },
  { label: '日K', value: 'day' },
  { label: '周K', value: 'week' },
  { label: '月K', value: 'month' },
]

const period = ref('minute')
const chartContainer = ref<HTMLElement>()
const quote = ref<any>(null)

const crosshair = ref({ time: '', open: '-', high: '-', low: '-', close: '-', volume: '-', change: '', changeNum: 0 })

let chart: IChartApi | null = null
let candleSeries: ISeriesApi<'Candlestick'> | null = null
let volumeSeries: ISeriesApi<'Histogram'> | null = null
let anchorSeries: ISeriesApi<'Line'> | null = null
let priceLines: IPriceLine[] = []
let priceSeries: ISeriesApi<'Line'> | null = null
let avgPriceSeries: ISeriesApi<'Line'> | null = null
let pctAnchorSeries: ISeriesApi<'Line'> | null = null
let timeLabelMap = new Map<number, string>()
let fetchId = 0
let klineData: any[] = []

const TRADING_SESSIONS = [[570, 690], [780, 900]] // 9:30-11:30, 13:00-15:00

function isTradingTime(ts: number): boolean {
  const d = new Date(ts * 1000)
  const mins = d.getHours() * 60 + d.getMinutes()
  return TRADING_SESSIONS.some(([s, e]) => mins >= s && mins < e)
}

function buildTimeLabels(candles: any[]): Map<number, string> {
  const map = new Map<number, string>()
  candles.forEach((c) => {
    const d = new Date(c.time * 1000)
    const pad = (n: number) => String(n).padStart(2, '0')
    map.set(c.time, `${pad(d.getHours())}:${pad(d.getMinutes())}`)
  })
  return map
}

function formatVolume(v: number) {
  if (!v) return '-'
  if (v > 1e8) return (v / 1e8).toFixed(2) + '亿'
  if (v > 1e4) return (v / 1e4).toFixed(2) + '万'
  return v.toFixed(0)
}

const metrics = computed(() => {
  if (!quote.value) return []
  return [
    { label: '今开', value: quote.value.open_px?.toFixed(2) || '-' },
    { label: '昨收', value: quote.value.preclose_px?.toFixed(2) || '-' },
    { label: '最高', value: quote.value.high_px?.toFixed(2) || '-' },
    { label: '最低', value: quote.value.low_px?.toFixed(2) || '-' },
    { label: '振幅', value: quote.value.amplitude != null ? quote.value.amplitude.toFixed(2) + '%' : '-' },
    { label: '换手率', value: quote.value.turnover_ratio != null ? quote.value.turnover_ratio.toFixed(2) + '%' : '-' },
    { label: '动态PE', value: quote.value.dyn_pe ? quote.value.dyn_pe.toFixed(1) : '-' },
    { label: '市净率', value: quote.value.dyn_pb_rate ? quote.value.dyn_pb_rate.toFixed(2) : '-' },
    { label: '总市值', value: formatVolume(quote.value.market_value) },
    { label: '流通市值', value: formatVolume(quote.value.circulation_value) },
    { label: '量比', value: quote.value.volume_ratio?.toFixed(2) || '-' },
    { label: '成交量', value: formatVolume(quote.value.turnover_volume) + '股' },
  ]
})

function getLimitRate(): number {
  if (props.name?.includes('ST')) return 0.05
  if (props.symbol.startsWith('688')) return 0.20
  if (/^30[01]/.test(props.symbol)) return 0.20
  if (/^8/.test(props.symbol)) return 0.30
  return 0.10
}

function formatTime(ts: number, p: string): string {
  const d = new Date(ts * 1000)
  const pad = (n: number) => String(n).padStart(2, '0')
  const ymd = `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`
  if (p === 'minute') return `${ymd} ${pad(d.getHours())}:${pad(d.getMinutes())}`
  return ymd
}

declare const __API_BASE_URL__: string

function getApiBase(): string {
  return localStorage.getItem('trading-agent-api-url') || __API_BASE_URL__
}

async function fetchRealtime() {
  try {
    const resp = await fetch(`${getApiBase()}/api/realtime/${props.symbol}`)
    if (resp.ok) quote.value = await resp.json()
  } catch (e) { console.error('[KlineDrawer] fetchRealtime error:', e) }
}

async function fetchKline() {
  const id = ++fetchId
  try {
    const resp = await fetch(`${getApiBase()}/api/kline/${props.symbol}?period=${period.value}&count=256`)
    if (!resp.ok) return
    const data = await resp.json()
    if (id !== fetchId) return
    klineData = data.candles || []

    if (period.value === 'minute') {
      klineData = klineData.filter((c: any) => isTradingTime(c.time))
    }

    if (id !== fetchId) return
    renderChart()
    updateCrosshairFromData()
  } catch (e) { console.error('[KlineDrawer] fetchKline error:', e) }
}

function updateCrosshairFromData() {
  if (klineData.length === 0) return
  const last = klineData[klineData.length - 1]
  const preclose = quote.value?.preclose_px
  const changeRate = preclose ? ((last.close - preclose) / preclose * 100) : 0

  if (period.value === 'minute') {
    let cv = 0, cvol = 0
    for (let j = 0; j < klineData.length; j++) {
      cv += klineData[j].close * klineData[j].volume
      cvol += klineData[j].volume
    }
    const vwap = cvol > 0 ? cv / cvol : last.close
    const lastLabel = timeLabelMap.get(last.time) || ''
    crosshair.value = {
      time: lastLabel,
      open: last.open?.toFixed(2) || '-',
      high: last.high?.toFixed(2) || '-',
      low: last.low?.toFixed(2) || '-',
      close: vwap?.toFixed(2) || '-',
      volume: formatVolume(last.volume || 0),
      change: changeRate >= 0 ? `+${changeRate.toFixed(2)}%` : `${changeRate.toFixed(2)}%`,
      changeNum: changeRate,
    }
  } else {
    crosshair.value = {
      time: formatTime(last.time, period.value),
      open: last.open?.toFixed(2) || '-',
      high: last.high?.toFixed(2) || '-',
      low: last.low?.toFixed(2) || '-',
      close: last.close?.toFixed(2) || '-',
      volume: formatVolume(last.volume || 0),
      change: changeRate >= 0 ? `+${changeRate.toFixed(2)}%` : `${changeRate.toFixed(2)}%`,
      changeNum: changeRate,
    }
  }
}

function buildCandleData(): CandlestickData[] {
  return klineData.map((c: any) => ({
    time: c.time as Time,
    open: c.open,
    high: c.high,
    low: c.low,
    close: c.close,
  }))
}

function buildVolumeData(): HistogramData[] {
  return klineData.map((c: any) => ({
    time: c.time as Time,
    value: c.volume || 0,
    color: c.close >= c.open ? 'rgba(255,92,92,0.3)' : 'rgba(29,158,117,0.3)',
  }))
}

function buildVwapData(): { time: Time; value: number }[] {
  let cumValue = 0
  let cumVolume = 0
  return klineData.map((c: any) => {
    cumValue += c.close * c.volume
    cumVolume += c.volume
    return {
      time: c.time as Time,
      value: cumVolume > 0 ? cumValue / cumVolume : c.close,
    }
  })
}

function buildPctAnchorData(preclose: number): { time: Time; value: number }[] {
  return klineData.map((c: any) => ({
    time: c.time as Time,
    value: (c.close - preclose) / preclose * 100,
  }))
}

function buildPriceData(): { time: Time; value: number }[] {
  return klineData.map((c: any) => ({
    time: c.time as Time,
    value: c.close,
  }))
}

function buildIntradayVolumeData(): HistogramData[] {
  return klineData.map((c: any) => ({
    time: c.time as Time,
    value: c.volume || 0,
    color: c.close >= c.open ? 'rgba(255,92,92,0.3)' : 'rgba(29,158,117,0.3)',
  }))
}

function initKlineChart() {
  if (!chartContainer.value) return

  chart = createChart(chartContainer.value, {
    width: chartContainer.value.clientWidth,
    height: 420,
    layout: {
      background: { color: '#111124' },
      textColor: '#888780',
      fontSize: 10,
    },
    grid: {
      vertLines: { color: 'rgba(255,255,255,0.06)' },
      horzLines: { color: 'rgba(255,255,255,0.06)' },
    },
    crosshair: {
      mode: CrosshairMode.Normal,
    },
    timeScale: {
      timeVisible: period.value === 'minute',
      secondsVisible: false,
      borderColor: 'rgba(255,255,255,0.06)',
      tickMarkFormatter: (time: Time) => formatTime(time as number, period.value),
    },
    rightPriceScale: {
      borderColor: 'rgba(255,255,255,0.06)',
      autoScale: true,
    },
    leftPriceScale: {
      visible: false,
    },
  })

  candleSeries = chart.addSeries(CandlestickSeries, {
    upColor: '#FF5C5C',
    downColor: '#1D9E75',
    borderUpColor: '#FF5C5C',
    borderDownColor: '#1D9E75',
    wickUpColor: '#FF5C5C',
    wickDownColor: '#1D9E75',
    priceScaleId: 'right',
  })

  volumeSeries = chart.addSeries(HistogramSeries, {
    priceScaleId: 'volume',
    priceFormat: { type: 'volume' },
  })

  // Set up volume price scale
  chart.priceScale('volume').applyOptions({
    scaleMargins: { top: 0.85, bottom: 0 },
    borderColor: 'rgba(255,255,255,0.06)',
  })

  chart.subscribeCrosshairMove((param: any) => {
    if (!param.point || !param.time || param.seriesData.size === 0) return
    const data = param.seriesData.get(candleSeries!)
    if (!data) return
    const preclose = quote.value?.preclose_px
    const changeRate = preclose ? ((data.close - preclose) / preclose * 100) : 0
    crosshair.value = {
      time: formatTime(data.time as number, period.value),
      open: data.open?.toFixed(2) || '-',
      high: data.high?.toFixed(2) || '-',
      low: data.low?.toFixed(2) || '-',
      close: data.close?.toFixed(2) || '-',
      volume: formatVolume((param.seriesData.get(volumeSeries!) as any)?.value || 0),
      change: changeRate >= 0 ? `+${changeRate.toFixed(2)}%` : `${changeRate.toFixed(2)}%`,
      changeNum: changeRate,
    }
  })
}

function initIntradayChart() {
  if (!chartContainer.value) return

  chart = createChart(chartContainer.value, {
    width: chartContainer.value.clientWidth,
    height: 420,
    layout: {
      background: { color: '#111124' },
      textColor: '#888780',
      fontSize: 10,
    },
    grid: {
      vertLines: { color: 'rgba(255,255,255,0.06)' },
      horzLines: { color: 'rgba(255,255,255,0.06)' },
    },
    crosshair: {
      mode: CrosshairMode.Normal,
    },
    timeScale: {
      timeVisible: true,
      secondsVisible: false,
      borderColor: 'rgba(255,255,255,0.06)',
      tickMarkFormatter: (time: Time) => {
        return timeLabelMap.get(time as number) || ''
      },
    },
    rightPriceScale: {
      borderColor: 'rgba(255,255,255,0.06)',
      autoScale: true,
      scaleMargins: { top: 0.05, bottom: 0.05 },
    },
    leftPriceScale: {
      visible: true,
      borderColor: 'rgba(255,255,255,0.06)',
      autoScale: true,
      scaleMargins: { top: 0.05, bottom: 0.05 },
    },
  })

  // Price line (left scale)
  priceSeries = chart.addSeries(LineSeries, {
    color: '#ffffff',
    lineWidth: 1,
    priceScaleId: 'left',
    crosshairMarkerVisible: true,
    crosshairMarkerRadius: 4,
    lastValueVisible: false,
    priceLineVisible: false,
  })

  // VWAP average price line (left scale)
  avgPriceSeries = chart.addSeries(LineSeries, {
    color: '#f5a623',
    lineWidth: 1,
    lineStyle: 2, // dashed
    priceScaleId: 'left',
    crosshairMarkerVisible: false,
    lastValueVisible: false,
    priceLineVisible: false,
  })

  // Hidden pct anchor (right scale) — drives right Y-axis autoScale
  pctAnchorSeries = chart.addSeries(LineSeries, {
    color: 'transparent',
    priceScaleId: 'right',
    lastValueVisible: false,
    priceLineVisible: false,
    crosshairMarkerVisible: false,
  })

  // Volume (separate scale at bottom)
  volumeSeries = chart.addSeries(HistogramSeries, {
    priceScaleId: 'volume',
    priceFormat: { type: 'volume' },
  })

  chart.priceScale('volume').applyOptions({
    scaleMargins: { top: 0.80, bottom: 0 },
    borderColor: 'rgba(255,255,255,0.06)',
  })

  chart.subscribeCrosshairMove((param: any) => {
    if (!param.point || param.time === undefined || param.seriesData.size === 0) return
    const ts = param.time as number
    const c = klineData.find((d: any) => d.time === ts)
    if (!c) return
    const preclose = quote.value?.preclose_px
    const changeRate = preclose ? ((c.close - preclose) / preclose * 100) : 0

    const idx = klineData.indexOf(c)
    let cv = 0, cvol = 0
    for (let j = 0; j <= idx; j++) {
      cv += klineData[j].close * klineData[j].volume
      cvol += klineData[j].volume
    }
    const vwapVal = cvol > 0 ? cv / cvol : c.close

    crosshair.value = {
      time: timeLabelMap.get(ts) || '',
      open: c.open?.toFixed(2) || '-',
      high: c.high?.toFixed(2) || '-',
      low: c.low?.toFixed(2) || '-',
      close: vwapVal?.toFixed(2) || '-',
      volume: formatVolume(c.volume || 0),
      change: changeRate >= 0 ? `+${changeRate.toFixed(2)}%` : `${changeRate.toFixed(2)}%`,
      changeNum: changeRate,
    }
  })
}

function clearPriceLines() {
  const series = period.value === 'minute' ? priceSeries : candleSeries
  if (!series) return
  for (const line of priceLines) {
    series.removePriceLine(line)
  }
  priceLines = []
}

function updatePriceLines() {
  if (!candleSeries) return
  clearPriceLines()

  if (period.value === 'minute' || !quote.value?.preclose_px) return

  const preclose = quote.value.preclose_px
  const rate = getLimitRate()
  const limitUp = Math.round(preclose * (1 + rate) * 100) / 100
  const limitDown = Math.round(preclose * (1 - rate) * 100) / 100

  priceLines.push(
    candleSeries.createPriceLine({
      price: preclose,
      color: 'rgba(255,255,255,0.20)',
      lineStyle: 2, // dashed
      lineWidth: 1,
      axisLabelVisible: true,
      title: `昨收 ${preclose.toFixed(2)}`,
    }),
    candleSeries.createPriceLine({
      price: limitUp,
      color: '#FF5C5C',
      lineStyle: 2,
      lineWidth: 1,
      axisLabelVisible: true,
      title: `涨停 ${limitUp.toFixed(2)}`,
    }),
    candleSeries.createPriceLine({
      price: limitDown,
      color: '#1D9E75',
      lineStyle: 2,
      lineWidth: 1,
      axisLabelVisible: true,
      title: `跌停 ${limitDown.toFixed(2)}`,
    }),
  )
}

function updateAnchorSeries() {
  if (!chart || !candleSeries) return

  // Remove old anchor series if exists
  if (anchorSeries) {
    chart.removeSeries(anchorSeries)
    anchorSeries = null
  }

  if (period.value === 'minute' || !quote.value?.preclose_px || klineData.length === 0) return

  const preclose = quote.value.preclose_px
  let maxUpper = 0, maxLower = 0
  for (const c of klineData) {
    if (c.high > preclose) maxUpper = Math.max(maxUpper, c.high - preclose)
    if (c.low < preclose) maxLower = Math.max(maxLower, preclose - c.low)
  }
  const range = Math.max(maxUpper, maxLower) * 1.15 || preclose * 0.02
  const yMin = preclose - range
  const yMax = preclose + range

  anchorSeries = chart.addSeries(LineSeries, {
    color: 'transparent',
    priceScaleId: 'right',
    lastValueVisible: false,
    priceLineVisible: false,
    crosshairMarkerVisible: false,
  })

  const firstTime = klineData[0].time as Time
  const lastTime = klineData[klineData.length - 1].time as Time
  anchorSeries.setData([
    { time: firstTime, value: yMin },
    { time: lastTime, value: yMax },
  ])
}

function renderChart() {
  if (!chartContainer.value) return

  if (period.value === 'minute') {
    renderIntradayChart()
  } else {
    renderKlineChart()
  }
}

function renderIntradayChart() {
  if (klineData.length === 0) {
    crosshair.value = { time: '暂无分时数据', open: '-', high: '-', low: '-', close: '-', volume: '-', change: '', changeNum: 0 }
    return
  }

  timeLabelMap = buildTimeLabels(klineData)

  if (!chart) {
    initIntradayChart()
  }

  if (!priceSeries || !avgPriceSeries || !volumeSeries || !pctAnchorSeries) return

  priceSeries.setData(buildPriceData())
  avgPriceSeries.setData(buildVwapData())
  const preclose = quote.value?.preclose_px
  if (preclose && preclose > 0) {
    pctAnchorSeries.setData(buildPctAnchorData(preclose))
  }
  volumeSeries.setData(buildIntradayVolumeData())

  // Preclose price line on price series
  clearPriceLines()
  if (quote.value?.preclose_px && priceSeries) {
    priceLines.push(
      priceSeries.createPriceLine({
        price: quote.value.preclose_px,
        color: 'rgba(255,255,255,0.20)',
        lineStyle: 2,
        lineWidth: 1,
        axisLabelVisible: true,
        title: `昨收 ${quote.value.preclose_px.toFixed(2)}`,
      })
    )
  }

  chart!.timeScale().fitContent()
}

function renderKlineChart() {
  if (klineData.length === 0) return

  if (!chart) {
    initKlineChart()
  }

  if (!candleSeries || !volumeSeries) return

  candleSeries.setData(buildCandleData())
  volumeSeries.setData(buildVolumeData())
  updatePriceLines()
  updateAnchorSeries()
  chart!.timeScale().fitContent()
}

function resetChart() {
  clearPriceLines()
  if (chart) {
    chart.remove()
    chart = null
    candleSeries = null
    priceSeries = null
    avgPriceSeries = null
    pctAnchorSeries = null
    volumeSeries = null
    anchorSeries = null
  }
  timeLabelMap = new Map<number, string>()
  fetchId++
}

watch(period, () => {
  resetChart()
  fetchKline()
})

onMounted(async () => {
  await fetchRealtime()
  await nextTick()
  fetchKline()
})

onUnmounted(() => {
  clearPriceLines()
  chart?.remove()
  chart = null
  candleSeries = null
  priceSeries = null
  avgPriceSeries = null
  pctAnchorSeries = null
  volumeSeries = null
  anchorSeries = null
  timeLabelMap = new Map<number, string>()
  fetchId++
})
</script>