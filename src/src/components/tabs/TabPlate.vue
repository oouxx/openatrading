<template>
  <div>
    <h2 class="text-xl font-bold mb-4">🗺️ 板块异动</h2>
    <div class="mb-6">
      <h3 class="text-lg font-medium mb-2">板块热度</h3>
      <div class="space-y-2">
        <div v-for="(item, idx) in plateHeat" :key="idx" class="flex items-center gap-3">
          <span class="w-24 text-sm text-gray-600 truncate">{{ item.name }}</span>
          <div class="flex-1 bg-gray-200 rounded-full h-6 overflow-hidden">
            <div class="h-full bg-blue-500 rounded-full transition-all duration-500" :style="{ width: item.percent + '%' }"></div>
          </div>
          <span class="w-12 text-sm text-right text-gray-700">{{ item.count }}</span>
        </div>
      </div>
    </div>
    <div>
      <h3 class="text-lg font-medium mb-2">异动事件</h3>
      <div v-if="sortedEvents.length === 0" class="text-gray-400 py-8 text-center">暂无板块异动数据</div>
      <div class="space-y-3">
        <div v-for="event in sortedEvents" :key="event.id" class="border-l-4 border-blue-500 pl-4 py-2">
          <div class="flex items-center gap-2 mb-1">
            <span class="text-sm font-medium text-blue-600">{{ event.plate_name || '未知板块' }}</span>
            <span class="text-xs text-gray-500">{{ formatTime(event.event_timestamp) }}</span>
            <span class="text-xs px-1.5 py-0.5 rounded" :class="event.pcp >= 0 ? 'bg-red-100 text-red-600' : 'bg-green-100 text-green-600'">{{ formatPcp(event.pcp) }}</span>
          </div>
          <div class="flex gap-1 mt-1 flex-wrap">
            <span v-for="stock in event.stocks" :key="stock.symbol" class="px-2 py-0.5 bg-gray-100 rounded text-xs font-mono">{{ stock.symbol }} {{ stock.name }}</span>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useReportStore } from '../../../stores/report'

const store = useReportStore()

interface FlatEvent {
  id: number
  plate_name: string
  pcp: number
  event_timestamp: number
  stocks: { symbol: string; name: string }[]
}

const flatEvents = computed<FlatEvent[]>(() => {
  const events: FlatEvent[] = []
  store.report?.plate_abnormal.forEach(e => {
    const d = e.plate_abnormal_event_data
    if (!d) return
    events.push({
      id: e.id,
      plate_name: d.plate_name || '未知板块',
      pcp: d.pcp ?? 0,
      event_timestamp: e.event_timestamp,
      stocks: d.related_stocks.map(s => ({ symbol: s.symbol, name: s.name })),
    })
  })
  return events
})

const sortedEvents = computed(() =>
  [...flatEvents.value].sort((a, b) => b.event_timestamp - a.event_timestamp)
)

const plateHeat = computed(() => {
  const counts: Record<string, number> = {}
  flatEvents.value.forEach(e => { counts[e.plate_name] = (counts[e.plate_name] || 0) + 1 })
  const max = Math.max(...Object.values(counts), 1)
  return Object.entries(counts).map(([name, count]) => ({ name, count, percent: (count / max) * 100 })).sort((a, b) => b.count - a.count).slice(0, 10)
})

function formatTime(ts: number): string {
  const d = new Date(ts * 1000)
  return d.toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' })
}

function formatPcp(pcp: number): string {
  if (pcp === 0) return '0%'
  return (pcp > 0 ? '+' : '') + (pcp * 100).toFixed(1) + '%'
}
</script>
