<template>
  <Teleport to="body">
    <div @click="$emit('close')" class="fixed inset-0 z-40" style="background: rgba(0,0,0,0.4)"></div>

    <div class="fixed top-0 right-0 h-full z-50 overflow-y-auto" style="width: 520px; background: var(--bg-secondary); border-left: 1px solid var(--border-subtle)">
      <!-- Header -->
      <div class="sticky top-0 z-10 p-4" style="background: var(--bg-secondary); border-bottom: 1px solid var(--border-subtle)">
        <div class="flex items-center justify-between mb-2">
          <div class="flex items-center gap-2">
            <span class="text-sm font-bold" style="color: var(--text-primary)">{{ name }}</span>
            <span class="text-xs font-mono" style="color: var(--text-muted)">{{ symbol }}</span>
          </div>
          <button @click="$emit('close')" class="text-lg leading-none p-1" style="color: var(--text-muted)">&times;</button>
        </div>
        <div v-if="quote" class="flex items-center gap-3">
          <span class="text-xl font-bold font-mono" :style="{ color: quote.px_change_rate >= 0 ? 'var(--red)' : 'var(--green)' }">{{ quote.last_px?.toFixed(2) }}</span>
          <span class="text-sm font-mono" :style="{ color: quote.px_change_rate >= 0 ? 'var(--red)' : 'var(--green)' }">{{ quote.px_change_rate >= 0 ? '+' : '' }}{{ quote.px_change_rate?.toFixed(2) }}%</span>
          <span class="text-xs" style="color: var(--text-muted)">成交 {{ formatVolume(quote.turnover_value) }}</span>
        </div>
      </div>

      <!-- Period Toggle -->
      <div class="flex gap-1 px-4 py-2 sticky z-10" style="background: var(--bg-secondary); border-bottom: 1px solid var(--border-subtle); top: 97px">
        <button v-for="p in periods" :key="p.value" @click="period = p.value"
          class="px-3 py-1 text-xs rounded font-mono transition"
          :style="period === p.value ? { background: 'var(--blue)', color: '#fff' } : { background: 'transparent', color: 'var(--text-muted)' }">
          {{ p.label }}
        </button>
      </div>

      <!-- Crosshair Info Bar -->
      <div class="flex items-center justify-between px-4 py-1 font-mono" style="background: #1a2332; border-bottom: 1px solid #1e2a3a; min-height: 30px; font-size: 11px">
        <span style="color: var(--text-primary)">{{ crosshair.time }}</span>
        <div class="flex items-center gap-2" style="color: var(--text-muted)">
          <span>开 <span style="color: var(--text-primary)">{{ crosshair.open }}</span></span>
          <span>高 <span style="color: var(--text-primary)">{{ crosshair.high }}</span></span>
          <span>低 <span style="color: var(--text-primary)">{{ crosshair.low }}</span></span>
          <span>收 <span style="color: var(--text-primary)">{{ crosshair.close }}</span></span>
          <span>量 <span style="color: var(--text-primary)">{{ crosshair.volume }}</span></span>
          <span :style="{ color: crosshair.changeNum >= 0 ? 'var(--red)' : 'var(--green)' }">{{ crosshair.change }}</span>
        </div>
      </div>

      <!-- Chart -->
      <div ref="chartContainer" class="w-full relative" style="height: 420px">
        <!-- Floating crosshair tooltip -->
        <div v-if="tooltip.visible" class="absolute pointer-events-none z-50 px-2 py-1 font-mono rounded" style="background: rgba(0,0,0,0.75); color: #fff; font-size: 11px; transform: translate(-50%, -120%); white-space: nowrap; border: 1px solid rgba(255,255,255,0.15)"
          :style="{ left: tooltip.x + 'px', top: tooltip.y + 'px' }">
          {{ tooltip.price }}
        </div>
      </div>

      <!-- Metrics -->
      <div v-if="quote" class="p-4 grid grid-cols-3 gap-2 text-xs">
        <div v-for="m in metrics" :key="m.label" class="glow-card p-3 text-center">
          <p style="color: var(--text-muted); font-size: 11px">{{ m.label }}</p>
          <p class="font-mono mt-1" style="color: var(--text-primary); font-size: 12px">{{ m.value }}</p>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { ref, watch, onMounted, onUnmounted, computed } from 'vue'
import { createChart, ColorType, CandlestickSeries, HistogramSeries, LineSeries, LineStyle } from 'lightweight-charts'

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

const crosshair = ref({ visible: false, time: '', open: '-', high: '-', low: '-', close: '-', volume: '-', change: '', changeNum: 0 })
const tooltip = ref({ visible: false, x: 0, y: 0, price: '' })

