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
          <span class="text-sm font-mono" :style="{ color: quote.px_change_rate >= 0 ? 'var(--red)' : 'var(--green)' }">{{ quote.px_change_rate >= 0 ? '+' : '' }}{{ (quote.px_change_rate * 100).toFixed(2) }}%</span>
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

      <!-- Chart -->
      <div ref="chartContainer" class="w-full" style="height: 360px"></div>

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
import { createChart, ColorType } from 'lightweight-charts'

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

let chart: any = null
let candleSeries: any = null

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
    { label: '振幅', value: quote.value.amplitude ? (quote.value.amplitude * 100).toFixed(2) + '%' : '-' },
    { label: '换手率', value: quote.value.turnover_ratio ? (quote.value.turnover_ratio * 100).toFixed(2) + '%' : '-' },
    { label: '动态PE', value: quote.value.dyn_pe ? quote.value.dyn_pe.toFixed(1) : '-' },
    { label: '市净率', value: quote.value.dyn_pb_rate ? quote.value.dyn_pb_rate.toFixed(2) : '-' },
    { label: '总市值', value: formatVolume(quote.value.market_value) },
    { label: '流通市值', value: formatVolume(quote.value.circulation_value) },
    { label: '量比', value: quote.value.volume_ratio?.toFixed(2) || '-' },
    { label: '成交量', value: formatVolume(quote.value.turnover_volume) + '股' },
  ]
})

function getApiBase(): string {
  return localStorage.getItem('trading-agent-api-url') || 'http://localhost:8080'
}

async function fetchRealtime() {
  try {
    const resp = await fetch(`${getApiBase()}/api/realtime/${props.symbol}`)
    if (resp.ok) quote.value = await resp.json()
  } catch { /* ignore */ }
}

async function fetchKline() {
  try {
    const resp = await fetch(`${getApiBase()}/api/kline/${props.symbol}?period=${period.value}&count=256`)
    if (!resp.ok) return
    const data = await resp.json()
    candleSeries?.setData(data.candles.map((c: any) => ({
      time: c.time as number,
      open: c.open,
      close: c.close,
      high: c.high,
      low: c.low,
    })))
  } catch { /* ignore */ }
}

async function initChart() {
  if (!chartContainer.value) return
  if (chart) { chart.remove(); chart = null }

  chart = createChart(chartContainer.value, {
    layout: {
      background: { type: ColorType.Solid, color: '#111827' },
      textColor: '#8899aa',
    },
    grid: { vertLines: { color: '#1e2a3a' }, horzLines: { color: '#1e2a3a' } },
    width: chartContainer.value.clientWidth || 520,
    height: 360,
    crosshair: { mode: 0 },
    timeScale: { borderColor: '#1e2a3a', timeVisible: true },
    rightPriceScale: { borderColor: '#1e2a3a' },
  })

  candleSeries = chart.addCandlestickSeries({
    upColor: '#00e676',
    downColor: '#ff1744',
    borderUpColor: '#00e676',
    borderDownColor: '#ff1744',
    wickUpColor: '#00e676',
    wickDownColor: '#ff1744',
  })

  await fetchKline()
}

watch(period, () => fetchKline())

onMounted(async () => {
  await fetchRealtime()
  initChart()
})

onUnmounted(() => {
  try { chart?.remove() } catch { /* ignore */ }
})
</script>
