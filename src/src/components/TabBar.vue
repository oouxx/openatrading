<template>
  <div class="glow-card overflow-hidden">
    <div class="flex" style="border-bottom: 1px solid var(--border-subtle)">
      <button
        v-for="tab in tabs"
        :key="tab.id"
        @click="activeTab = tab.id"
        class="tab-btn"
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
  { id: 'limit-up', label: '涨停榜' },
  { id: 'plate', label: '板块异动' },
  { id: 'sentiment', label: '舆情分析' },
  { id: 'ai', label: 'AI 观点' },
  { id: 'tgb', label: '淘股吧' },
]
</script>
