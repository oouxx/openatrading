<template>
  <div>
    <h2 class="text-xl font-bold mb-4">🗺️ 板块异动</h2>
    
    <!-- 板块热度排行 (CSS Bar) -->
    <div class="mb-6">
      <h3 class="text-lg font-medium mb-2">板块热度</h3>
      <div class="space-y-2">
        <div v-for="(item, idx) in plateHeat" :key="idx" class="flex items-center gap-3">
          <span class="w-24 text-sm text-gray-600">{{ item.name }}</span>
          <div class="flex-1 bg-gray-200 rounded-full h-6 overflow-hidden">
            <div 
              class="h-full bg-blue-500 rounded-full transition-all duration-500"
              :style="{ width: item.percent + '%' }"
            ></div>
          </div>
          <span class="w-12 text-sm text-right text-gray-700">{{ item.count }}</span>
        </div>
      </div>
    </div>

    <!-- 异动事件列表 -->
    <div>
      <h3 class="text-lg font-medium mb-2">异动事件</h3>
      <div class="space-y-3">
        <div 
          v-for="(event, idx) in events" 
          :key="idx"
          class="border-l-4 border-blue-500 pl-4 py-2"
        >
          <div class="flex items-center gap-2 mb-1">
            <span class="text-sm font-medium text-blue-600">{{ event.plate_name }}</span>
            <span class="text-xs text-gray-500">{{ event.time }}</span>
          </div>
          <p class="text-sm text-gray-700">{{ event.description }}</p>
          <div class="flex gap-1 mt-1">
            <span 
              v-for="code in event.related_stocks" 
              :key="code"
              class="px-2 py-0.5 bg-gray-100 rounded text-xs font-mono"
            >
              {{ code }}
            </span>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useReportStore } from '../../stores/report'

const store = useReportStore()

const events = computed(() => store.report?.plate_abnormal || [])

const plateHeat = computed(() => {
  const counts: Record<string, number> = {}
  events.value.forEach(e => {
    counts[e.plate_name] = (counts[e.plate_name] || 0) + 1
  })
  const max = Math.max(...Object.values(counts), 1)
  return Object.entries(counts)
    .map(([name, count]) => ({ 
      name, 
      count, 
      percent: (count / max) * 100 
    }))
    .sort((a, b) => b.count - a.count)
})
</script>
