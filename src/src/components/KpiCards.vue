<template>
  <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4 mb-6">
    <div class="bg-white rounded-xl shadow p-4 border-l-4 border-green-500">
      <div class="flex justify-between items-start">
        <div>
          <p class="text-gray-500 text-sm">📈 上涨家数</p>
          <p class="text-3xl font-bold mt-2">{{ overview.rise_count }}</p>
        </div>
        <span class="text-green-500 text-sm">↑ 环比+5%</span>
      </div>
    </div>

    <div class="bg-white rounded-xl shadow p-4 border-l-4 border-red-500">
      <div class="flex justify-between items-start">
        <div>
          <p class="text-gray-500 text-sm">📉 下跌家数</p>
          <p class="text-3xl font-bold mt-2">{{ overview.fall_count }}</p>
        </div>
        <span class="text-red-500 text-sm">↓ 环比-2%</span>
      </div>
    </div>

    <div class="bg-white rounded-xl shadow p-4 border-l-4 border-yellow-500">
      <div class="flex justify-between items-start">
        <div>
          <p class="text-gray-500 text-sm">📊 涨跌比</p>
          <p class="text-3xl font-bold mt-2" :class="bombRateColor">{{ ratio }}</p>
          <p class="text-xs text-gray-400 mt-1">{{ overview.limit_up_count }}涨停 / {{ overview.limit_down_count }}跌停</p>
        </div>
        <span class="text-gray-500 text-sm">今日</span>
      </div>
    </div>

    <div class="bg-white rounded-xl shadow p-4 border-l-4 border-yellow-500">
      <div class="flex justify-between items-start">
        <div>
          <p class="text-gray-500 text-sm">⚠️ 炸板率</p>
          <p class="text-3xl font-bold mt-2" :class="bombRateColor">{{ (overview.bomb_rate * 100).toFixed(1) }}%</p>
        </div>
        <span class="text-gray-500 text-sm">正常范围</span>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'

const props = defineProps<{
  overview: {
    rise_count: number
    fall_count: number
    limit_up_count: number
    limit_down_count: number
    bomb_rate: number
  }
}>()

const bombRateColor = computed(() => {
  if (props.overview.bomb_rate > 0.3) return 'text-red-600'
  if (props.overview.bomb_rate > 0.2) return 'text-yellow-600'
  return 'text-green-600'
})

const ratio = computed(() => {
  if (props.overview.fall_count === 0) return '∞'
  return (props.overview.rise_count / props.overview.fall_count).toFixed(2)
})
</script>
