<template>
  <div>
  <Teleport to="body">
    <div @click="$emit('close')" class="fixed inset-0 z-40" style="background: rgba(0,0,0,0.4)"></div>

    <motion.div
      class="fixed top-0 right-0 h-full z-50 overflow-y-auto bg-surface-dark border-l border-hairline-dark"
      style="width: 520px"
      :initial="{ x: 520 }"
      :animate="{ x: 0 }"
      :exit="{ x: 520 }"
      :transition="{ duration: 0.3, ease: [0.32, 0.72, 0, 1] }"
    >
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
      <div class="flex items-center justify-between px-3 py-1 font-mono text-xs bg-surface-dark-2 border-b border-hairline-dark h-[28px] whitespace-nowrap overflow-hidden">
        <span class="text-white shrink-0">{{ crosshair.time }}</span>
        <div class="flex items-center gap-1.5 text-white/40 shrink-0">
          <span>开 <span class="text-white">{{ crosshair.open }}</span></span>
          <span>高 <span class="text-white">{{ crosshair.high }}</span></span>
          <span>低 <span class="text-white">{{ crosshair.low }}</span></span>
          <span>价 <span class="text-white">{{ crosshair.close }}</span></span>
          <span>量 <span class="text-white">{{ crosshair.volume }}</span></span>
          <span :class="crosshair.changeNum >= 0 ? 'text-up' : 'text-down'">{{ crosshair.change }}</span>
        </div>
      </div>

      <!-- Chart -->
      <div ref="chartContainer" class="w-full" style="height: 420px"></div>

      <!-- Plates -->
      <div v-if="plates.length > 0" class="px-4 py-3 border-t border-hairline-dark">
        <p class="text-xs text-white/40 mb-2">所属板块</p>
        <div class="space-y-1.5 max-h-[240px] overflow-y-auto custom-scroll">
          <div v-for="p in plates" :key="p.plate_id" class="bg-surface-dark rounded-card border border-hairline-dark p-2.5 cursor-pointer hover:border-white/20 transition-colors" @click="p.reason && (dialogPlate = p)">
            <div class="flex items-center justify-between gap-2">
              <span class="text-xs font-bold text-white truncate">{{ p.plate_name }}</span>
              <span class="text-xs font-mono shrink-0" :class="p.core_avg_pcp >= 0 ? 'text-up' : 'text-down'">
                {{ p.core_avg_pcp >= 0 ? '+' : '' }}{{ (p.core_avg_pcp * 100).toFixed(2) }}%
              </span>
            </div>
            <p v-if="p.reason" class="text-[11px] text-white/50 mt-1 leading-relaxed line-clamp-2">{{ p.reason }}</p>
          </div>
        </div>
      </div>

      <!-- Plate Reason Dialog -->
      <Teleport to="body">
        <div v-if="dialogPlate" @click="dialogPlate = null" class="fixed inset-0 z-[60]" style="background: rgba(0,0,0,0.5)"></div>
        <motion.div v-if="dialogPlate"
          class="fixed inset-0 z-[61] flex items-center justify-center pointer-events-none"
          :initial="{ opacity: 0, scale: 0.95 }"
          :animate="{ opacity: 1, scale: 1 }"
          :exit="{ opacity: 0, scale: 0.95 }"
          :transition="{ duration: 0.2 }"
        >
          <div class="pointer-events-auto bg-surface-dark border border-hairline-dark rounded-card p-5 w-[420px] max-w-[90vw] shadow-2xl">
            <div class="flex items-center justify-between mb-3">
              <div class="flex items-center gap-2">
                <span class="text-sm font-bold text-white">{{ dialogPlate.plate_name }}</span>
                <span class="text-xs font-mono" :class="dialogPlate.core_avg_pcp >= 0 ? 'text-up' : 'text-down'">
                  {{ dialogPlate.core_avg_pcp >= 0 ? '+' : '' }}{{ (dialogPlate.core_avg_pcp * 100).toFixed(2) }}%
                </span>
              </div>
              <button @click="dialogPlate = null" class="text-lg leading-none p-1 text-white/40 hover:text-white/60">&times;</button>
            </div>
            <p class="text-[13px] text-white/70 leading-relaxed">{{ dialogPlate.reason }}</p>
          </div>
        </motion.div>
      </Teleport>

      <!-- Metrics -->
      <div v-if="quote" class="p-4 grid grid-cols-3 gap-2 text-xs">
        <div v-for="m in metrics" :key="m.label" class="bg-surface-dark rounded-card border border-hairline-dark p-3 text-center">
          <p class="text-white/40 text-[11px]">{{ m.label }}</p>
          <p class="font-mono mt-1 text-white text-xs">{{ m.value }}</p>
        </div>
      </div>
    </motion.div>
  </Teleport>
  </div>
