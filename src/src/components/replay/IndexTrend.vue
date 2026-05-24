<template>
  <div class="space-y-2">
    <div v-for="idx in indices" :key="idx.key" class="border-b border-hairline-dark pb-2 last:border-b-0 last:pb-0">
      <div class="flex items-center justify-between mb-1">
        <span class="text-xs text-white/60">{{ idx.label }}</span>
        <div class="flex items-center gap-2">
          <span class="font-mono text-sm text-white">{{ idx.lastPx }}</span>
          <span class="font-mono text-xs" :class="idx.changeRate >= 0 ? 'text-up' : 'text-down'">
            {{ idx.changeRate >= 0 ? '+' : '' }}{{ idx.changeRate.toFixed(2) }}%
          </span>
        </div>
      </div>
      <div :ref="(el) => setChartRef(idx.key, el as HTMLElement)" class="w-full" style="height: 80px"></div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, watch, nextTick } from 'vue'
import { createChart, ColorType, LineSeries } from 'lightweight-charts'

interface IndexTick {
  tick_at: number
  close_px: number
}

interface IndexTrendData {
  sh: IndexTick[]
  sz: IndexTick[]
  cyb: IndexTick[]
}

const props = defineProps<{
  trendData: IndexTrendData | null
}>()

const chartRefs: Record<string, HTMLElement | null> = {}
const charts: Record<string, ReturnType<typeof createChart>> = {}
const seriesMap: Record<string, any> = {}

function setChartRef(key: string, el: HTMLElement | null) {
  chartRefs[key] = el
}

const indices = computed(() => {
  if (!props.trendData) return []
  return [
    { key: 'sh', label: '上证综指', ticks: props.trendData.sh, ...computeLast(props.trendData.sh) },
    { key: 'sz', label: '深证成指', ticks: props.trendData.sz, ...computeLast(props.trendData.sz) },
    { key: 'cyb', label: '创业板指', ticks: props.trendData.cyb, ...computeLast(props.trendData.cyb) },
  ]
})

function computeLast(ticks: IndexTick[]) {
  if (!ticks.length) return { lastPx: '--', changeRate: 0 }
  const last = ticks[ticks.length - 1].close_px
  const first = ticks[0].close_px
  const changeRate = first > 0 ? ((last - first) / first) * 100 : 0
  return { lastPx: last.toFixed(2), changeRate }
}

function initChart(key: string) {
  const el = chartRefs[key]
  if (!el || charts[key]) return

  const chart = createChart(el, {
    layout: {
      background: { type: ColorType.Solid, color: 'transparent' },
      textColor: 'rgba(255,255,255,0.3)',
      fontSize: 10,
    },
    grid: {
      vertLines: { color: 'rgba(255,255,255,0.04)' },
      horzLines: { color: 'rgba(255,255,255,0.04)' },
    },
    rightPriceScale: {
      visible: false,
      borderVisible: false,
    },
    timeScale: {
      visible: false,
      fixLeftEdge: true,
      fixRightEdge: true,
    },
    crosshair: { mode: 0 },
    handleScroll: false,
    handleScale: false,
  })

  const series = chart.addSeries(LineSeries, {
    lineWidth: 2,
    lastValueVisible: false,
    priceLineVisible: false,
    crosshairMarkerVisible: false,
  })

  charts[key] = chart
  seriesMap[key] = series
}

function updateCharts() {
  for (const idx of indices.value) {
    if (!charts[idx.key]) {
      initChart(idx.key)
    }
    const series = seriesMap[idx.key]
    if (!series || !idx.ticks.length) continue

    const data = idx.ticks.map((t) => ({
      time: t.tick_at as any,
      value: t.close_px,
    }))
    series.setData(data)

    const lastVal = idx.ticks[idx.ticks.length - 1].close_px
    const firstVal = idx.ticks[0].close_px
    const color = lastVal >= firstVal ? '#F5222D' : '#52C41A'
    series.applyOptions({
      color,
      topColor: color.replace(')', ',0.15)').replace('rgb', 'rgba'),
      bottomColor: 'rgba(0,0,0,0)',
    })
  }
}

watch(
  () => props.trendData,
  async () => {
    await nextTick()
    updateCharts()
  },
  { deep: true },
)

onMounted(async () => {
  await nextTick()
  updateCharts()
})

onUnmounted(() => {
  for (const key of Object.keys(charts)) {
    charts[key]?.remove()
    delete charts[key]
    delete seriesMap[key]
  }
})
</script>