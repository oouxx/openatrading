<template>
  <div ref="chartRef" class="w-full" style="height: 200px"></div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch } from 'vue'
import * as echarts from 'echarts'

interface RelatedStock {
  symbol: string
  name: string
  mtm: number
  pcp: number
}

interface TopPlate {
  plate_name: string
  pcp: number
  good_or_bad: number
  related_stocks: RelatedStock[]
}

const props = defineProps<{
  topPlates: TopPlate[]
}>()

const chartRef = ref<HTMLElement>()
let instance: echarts.ECharts | null = null
let prevPlateNames: string[] = []

function buildOption() {
  const plates = props.topPlates
  if (plates.length === 0) return {}

  const data: Array<{
    name: string
    value: number
    pcp: number
    good_or_bad: number
    related_stocks: RelatedStock[]
    isNew: boolean
  }> = plates.map((p) => ({
    name: p.plate_name,
    value: Math.abs(p.pcp),
    pcp: p.pcp,
    good_or_bad: p.good_or_bad,
    related_stocks: p.related_stocks,
    isNew: !prevPlateNames.includes(p.plate_name),
  }))

  const maxPcp = Math.max(...plates.map((p) => Math.abs(p.pcp)), 0.001)
  const newNames = data.filter((d) => d.isNew).map((d) => d.name)
  prevPlateNames = plates.map((p) => p.plate_name)

  return {
    tooltip: {
      formatter: (p: any) => {
        const d = p.data
        if (!d) return ''
        const tag = d.good_or_bad === 1 ? '利好' : d.good_or_bad === 2 ? '利空' : '中性'
        const pcpStr = (d.pcp * 100).toFixed(2)
        let html = `<b>${d.name}</b><br/>涨跌幅: ${pcpStr}%<br/>多空: ${tag}`
        if (d.related_stocks && d.related_stocks.length > 0) {
          const stocks = d.related_stocks
            .slice(0, 5)
            .map((s: RelatedStock) => `${s.name}(${(s.pcp * 100).toFixed(1)}%)`)
            .join('、')
          html += `<br/>关联个股: ${stocks}`
        }
        return html
      },
    },
    series: [
      {
        type: 'treemap',
        roam: false,
        nodeClick: false as const,
        breadcrumb: { show: false },
        label: {
          show: true,
          formatter: (p: any) => {
            const d = p.data
            const tag = d.good_or_bad === 1 ? '多' : d.good_or_bad === 2 ? '空' : ''
            return `${d.name}\n${(d.pcp * 100).toFixed(1)}% ${tag}`
          },
          fontSize: 11,
          color: '#fff',
        },
        data,
        levels: [
          {
            itemStyle: { borderColor: '#0D1117', borderWidth: 2, gapWidth: 1 },
          },
          {
            colorMappingBy: 'value' as const,
            color: data.map((d) =>
              d.pcp >= 0
                ? `rgba(245, 34, 45, ${0.3 + (Math.abs(d.pcp) / maxPcp) * 0.7})`
                : `rgba(82, 196, 26, ${0.3 + (Math.abs(d.pcp) / maxPcp) * 0.7})`,
            ),
          },
        ],
        animationDurationUpdate: 600,
        animationEasingUpdate: 'cubicInOut',
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

watch(() => props.topPlates, update, { deep: true })
</script>