<template>
  <div class="bg-white rounded-xl shadow">
    <div class="flex border-b">
      <button
        v-for="tab in tabs"
        :key="tab.id"
        @click="activeTab = tab.id"
        class="px-6 py-3 font-medium text-sm transition"
        :class="activeTab === tab.id ? 'text-blue-600 border-b-2 border-blue-600' : 'text-gray-500 hover:text-gray-700'"
      >
        {{ tab.label }}
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'

const activeTab = defineModel<string>({ default: 'limit-up' })

const tabs = [
  { id: 'limit-up', label: '📈 涨停榜' },
  { id: 'plate', label: '🗺️ 板块异动' },
  { id: 'sentiment', label: '📰 舆情分析' },
  { id: 'ai', label: '🤖️ AI 观点' },
  { id: 'tgb', label: '📝 淘股吧' }
]
</script>
