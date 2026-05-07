<template>
  <div>
    <div class="flex items-center gap-3 mb-8 text-caption">
      <span class="text-white/60">
        <b class="text-white">{{ totalEvents }}</b> 条事件 · <b class="text-white">{{ timelineDays.length }}</b> 天
      </span>
    </div>

    <div v-if="timelineDays.length === 0" class="text-center py-12 text-caption text-white/40">暂无时间线数据</div>

    <div class="space-y-8">
      <div v-for="day in timelineDays" :key="day.date">
        <h3 class="text-caption-strong font-mono mb-4 px-2 text-primary">{{ day.date }}</h3>
        <div class="space-y-3">
          <div v-for="item in day.items" :key="item.title" class="bg-surface-dark rounded-card border border-hairline-dark pl-4 py-3 pr-4 flex gap-3 items-start">
            <span class="text-caption flex-shrink-0 mt-0.5">{{ item.grade_emoji }}</span>
            <div class="flex-1 min-w-0">
              <div class="flex items-center gap-2 flex-wrap">
                <span class="text-body-strong text-white">{{ item.title }}</span>
                <span v-if="item.tags" class="text-caption px-2 py-0.5 rounded-badge font-mono bg-white/10 text-white/40">{{ item.tags }}</span>
              </div>
              <p v-if="item.content" class="text-caption mt-1 leading-relaxed text-white/60">{{ item.content }}</p>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted } from 'vue'
import { timeline, fetchTimeline } from '@/stores/data'

interface TimelineItem {
  grade_emoji: string
  title: string
  tags: string
  content: string
}

const timelineDays = computed(() => {
  const items: { date: string; items: TimelineItem[] }[] = []
  ;[...(timeline.data || [])].reverse().forEach((day: any) => {
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

onMounted(() => fetchTimeline())
</script>
