<template>
  <div ref="chartRef" style="height: 120px"></div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch } from 'vue'
import * as echarts from 'echarts'

const props = defineProps<{
  bullCount: number
  bearCount: number
}>()

const chartRef = ref<HTMLElement>()
let instance: echarts.ECharts | null = null

function buildOption() {
  return {
    grid: { top: 8, right: 8, bottom: 8, left: 8 },
    xAxis: { show: false },
    yAxis: { show: false },
    series: [
      {
        type: 'bar',
        data: [
          {
            value: props.bullCount,
            itemStyle: { color: '#F5222D', borderRadius: [4, 4, 0, 0] },
          },
        ],
        barWidth: 40,
        label: {
          show: true,
          position: 'top',
          color: '#F5222D',
          fontSize: 14,
          fontWeight: 'bold',
          formatter: () => `多 ${props.bullCount}`,
        },
      },
      {
        type: 'bar',
        data: [
          {
            value: props.bearCount,
            itemStyle: { color: '#52C41A', borderRadius: [4, 4, 0, 0] },
          },
        ],
        barWidth: 40,
        barGap: '20%',
        label: {
          show: true,
          position: 'top',
          color: '#52C41A',
          fontSize: 14,
          fontWeight: 'bold',
          formatter: () => `空 ${props.bearCount}`,
        },
      },
    ],
  }
}

function update() {
  if (!instance) return
  instance.setOption(buildOption(), true)
}

onMounted(() => {
  if (!chartRef.value) return
  instance = echarts.init(chartRef.value)
  update()
  window.addEventListener('resize', () => instance?.resize())
})

onUnmounted(() => {
  instance?.dispose()
})

watch(() => [props.bullCount, props.bearCount], update)
</script>