</template>

<script setup lang="ts">
import { motion } from 'motion-v'
import { getApiBaseUrl } from '@/stores/data'
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
const $emit = defineEmits<{ close: [] }>()

const periods = [
  { label: '分时', value: 'minute' },
  { label: '日K', value: 'day' },
  { label: '周K', value: 'week' },
  { label: '月K', value: 'month' },
]

const period = ref('minute')
const chartContainer = ref<HTMLElement>()
const quote = ref<any>(null)
const plates = ref<Array<{ plate_id: number; plate_name: string; core_avg_pcp: number; reason: string }>>([])
const dialogPlate = ref<{ plate_id: number; plate_name: string; core_avg_pcp: number; reason: string } | null>(null)

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

const TRADING_SESSIONS = [[570, 690], [780, 900]]

function getLimitPct(symbol: string): number {
  const code = symbol.replace(/\.[A-Z]+$/, '')

  if (/^[48]\d{5}$/.test(code)) return 30  // 北交所
  if (/^68\d{4}$/.test(code)) return 20    // 科创板
  if (/^30\d{4}$/.test(code)) return 20    // 创业板
  if (/^60\d{4}$/.test(code)) return 10    // 沪市主板
  if (/^00\d{4}$/.test(code)) return 10    // 深市主板
  
  return 10 // fallback
}

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

function formatTime(ts: number, p: string): string {
  const d = new Date(ts * 1000)
  const pad = (n: number) => String(n).padStart(2, '0')
  const ymd = `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`
  if (p === 'minute') return `${ymd} ${pad(d.getHours())}:${pad(d.getMinutes())}`
  return ymd
}

async function fetchRealtime() {
  try {
    const resp = await fetch(`${getApiBaseUrl()}/api/realtime/${props.symbol}`)
    if (resp.ok) quote.value = await resp.json()
  } catch (e) { console.error('[KlineDrawer] fetchRealtime error:', e) }
}

async function fetchPlates() {
  try {
    const resp = await fetch(`${getApiBaseUrl()}/api/plates-by-any-stock/${props.symbol}`)
    if (resp.ok) plates.value = await resp.json()
  } catch (e) { console.error('[KlineDrawer] fetchPlates error:', e) }
}

