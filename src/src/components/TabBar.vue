<template>
  <div class="flex overflow-x-auto gap-1" style="padding: 12px 0">
    <button
      v-for="tab in tabs"
      :key="tab.id"
      @click="activeTab = tab.id"
      class="flex-shrink-0 px-4 transition-colors duration-200 border-b-2 cursor-pointer"
      style="padding-top: 10px; padding-bottom: 10px"
      :class="activeTab === tab.id
        ? 'text-ink border-primary'
        : 'text-ink-muted border-transparent hover:text-ink/70'"
    >
      <span class="text-caption">{{ tab.label }}</span>
    </button>
  </div>
</template>

<script setup lang="ts">
const activeTab = defineModel<string>({ default: 'limit-up' })

const tabs = [
  { id: 'limit-up', label: '涨停数据' },
  { id: 'plate', label: '板块异动' },
  { id: 'promotion', label: '晋级' },
  { id: 'timeline', label: '时间线' },
  { id: 'sentiment', label: '舆情分析' },
  { id: 'ai', label: 'AI 观点' },
  { id: 'tgb', label: '淘股吧' },
]
</script>
