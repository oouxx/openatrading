<template>
  <div>
    <h2 class="text-xl font-bold mb-4">📈 涨停榜</h2>
    
    <!-- 热门板块标签云 -->
    <div class="mb-6">
      <h3 class="text-lg font-medium mb-2">热门板块</h3>
      <div class="flex flex-wrap gap-2">
        <span 
          v-for="(plate, idx) in plates" 
          :key="idx"
          class="px-3 py-1 rounded-full text-sm"
          :style="{ fontSize: (12 + idx * 2) + 'px', backgroundColor: getPlateColor(idx) }"
        >
          {{ plate.name }} ({{ plate.count }})
        </span>
      </div>
    </div>

    <!-- 风险提示 -->
    <div 
      v-if="bombRate > 0.3" 
      class="bg-red-50 border-l-4 border-red-500 p-3 mb-4 rounded"
    >
      <p class="text-red-700 font-medium">⚠️ 警告：炸板率高达 {{ (bombRate * 100).toFixed(1) }}%，市场风险较高！</p>
    </div>

    <!-- 连板梯队表格 -->
    <div>
      <h3 class="text-lg font-medium mb-2">连板梯队</h3>
      <table class="w-full text-sm">
        <thead>
          <tr class="bg-gray-50">
            <th class="px-4 py-2 text-left">连板天数</th>
            <th class="px-4 py-2 text-left">代码</th>
            <th class="px-4 py-2 text-left">名称</th>
            <th class="px-4 py-2 text-right">涨跌幅</th>
            <th class="px-4 py-2 text-right">换手率</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="stock in stocks" :key="stock.symbol" class="border-t hover:bg-gray-50">
            <td class="px-4 py-2">
              <span class="inline-block px-2 py-1 bg-blue-100 text-blue-800 rounded text-xs">
                {{ stock.limit_up_days }}板
              </span>
            </td>
            <td class="px-4 py-2 font-mono">{{ stock.symbol }}</td>
            <td class="px-4 py-2 font-medium">{{ stock.stock_name }}</td>
            <td class="px-4 py-2 text-right text-green-600">+{{ stock.change_percent.toFixed(2) }}%</td>
            <td class="px-4 py-2 text-right">{{ stock.turnover_ratio.toFixed(2) }}%</td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useReportStore } from '../../stores/report'

const store = useReportStore()

const stocks = computed(() => store.report?.limit_up || [])
const bombRate = computed(() => store.report?.overview.bomb_rate || 0)

const plates = computed(() => {
  const counts: Record<string, number> = {}
  store.report?.plate_abnormal.forEach(e => {
    counts[e.plate_name] = (counts[e.plate_name] || 0) + 1
  })
  return Object.entries(counts).map(([name, count]) => ({ name, count })).sort((a, b) => b.count - a.count)
})

const getPlateColor = (idx: number) => {
  const colors = ['#3B82F6', '#60A5FA', '#93C5FD', '#BFDBFE']
  return colors[idx % colors.length]
}
</script>
