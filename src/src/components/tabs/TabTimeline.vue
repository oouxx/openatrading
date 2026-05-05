<template>
  <div>
    <div class="flex items-center gap-3 mb-6 text-sm">
      <span style="color: var(--text-muted)">
        <b style="color: var(--text-primary)">{{ totalEvents }}</b> 条事件 · <b style="color: var(--text-primary)">{{ timelineDays.length }}</b> 天
      </span>
    </div>

    <div v-if="timelineDays.length === 0" class="text-center py-12 text-sm" style="color: var(--text-muted)">暂无时间线数据</div>

    <div class="space-y-6">
      <div v-for="day in timelineDays" :key="day.date">
        <h3 class="text-xs font-mono font-medium mb-3 px-2" style="color: var(--blue)">{{ day.date }}</h3>
        <div class="space-y-2">
          <div v-for="item in day.items" :key="item.title" class="glow-card pl-4 py-3 pr-4 flex gap-3 items-start">
            <span class="text-sm flex-shrink-0 mt-0.5">{{ item.grade_emoji }}</span>
            <div class="flex-1 min-w-0">
              <div class="flex items-center gap-2 flex-wrap">
                <span class="text-sm font-medium" style="color: var(--text-primary)">{{ item.title }}</span>
                <span v-if="item.tags" class="text-xs px-1.5 py-0.5 rounded font-mono" style="background: var(--border-subtle); color: var(--text-muted)">{{ item.tags }}</span>
              </div>
              <p v-if="item.content" class="text-xs mt-1 leading-relaxed" style="color: var(--text-secondary)">{{ item.content }}</p>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useReportStore } from '@/stores/report'

const store = useReportStore()

interface TimelineItem {
  grade_emoji: string
  title: string
  tags: string
  content: string
}

const timelineDays = computed(() => {
  const items: { date: string; items: TimelineItem[] }[] = []
  ;[...(store.report?.timeline || [])].reverse().forEach((day: any) => {
    if (!day.list?.length) return
    items.push({
      date: day.date,
      items: day.list.map((item: any) => ({
        grade_emoji: item.timeline?.grade === 6 ? '🌟' : item.timeline?.grade === 5 ? '⭐' : '•',
        title: item.title || '',
        tags: (item.timeline?.theme_list || []).map((t: any) => t.name).join(', '),
        content: (item.content || '').replace(/\n/g, ' ').substring(0, 200),
      })),
    })
  })
  return items
})

const totalEvents = computed(() =>
  timelineDays.value.reduce((sum, d) => sum + d.items.length, 0)
)
</script>
