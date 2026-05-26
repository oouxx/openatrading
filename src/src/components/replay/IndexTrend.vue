<template>
  <div class="h-full flex flex-col">
    <div v-for="idx in indices" :key="idx.key" class="flex-1 flex flex-col min-h-0 border-b border-hairline-dark pb-1 last:border-b-0 last:pb-0">
      <div class="flex items-center justify-between px-1 shrink-0">
        <span class="text-[10px] text-white/60">{{ idx.label }}</span>
        <div class="flex items-center gap-2">
          <span class="font-mono text-xs text-white">{{ idx.lastPx }}</span>
          <span class="font-mono text-[10px]" :class="idx.changeRate >= 0 ? 'text-up' : 'text-down'">
            {{ idx.changeRate >= 0 ? '+' : '' }}{{ idx.changeRate.toFixed(2) }}%
          </span>
        </div>
      </div>
      <div :ref="(el) => setChartRef(idx.key, el as HTMLElement)" class="w-full flex-1 min-h-0"></div>
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

/** Filter ticks to A-share market hours (09:30–15:00 Beijing = 01:30–07:00 UTC). */
function filterMarketHours(ticks: IndexTick[]): IndexTick[] {
  if (!ticks.length) return ticks
  const dayStart = ticks[0].tick_at - (ticks[0].tick_at % 86400)
  const openCutoff = dayStart + 5400 // 01:30 UTC = 09:30 Beijing
  const closeCutoff = dayStart + 7 * 3600 // 07:00 UTC = 15:00 Beijing
  return ticks.filter((t) => t.tick_at >= openCutoff && t.tick_at <= closeCutoff)
}

const indices = computed(() => {
  if (!props.trendData) return []
  const sh = filterMarketHours(props.trendData.sh)
  const sz = filterMarketHours(props.trendData.sz)
  const cyb = filterMarketHours(props.trendData.cyb)
  return [
    { key: 'sh', label: '上证综指', ticks: sh, ...computeLast(sh) },
    { key: 'sz', label: '深证成指', ticks: sz, ...computeLast(sz) },
    { key: 'cyb', label: '创业板指', ticks: cyb, ...computeLast(cyb) },
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
    width: el.clientWidth,
    height: el.clientHeight,
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
      visible: true,
      borderVisible: false,
      scaleMargins: { top: 0.1, bottom: 0.1 },
      entireTextOnly: true,
    },
    timeScale: {
      visible: true,
      fixLeftEdge: true,
      fixRightEdge: true,
      timeVisible: true,
      secondsVisible: false,
      tickMarkFormatter: (t: any) => {
        const secs = ((t as number) % 86400 + 86400 + 8 * 3600) % 86400
        const h = Math.floor(secs / 3600)
        const m = Math.floor((secs % 3600) / 60)
        return `${String(h).padStart(2, '0')}:${String(m).padStart(2, '0')}`
      },
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

    const chartObj = charts[idx.key]
    const container = chartRefs[idx.key]
    if (container) {
      chartObj.resize(container.clientWidth, container.clientHeight)
    }
    const firstTick = idx.ticks[0].tick_at
    const lastTick = idx.ticks[idx.ticks.length - 1].tick_at
    chartObj.timeScale().setVisibleRange({ from: firstTick, to: lastTick })

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

let resizeObserver: ResizeObserver | null = null

onMounted(async () => {
  await nextTick()
  updateCharts()
  resizeObserver = new ResizeObserver(() => updateCharts())
  for (const key of Object.keys(chartRefs)) {
    const el = chartRefs[key]
    if (el) resizeObserver.observe(el)
  }
})

onUnmounted(() => {
  resizeObserver?.disconnect()
  for (const key of Object.keys(charts)) {
    charts[key]?.remove()
    delete charts[key]
    delete seriesMap[key]
  }
})
</script>