let chart: any = null
let candleSeries: any = null
let volumeSeries: any = null
let limitUpLine: any = null
let limitDownLine: any = null
let zeroLine: any = null
let centerBounding: any = null
let klineData: any[] = []

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

function getLimitRate(symbol: string, name: string): number {
  // ST 股 ±5%
  if (name?.includes('ST')) return 0.05
  // 科创板 688xxx ±20%
  if (symbol.startsWith('688')) return 0.20
  // 创业板 300xxx, 301xxx ±20%
  if (/^30[01]/.test(symbol)) return 0.20
  // 北交所 8xxxxx ±30%
  if (/^8/.test(symbol)) return 0.30
  // 主板 ±10%
  return 0.10
}

function updateLimitLines() {
  // remove old lines
  if (limitUpLine) { candleSeries?.removePriceLine(limitUpLine); limitUpLine = null }
  if (limitDownLine) { candleSeries?.removePriceLine(limitDownLine); limitDownLine = null }
  if (zeroLine) { candleSeries?.removePriceLine(zeroLine); zeroLine = null }

  if (!quote.value?.preclose_px || !candleSeries) return

  const preclose = quote.value.preclose_px

  // 昨收零轴线 — 仅1分K
  if (period.value === 'minute') {
    zeroLine = candleSeries.createPriceLine({
      price: preclose,
      color: 'rgba(255,255,255,0.30)',
      lineWidth: 1,
      lineStyle: LineStyle.Dashed,
      axisLabelVisible: true,
      title: `昨收 ${preclose.toFixed(2)}`,
    })
  }

  // 涨停/跌停线 — 仅1分K
  if (period.value !== 'minute') return

  const rate = getLimitRate(props.symbol, props.name)
  const limitUp = Math.round(preclose * (1 + rate) * 100) / 100
  const limitDown = Math.round(preclose * (1 - rate) * 100) / 100

  limitUpLine = candleSeries.createPriceLine({
    price: limitUp,
    color: '#ff1744',
    lineWidth: 1,
    lineStyle: LineStyle.Dashed,
    axisLabelVisible: true,
    title: `涨停 ${limitUp.toFixed(2)}`,
  })

  limitDownLine = candleSeries.createPriceLine({
    price: limitDown,
    color: '#00e676',
    lineWidth: 1,
    lineStyle: LineStyle.Dashed,
    axisLabelVisible: true,
    title: `跌停 ${limitDown.toFixed(2)}`,
  })
}

function updateCenterBounding() {
  if (!klineData.length || !quote.value?.preclose_px || !centerBounding) return
  const preclose = quote.value.preclose_px
  let maxUpper = 0
  let maxLower = 0
  for (const c of klineData) {
    if (c.high > preclose) maxUpper = Math.max(maxUpper, c.high - preclose)
    if (c.low < preclose) maxLower = Math.max(maxLower, preclose - c.low)
  }
  const range = Math.max(maxUpper, maxLower) * 1.15 || preclose * 0.02
  centerBounding.setData([
    { time: klineData[0].time, value: preclose + range },
    { time: klineData[klineData.length - 1].time, value: preclose - range },
  ])
}

function formatCrosshairTime(ts: number, p: string): string {
  const d = new Date(ts * 1000)
  const pad = (n: number) => String(n).padStart(2, '0')
  const ymd = `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`
  if (p === 'minute') return `${ymd} ${pad(d.getHours())}:${pad(d.getMinutes())}`
  return ymd
}

function crosshairHandler(param: any) {
  if (!param.point || !param.time || !candleSeries) {
    tooltip.value = { visible: false, x: 0, y: 0, price: '' }
    return
  }

  const candleData = param.seriesData?.get(candleSeries) as any
  const volData = param.seriesData?.get(volumeSeries) as any

  if (candleData) {
    const preclose = quote.value?.preclose_px
    const changeRate = preclose ? ((candleData.close - preclose) / preclose * 100) : 0
    crosshair.value = {
      visible: true,
      time: formatCrosshairTime(candleData.time as number, period.value),
      open: candleData.open?.toFixed(2) || '-',
      high: candleData.high?.toFixed(2) || '-',
      low: candleData.low?.toFixed(2) || '-',
      close: candleData.close?.toFixed(2) || '-',
      volume: formatVolume(volData?.value || 0),
      change: changeRate >= 0 ? `+${changeRate.toFixed(2)}%` : `${changeRate.toFixed(2)}%`,
      changeNum: changeRate,
    }
    tooltip.value = {
      visible: true,
      x: param.point.x,
      y: param.point.y,
      price: candleData.close?.toFixed(2) || '-',
    }
  }
}