async function fetchKline() {
  const id = ++fetchId
  try {
    const resp = await fetch(`${getApiBaseUrl()}/api/kline/${props.symbol}?period=${period.value}&count=256`)
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

  let changeRate: number
  if (period.value === 'minute') {
    changeRate = preclose ? ((last.close - preclose) / preclose * 100) : (last.change_rate ?? 0)
  } else {
    changeRate = last.change_rate ?? (preclose ? ((last.close - preclose) / preclose * 100) : 0)
  }

  if (period.value === 'minute') {
    const lastLabel = timeLabelMap.get(last.time) || ''
    crosshair.value = {
      time: lastLabel,
      open: last.open?.toFixed(2) || '-',
      high: last.high?.toFixed(2) || '-',
      low: last.low?.toFixed(2) || '-',
      close: last.close?.toFixed(2) || '-',
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
    color: c.close >= (c.open ?? c.close) ? 'rgba(255,92,92,0.3)' : 'rgba(29,158,117,0.3)',
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

function buildPriceData(): { time: Time; value: number }[] {
  return klineData.map((c: any) => ({
    time: c.time as Time,
    value: c.close,
  }))
}

function buildIntradayVolumeData(): HistogramData[] {
  const preclose = quote.value?.preclose_px
  return klineData.map((c: any) => ({
    time: c.time as Time,
    value: c.volume || 0,
    color: c.close >= (c.open ?? preclose ?? c.close) ? 'rgba(255,92,92,0.3)' : 'rgba(29,158,117,0.3)',
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
      panes: {
        enableResize: false,
        separatorColor: 'rgba(255,255,255,0.08)',
        separatorHoverColor: 'rgba(255,255,255,0.08)',
      },
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
    priceFormat: { type: 'price', precision: 2, minMove: 0.01 },
  })

  chart.addPane(true)

  volumeSeries = chart.addSeries(HistogramSeries, {
    priceScaleId: 'right',
    priceFormat: {
      type: 'custom',
      formatter: (v: number) => {
        if (v >= 1e8) return (v / 1e8).toFixed(1) + '亿'
        if (v >= 1e4) return (v / 1e4).toFixed(0) + '万'
        return v.toFixed(0)
      },
      minMove: 1,
    },
  }, 1)

  chart.panes()[0].setHeight(340)
  chart.panes()[1].setHeight(80)

  chart.priceScale('right', 1).applyOptions({
    borderVisible: false,
    autoScale: true,
  })

  chart.subscribeCrosshairMove((param: any) => {
    if (!param.point || param.time === undefined || param.seriesData.size === 0) return
    const ts = param.time as number
    const c = klineData.find((d: any) => d.time === ts)
    if (!c) return
    const changeRate = c.change_rate ?? 0
    crosshair.value = {
      time: formatTime(ts, period.value),
      open: c.open?.toFixed(2) || '-',
      high: c.high?.toFixed(2) || '-',
      low: c.low?.toFixed(2) || '-',
      close: c.close?.toFixed(2) || '-',
      volume: formatVolume(c.volume || 0),
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
      panes: {
        enableResize: false,
        separatorColor: 'rgba(255,255,255,0.08)',
        separatorHoverColor: 'rgba(255,255,255,0.08)',
      },
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
      borderVisible: false,
      autoScale: false,
    },
    leftPriceScale: {
      visible: true,
      borderVisible: false,
      autoScale: false,
    },
  })

  priceSeries = chart.addSeries(LineSeries, {
    color: '#ffffff',
    lineWidth: 1,
    priceScaleId: 'left',
    crosshairMarkerVisible: true,
    crosshairMarkerRadius: 4,
    lastValueVisible: false,
    priceLineVisible: false,
    priceFormat: { type: 'price', precision: 2, minMove: 0.01 },
  })

  avgPriceSeries = chart.addSeries(LineSeries, {
    color: '#f5a623',
    lineWidth: 1,
    lineStyle: 2,
    priceScaleId: 'left',
    crosshairMarkerVisible: false,
    lastValueVisible: false,
    priceLineVisible: false,
    priceFormat: { type: 'price', precision: 2, minMove: 0.01 },
  })

  pctAnchorSeries = chart.addSeries(LineSeries, {
    color: 'transparent',
    priceScaleId: 'right',
    lastValueVisible: false,
    priceLineVisible: false,
    crosshairMarkerVisible: false,
    priceFormat: { type: 'custom', formatter: (v: number) => `${v >= 0 ? '+' : ''}${v.toFixed(2)}%`, minMove: 0.01 },
  })

  chart.addPane(true)

  volumeSeries = chart.addSeries(HistogramSeries, {
    priceScaleId: 'right',
    priceFormat: {
      type: 'custom',
      formatter: (v: number) => {
        if (v >= 1e8) return (v / 1e8).toFixed(1) + '亿'
        if (v >= 1e4) return (v / 1e4).toFixed(0) + '万'
        return v.toFixed(0)
      },
      minMove: 1,
    },
  }, 1)

  chart.panes()[0].setHeight(320)
  chart.panes()[1].setHeight(100)

  chart.priceScale('right', 1).applyOptions({
    borderVisible: false,
    autoScale: true,
  })

  chart.subscribeCrosshairMove((param: any) => {
    if (!param.point || param.time === undefined || param.seriesData.size === 0) return
    const ts = param.time as number
    const c = klineData.find((d: any) => d.time === ts)
    if (!c) return
    const preclose = quote.value?.preclose_px
    const changeRate = preclose ? ((c.close - preclose) / preclose * 100) : (c.change_rate ?? 0)

    crosshair.value = {
      time: timeLabelMap.get(ts) || '',
      open: c.open?.toFixed(2) || '-',
      high: c.high?.toFixed(2) || '-',
      low: c.low?.toFixed(2) || '-',
      close: c.close?.toFixed(2) || '-',
      volume: formatVolume(c.volume || 0),
      change: changeRate >= 0 ? `+${changeRate.toFixed(2)}%` : `${changeRate.toFixed(2)}%`,
      changeNum: changeRate,
    }
  })
}

function clearPriceLines() {
  const seriesList = period.value === 'minute'
    ? [priceSeries, pctAnchorSeries].filter((s): s is ISeriesApi<any> => s !== null)
    : [candleSeries].filter((s): s is ISeriesApi<any> => s !== null)
  for (const series of seriesList) {
    for (const line of priceLines) {
      try { series.removePriceLine(line) } catch {}
    }
  }
  priceLines = []
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

  const preclose = quote.value?.preclose_px

  priceSeries.setData(buildPriceData())
  avgPriceSeries.setData(buildVwapData())
  volumeSeries.setData(buildIntradayVolumeData())

  if (chart && preclose && preclose > 0) {
    const limitPct = getLimitPct(props.symbol)
    const firstTime = klineData[0].time as Time
    const lastTime = klineData[klineData.length - 1].time as Time

    pctAnchorSeries.setData([
      { time: firstTime, value: -limitPct },
      { time: lastTime, value: limitPct },
    ])
  }

  clearPriceLines()
  if (preclose && preclose > 0 && priceSeries) {
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
    priceLines.push(
      pctAnchorSeries.createPriceLine({
        price: 0,
        color: 'rgba(255,255,255,0.20)',
        lineStyle: 2,
        lineWidth: 1,
        axisLabelVisible: true,
        title: '0.00%',
      })
    )
  }

  chart!.timeScale().fitContent()

  if (chart && preclose && preclose > 0) {
    const limitPct = getLimitPct(props.symbol)
    const priceRange = preclose * limitPct / 100

    chart.priceScale('left', 0).setAutoScale(false)
    chart.priceScale('left', 0).setVisibleRange({ from: preclose - priceRange, to: preclose + priceRange })

    chart.priceScale('right', 0).setAutoScale(false)
    chart.priceScale('right', 0).setVisibleRange({ from: -limitPct, to: limitPct })
  }
}

function visibleBarsForPeriod(p: string): number {
  if (p === 'day') return 60
  if (p === 'week') return 52
  return 24 // month
}

function renderKlineChart() {
  if (klineData.length === 0) return

  if (!chart) {
    initKlineChart()
  }

  if (!candleSeries || !volumeSeries) return

  candleSeries.setData(buildCandleData())
  volumeSeries.setData(buildVolumeData())
  updateAnchorSeries()

  const n = visibleBarsForPeriod(period.value)
  const to = klineData.length - 1
  const from = Math.max(0, to - n + 1)
  chart!.timeScale().setVisibleLogicalRange({ from, to })
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

function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape') $emit('close')
}

onMounted(async () => {
  await fetchRealtime()
  fetchPlates()
  await nextTick()
  fetchKline()
  window.addEventListener('keydown', onKeydown)
})

onUnmounted(() => {
  window.removeEventListener('keydown', onKeydown)
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