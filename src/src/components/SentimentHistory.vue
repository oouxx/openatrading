<template>
  <div class="bg-bg-surface rounded-card border border-hairline p-card-inner">
    <h3 class="text-caption-strong text-ink-muted mb-4 uppercase tracking-wider">情绪历史走势（近10日）</h3>
    <div v-if="!data?.length" class="text-center py-8 text-caption text-ink-muted">暂无情绪历史数据</div>
    <div v-else ref="chartContainer" style="height: 200px"></div>
    <p v-if="data?.length" class="text-caption text-ink-muted mt-3">{{ interpretation }}</p>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch, nextTick } from 'vue'
import { createChart, LineSeries, type IChartApi, type Time } from 'lightweight-charts'

const props = defineProps<{
  data?: { date: string; value: number }[]
}>()

const chartContainer = ref<HTMLElement>()
const interpretation = ref('')
let chart: IChartApi | null = null

function renderChart() {
  if (!chartContainer.value || !props.data?.length) return

  if (chart) { chart.remove(); chart = null }

  chart = createChart(chartContainer.value, {
    width: chartContainer.value.clientWidth,
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

  const chartData = props.data.map((d) => ({ time: d.date as Time, value: d.value }))
  series.setData(chartData)

  // Mark today
  const last = props.data[props.data.length - 1]
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

  // Interpretation text
  const recent = props.data.slice(-3)
  if (recent.length >= 3 && recent.every(d => d.value >= 75)) {
    interpretation.value = '连续3日偏热区间，历史规律显示此后2~3日有回调风险，注意控仓'
  } else if (recent.length >= 3 && recent.every(d => d.value >= 50)) {
    interpretation.value = '情绪正常偏暖，短线操作环境尚可'
  } else if (props.data.length > 0 && props.data[props.data.length - 1].value < 50) {
    interpretation.value = '情绪偏冷，等待回暖信号'
  } else {
    interpretation.value = ''
  }

  chart.timeScale().fitContent()
}

onMounted(() => { nextTick(() => renderChart()) })
onUnmounted(() => { chart?.remove(); chart = null })

watch(() => props.data, () => { nextTick(() => renderChart()) }, { deep: true })
</script>