function getApiBase(): string {
  return localStorage.getItem('trading-agent-api-url') || 'http://localhost:8080'
}

async function fetchRealtime() {
  try {
    const resp = await fetch(`${getApiBase()}/api/realtime/${props.symbol}`)
    if (resp.ok) quote.value = await resp.json()
  } catch (e) { console.error('[KlineDrawer] fetchRealtime error:', e) }
}

function toChartTime(ts: number): number {
  // lightweight-charts 使用 Unix 秒级时间戳作为 UTCTimestamp
  return ts
}

async function fetchKline() {
  try {
    const resp = await fetch(`${getApiBase()}/api/kline/${props.symbol}?period=${period.value}&count=256`)
    if (!resp.ok) return
    const data = await resp.json()
    klineData = data.candles
    candleSeries?.setData(data.candles.map((c: any) => ({
      time: toChartTime(c.time),
      open: c.open,
      high: c.high,
      low: c.low,
      close: c.close,
    })))
    volumeSeries?.setData(data.candles.map((c: any) => ({
      time: toChartTime(c.time),
      value: c.volume,
      color: c.close >= c.open ? 'rgba(255,23,68,0.3)' : 'rgba(0,230,118,0.3)',
    })))
    if (period.value === 'minute') updateCenterBounding()

    // 初始化信息栏为最新一根K线数据
    if (data.candles.length > 0) {
      const last = data.candles[data.candles.length - 1]
      const preclose = quote.value?.preclose_px
      const changeRate = preclose ? ((last.close - preclose) / preclose * 100) : 0
      crosshair.value = {
        visible: true,
        time: formatCrosshairTime(last.time, period.value),
        open: last.open?.toFixed(2) || '-',
        high: last.high?.toFixed(2) || '-',
        low: last.low?.toFixed(2) || '-',
        close: last.close?.toFixed(2) || '-',
        volume: formatVolume(last.volume || 0),
        change: changeRate >= 0 ? `+${changeRate.toFixed(2)}%` : `${changeRate.toFixed(2)}%`,
        changeNum: changeRate,
      }
    }
  } catch (e) { console.error('[KlineDrawer] fetchKline error:', e) }
}

async function initChart() {
  if (!chartContainer.value) return
  if (chart) { chart.remove(); chart = null; volumeSeries = null; centerBounding = null }

  chart = createChart(chartContainer.value, {
    layout: {
      background: { type: ColorType.Solid, color: '#111827' },
      textColor: '#8899aa',
    },
    grid: { vertLines: { color: '#1e2a3a' }, horzLines: { color: '#1e2a3a' } },
    width: chartContainer.value.clientWidth || 520,
    height: 420,
    crosshair: { mode: 0 },
    timeScale: {
      borderColor: '#1e2a3a',
      timeVisible: true,
      secondsVisible: false,
    },
    localization: {
      timeFormatter: (ts: number) => {
        const d = new Date(ts * 1000)
        return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
      },
    },
    rightPriceScale: { borderColor: '#1e2a3a' },
  })

  candleSeries = chart.addSeries(CandlestickSeries, {
    upColor: '#ff1744',
    downColor: '#00e676',
    borderUpColor: '#ff1744',
    borderDownColor: '#00e676',
    wickUpColor: '#ff1744',
    wickDownColor: '#00e676',
    priceScaleId: 'right',
  })

  volumeSeries = chart.addSeries(HistogramSeries, {
    color: '#334455',
    priceFormat: { type: 'volume' },
    priceScaleId: 'volume',
  })
  chart.priceScale('volume').applyOptions({
    scaleMargins: { top: 0.8, bottom: 0 },
  })

  // 隐藏的边界序列，用于1分K将昨收价固定在价格轴中央
  centerBounding = chart.addSeries(LineSeries, {
    lineVisible: false,
    lastValueVisible: false,
    crosshairMarkerVisible: false,
    priceLineVisible: false,
    priceScaleId: 'right',
  })

  chart.subscribeCrosshairMove(crosshairHandler)

  await fetchKline()
  updateLimitLines()
}

watch(period, () => { fetchKline(); updateLimitLines() })

onMounted(async () => {
  await fetchRealtime()
  initChart()
})

onUnmounted(() => {
  try { chart?.unsubscribeCrosshairMove(crosshairHandler) } catch { /* ignore */ }
  try { chart?.remove() } catch { /* ignore */ }
})
</script>
