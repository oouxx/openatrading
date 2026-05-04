<template>
  <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4 mb-6">
    <div class="bg-white rounded-xl shadow p-4 border-l-4 border-green-500">
      <div class="flex justify-between items-start">
        <div>
          <p class="text-gray-500 text-sm">📈 涨停家数</p>
          <p class="text-3xl font-bold mt-2">{{ overview.rise_count }}</p>
        </div>
        <span class="text-green-500 text-sm">↑ 环比+5%</span>
      </div>
    </div>

    <div class="bg-white rounded-xl shadow p-4 border-l-4 border-red-500">
      <div class="flex justify-between items-start">
        <div>
          <p class="text-gray-500 text-sm">📉 跌停家数</p>
          <p class="text-3xl font-bold mt-2">{{ overview.fall_count }}</p>
        </div>
        <span class="text-red-500 text-sm">↓ 环比-2%</span>
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

    <div class="bg-white rounded-xl shadow p-4 border-l-4" :class="sentimentBorder">
      <div class="flex justify-between items-start">
        <div>
          <p class="text-gray-500 text-sm">🎯 市场情绪</p>
          <p class="text-2xl font-bold mt-2 flex items-center gap-2">
            {{ overview.sentiment }}
            <span class="w-3 h-3 rounded-full" :class="sentimentDot"></span>
          </p>
        </div>
        <span class="text-gray-500 text-sm">今日</span>
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
    bomb_rate: number
    sentiment: string
  }
}>()

const bombRateColor = computed(() => {
  if (props.overview.bomb_rate > 0.3) return 'text-red-600'
  if (props.overview.bomb_rate > 0.2) return 'text-yellow-600'
  return 'text-green-600'
})

const sentimentBorder = computed(() => {
  if (props.overview.sentiment === '高') return 'border-red-500'
  if (props.overview.sentiment === '中') return 'border-yellow-500'
  return 'border-green-500'
})

const sentimentDot = computed(() => {
  if (props.overview.sentiment === '高') return 'bg-red-500'
  if (props.overview.sentiment === '中') return 'bg-yellow-500'
  return 'bg-green-500'
})
</script>
