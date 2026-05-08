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
          <span class="text-xl font-bold font-mono" :class="quote.px_change_rate >= 0 ? 'text-data-red' : 'text-data-green'">{{ quote.last_px?.toFixed(2) }}</span>
          <span class="text-sm font-mono" :class="quote.px_change_rate >= 0 ? 'text-data-red' : 'text-data-green'">{{ quote.px_change_rate >= 0 ? '+' : '' }}{{ quote.px_change_rate?.toFixed(2) }}%</span>
          <span class="text-xs text-white/40">成交 {{ formatVolume(quote.turnover_value) }}</span>
        </div>
      </div>

      <!-- Period Toggle -->
      <div class="flex gap-1 px-4 py-2 sticky z-10 bg-surface-dark border-b border-hairline-dark top-[97px]">
        <button v-for="p in periods" :key="p.value" @click="period = p.value"
          :class="['px-3 py-1 text-xs font-mono transition rounded-button', period === p.value ? 'bg-primary text-white' : 'bg-transparent text-white/40']">
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
          <span>收 <span class="text-white">{{ crosshair.close }}</span></span>
          <span>量 <span class="text-white">{{ crosshair.volume }}</span></span>
          <span :class="crosshair.changeNum >= 0 ? 'text-data-red' : 'text-data-green'">{{ crosshair.change }}</span>
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
  type PriceLine,
} from 'lightweight-charts'

const props = defineProps<{ symbol: string; name: string }>()
defineEmits<{ close: [] }>()

const periods = [
  { label: '1分', value: 'minute' },
  { label: '日K', value: 'day' },
  { label: '周K', value: 'week' },
  { label: '月K', value: 'month' },
]

const period = ref('day')
const chartContainer = ref<HTMLElement>()
const quote = ref<any>(null)

const crosshair = ref({ time: '', open: '-', high: '-', low: '-', close: '-', volume: '-', change: '', changeNum: 0 })

let chart: IChartApi | null = null
let candleSeries: ISeriesApi<'Candlestick'> | null = null
let volumeSeries: ISeriesApi<'Histogram'> | null = null
let anchorSeries: ISeriesApi<'Line'> | null = null
let priceLines: PriceLine[] = []
let klineData: any[] = []

const TRADING_SESSIONS = [[570, 690], [780, 900]] // 9:30-11:30, 13:00-15:00

function isTradingTime(ts: number): boolean {
  const d = new Date(ts * 1000)
  const mins = d.getHours() * 60 + d.getMinutes()
  return TRADING_SESSIONS.some(([s, e]) => mins >= s && mins < e)
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
  try {
    const resp = await fetch(`${getApiBase()}/api/kline/${props.symbol}?period=${period.value}&count=256`)
    if (!resp.ok) return
    const data = await resp.json()
    klineData = data.candles || []

    if (period.value === 'minute') {
      klineData = klineData.filter((c: any) => isTradingTime(c.time))
    }

    renderChart()
    updateCrosshairFromData()
  } catch (e) { console.error('[KlineDrawer] fetchKline error:', e) }
}

function updateCrosshairFromData() {
  if (klineData.length > 0) {
    const last = klineData[klineData.length - 1]
    const preclose = quote.value?.preclose_px
    const changeRate = preclose ? ((last.close - preclose) / preclose * 100) : 0
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
    color: c.close >= c.open ? 'rgba(255,59,48,0.3)' : 'rgba(52,199,89,0.3)',
  }))
}

function initChart() {
  if (!chartContainer.value) return

  chart = createChart(chartContainer.value, {
    width: chartContainer.value.clientWidth,
    height: 420,
    layout: {
      background: { color: '#272729' },
      textColor: '#86868b',
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
    upColor: '#ff3b30',
    downColor: '#34c759',
    borderUpColor: '#ff3b30',
    borderDownColor: '#34c759',
    wickUpColor: '#ff3b30',
    wickDownColor: '#34c759',
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

function clearPriceLines() {
  if (!candleSeries) return
  for (const line of priceLines) {
    candleSeries.removePriceLine(line)
  }
  priceLines = []
}

function updatePriceLines() {
  if (!candleSeries) return
  clearPriceLines()

  if (period.value !== 'minute' || !quote.value?.preclose_px) return

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
      color: '#ff3b30',
      lineStyle: 2,
      lineWidth: 1,
      axisLabelVisible: true,
      title: `涨停 ${limitUp.toFixed(2)}`,
    }),
    candleSeries.createPriceLine({
      price: limitDown,
      color: '#34c759',
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

  if (period.value !== 'minute' || !quote.value?.preclose_px || klineData.length === 0) return

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
    lineWidth: 0,
    priceScaleId: candleSeries.priceScaleId(),
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

  if (!chart) {
    initChart()
  }

  if (!candleSeries || !volumeSeries) return

  candleSeries.setData(buildCandleData())
  volumeSeries.setData(buildVolumeData())
  updatePriceLines()
  updateAnchorSeries()
  chart!.timeScale().fitContent()
}

watch(period, () => fetchKline())

onMounted(async () => {
  await fetchRealtime()
  await nextTick()
  fetchKline()
})

onUnmounted(() => {
  chart?.remove()
  chart = null
  candleSeries = null
  volumeSeries = null
  anchorSeries = null
})
</script>
