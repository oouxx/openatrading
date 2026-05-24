<template>
  <div ref="chartContainer" class="w-full" style="height: 280px"></div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch } from 'vue'
import { createChart, ColorType, LineSeries } from 'lightweight-charts'

const props = defineProps<{
  snapshots: Array<{ timestamp: number; time_label: string; emotion_score: number }>
  currentTime: number
}>()

const chartContainer = ref<HTMLElement>()
let chart: ReturnType<typeof createChart> | null = null
let lineSeries: any = null

function emotionColor(score: number): string {
  if (score > 60) return '#F5222D'
  if (score < 40) return '#52C41A'
  return '#FAAD14'
}

function initChart() {
  if (!chartContainer.value) return
  chart = createChart(chartContainer.value, {
    layout: {
      background: { type: ColorType.Solid, color: 'transparent' },
      textColor: 'rgba(255,255,255,0.4)',
    },
    grid: {
      vertLines: { color: 'rgba(255,255,255,0.06)' },
      horzLines: { color: 'rgba(255,255,255,0.06)' },
    },
    rightPriceScale: {
      visible: true,
      borderVisible: false,
      scaleMargins: { top: 0.1, bottom: 0.1 },
    },
    timeScale: { timeVisible: false, fixLeftEdge: true, fixRightEdge: true },
    crosshair: { mode: 0 },
    handleScroll: false,
    handleScale: false,
  })
  chart.timeScale().applyOptions({ visible: false })

  lineSeries = chart.addSeries(LineSeries, {
    lineWidth: 2,
    color: '#FAAD14',
    lastValueVisible: false,
    priceLineVisible: false,
  })
}

function updateData() {
  if (!lineSeries) return
  const toShow =
    props.currentTime > 0
      ? props.snapshots.filter((s) => s.timestamp <= props.currentTime)
      : props.snapshots

  if (toShow.length === 0) return

  const data = toShow.map((s) => ({
    time: s.timestamp as any,
    value: s.emotion_score,
    color: emotionColor(s.emotion_score),
  }))
  lineSeries.setData(data)

  const lastColor = emotionColor(toShow[toShow.length - 1].emotion_score)
  lineSeries.applyOptions({ color: lastColor })
}

onMounted(() => {
  initChart()
  updateData()
})

onUnmounted(() => {
  chart?.remove()
})

watch(() => [props.snapshots, props.currentTime], updateData, { deep: true })
</script>