<template>
  <div class="flex overflow-x-auto gap-1" style="padding: 12px 0">
    <button
      v-for="tab in tabs"
      :key="tab.id"
      @click="activeTab = tab.id"
      class="flex-shrink-0 px-4 transition-colors duration-200 border-b-2 cursor-pointer"
      style="padding-top: 10px; padding-bottom: 10px"
      :class="activeTab === tab.id
        ? 'text-white border-ai'
        : 'text-ink-muted border-transparent hover:text-white/80'"
    >
      <span class="text-caption">{{ tab.label }}</span>
    </button>
  </div>
</template>

<script setup lang="ts">
const activeTab = defineModel<string>({ default: 'strong-stocks' })

const tabs = [
  { id: 'strong-stocks', label: '强势股' },
  { id: 'promotion', label: '晋级分析' },
  { id: 'plate', label: '板块异动' },
  { id: 'timeline', label: '时间线' },
  { id: 'sentiment-chip', label: '情绪筹码' },
  { id: 'market-intel', label: '市场情报' },
]
</script>
