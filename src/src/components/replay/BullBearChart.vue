<template>
  <div ref="chartRef" style="height: 120px"></div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch, computed } from 'vue'
import * as echarts from 'echarts'

const props = defineProps<{
  bullCount: number
  bearCount: number
}>()

const chartRef = ref<HTMLElement>()
let instance: echarts.ECharts | null = null

const ratio = computed(() => {
  const total = props.bullCount + props.bearCount
  if (total === 0) return '--'
  return (props.bullCount / total * 100).toFixed(0) + '%'
})

function buildOption() {
  return {
    grid: { top: 8, right: 8, bottom: 8, left: 8 },
    xAxis: { type: 'category', data: ['多方', '空方'], show: false },
    yAxis: { show: false },
    series: [
      {
        type: 'bar',
        data: [
          {
            value: props.bullCount,
            itemStyle: { color: '#F5222D', borderRadius: [4, 4, 0, 0] },
          },
          {
            value: props.bearCount,
            itemStyle: { color: '#52C41A', borderRadius: [4, 4, 0, 0] },
          },
        ],
        barWidth: 40,
        barGap: '30%',
        label: {
          show: true,
          position: 'top',
          fontSize: 16,
          fontWeight: 'bold',
          formatter: (p: any) => {
            const label = p.dataIndex === 0 ? '多' : '空'
            return `${label} ${p.value}`
          },
          color: '#fff',
        },
      },
    ],
    graphic: [
      {
        type: 'text',
        right: 8,
        top: 4,
        style: {
          text: `多空比 ${ratio.value}`,
          fontSize: 11,
          fill: 'rgba(255,255,255,0.4)',
        },
      },
    ],
  }
}

function update() {
  if (!instance) return
  instance.setOption(buildOption(), true)
}

const onResize = () => instance?.resize()

onMounted(() => {
  if (!chartRef.value) return
  instance = echarts.init(chartRef.value)
  update()
  window.addEventListener('resize', onResize)
})

onUnmounted(() => {
  window.removeEventListener('resize', onResize)
  instance?.dispose()
})

watch(() => [props.bullCount, props.bearCount], update)
</script>
