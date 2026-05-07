<template>
  <div class="glow-card overflow-hidden">
    <div class="flex overflow-x-auto" style="border-bottom: 1px solid var(--border-subtle)">
      <button
        v-for="tab in tabs"
        :key="tab.id"
        @click="activeTab = tab.id"
        class="tab-btn flex-shrink-0"
        :class="{ active: activeTab === tab.id }"
      >
        {{ tab.label }}
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
const activeTab = defineModel<string>({ default: 'limit-up' })

const tabs = [
  { id: 'limit-up', label: '📈 涨停数据' },
  { id: 'plate', label: '🗺 板块异动' },
  { id: 'promotion', label: '🏆 晋级' },
  { id: 'timeline', label: '📅 时间线' },
  { id: 'sentiment', label: '📰 舆情分析' },
  { id: 'ai', label: '🤖 AI 观点' },
  { id: 'tgb', label: '📝 淘股吧' },
]
</script>
