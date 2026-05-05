<template>
  <div>
    <div class="flex items-center gap-4 mb-6 text-sm">
      <span style="color: var(--text-muted)">
        <b style="color: var(--text-primary)">{{ flatEvents.length }}</b> 条异动事件
      </span>
    </div>

    <!-- Heat Map -->
    <div class="glow-card p-5 mb-6">
      <h3 class="text-xs font-medium mb-4" style="color: var(--text-muted); letter-spacing: 0.5px">板块热度</h3>
      <div class="space-y-2">
        <div v-for="(item, idx) in plateHeat" :key="idx" class="flex items-center gap-3">
          <span class="w-24 text-xs truncate" style="color: var(--text-secondary)">{{ item.name }}</span>
          <div class="flex-1 h-5 rounded-full overflow-hidden" style="background: var(--border-subtle)">
            <div class="h-full rounded-full transition-all duration-700"
              :style="{ width: item.percent + '%', background: 'linear-gradient(90deg, var(--blue-dim), var(--blue))' }"
            ></div>
          </div>
          <span class="w-10 text-xs text-right font-mono" style="color: var(--text-secondary)">{{ item.count }}</span>
        </div>
        <div v-if="plateHeat.length === 0" class="text-center py-8 text-sm" style="color: var(--text-muted)">暂无数据</div>
      </div>
    </div>

    <!-- Event Timeline -->
    <div class="space-y-3">
      <div v-for="event in sortedEvents" :key="event.id" class="glow-card pl-4 py-3 pr-4 flex gap-4 items-start">
        <div class="w-1 self-stretch rounded-full flex-shrink-0"
          :style="{ background: event.pcp >= 0 ? 'var(--red)' : 'var(--green)' }"></div>
        <div class="flex-1 min-w-0">
          <div class="flex items-center gap-2 mb-1 flex-wrap">
            <span class="text-sm font-medium" style="color: var(--text-primary)">{{ event.plate_name || '未知板块' }}</span>
            <span class="text-xs font-mono" :style="{ color: event.pcp >= 0 ? 'var(--red)' : 'var(--green)' }">{{ formatPcp(event.pcp) }}</span>
          </div>
          <div class="flex gap-1 flex-wrap mt-2">
            <span v-for="stock in event.stocks" :key="stock.symbol"
              class="px-2 py-0.5 rounded text-xs font-mono"
              style="background: var(--border-subtle); color: var(--text-secondary)">
              {{ stock.symbol }} {{ stock.name }}
            </span>
          </div>
        </div>
        <span class="text-xs flex-shrink-0 font-mono" style="color: var(--text-muted)">{{ formatTime(event.event_timestamp) }}</span>
      </div>
      <div v-if="sortedEvents.length === 0" class="text-center py-12 text-sm" style="color: var(--text-muted)">暂无板块异动数据</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useReportStore } from '@/stores/report'

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
  store.report?.plate_abnormal.forEach((e: any) => {
    const d = e.plate_abnormal_event_data
    if (!d) return
    events.push({
      id: e.id,
      plate_name: d.plate_name || '未知板块',
      pcp: d.pcp ?? 0,
      event_timestamp: e.event_timestamp,
      stocks: (d.related_stocks || []).map((s: any) => ({ symbol: s.symbol, name: s.name })),
    })
  })
  return events
})

const sortedEvents = computed(() => [...flatEvents.value].sort((a, b) => b.event_timestamp - a.event_timestamp))

const plateHeat = computed(() => {
  const counts: Record<string, number> = {}
  flatEvents.value.forEach(e => { counts[e.plate_name] = (counts[e.plate_name] || 0) + 1 })
  const max = Math.max(...Object.values(counts), 1)
  return Object.entries(counts)
    .map(([name, count]) => ({ name, count, percent: Math.round((count / max) * 100) }))
    .sort((a, b) => b.count - a.count).slice(0, 10)
})

function formatTime(ts: number): string {
  return new Date(ts * 1000).toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' })
}
function formatPcp(pcp: number): string {
  return pcp === 0 ? '0%' : (pcp > 0 ? '+' : '') + (pcp * 100).toFixed(1) + '%'
}
</script>
