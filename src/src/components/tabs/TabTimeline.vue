<template>
  <div>
    <!-- Sub-view toggle buttons -->
    <div class="flex items-center gap-1 mb-6">
      <button
        v-for="v in subViews"
        :key="v.id"
        @click="activeSubView = v.id"
        class="px-4 py-1.5 rounded-button text-caption font-mono transition-colors cursor-pointer"
        :class="activeSubView === v.id ? 'bg-ai/20 text-ai' : 'text-ink-muted hover:text-white/80'"
      >
        {{ v.label }}
      </button>
    </div>

    <!-- 今日大盘 -->
    <div v-if="activeSubView === 'dapan'">
      <div v-if="dapanAbnormal.loading" class="flex items-center justify-center py-16 gap-3">
        <span class="w-2 h-2 rounded-full bg-ai animate-pulse"></span>
        <span class="text-caption font-mono text-white/40">加载大盘异动...</span>
      </div>

      <div v-else-if="dapanAbnormal.error" class="bg-surface-dark rounded-card border border-hairline-dark p-card-inner text-center text-caption text-white/40">
        {{ dapanAbnormal.error }}
      </div>

      <template v-else>
        <div class="flex items-center gap-3 mb-6 text-caption">
          <span class="text-white/60">
            共 <b class="text-white">{{ dapanAbnormal.data.length }}</b> 条异动
          </span>
        </div>

        <div v-if="dapanAbnormal.data.length === 0" class="text-center py-12 text-caption text-white/40">
          今日暂无大盘异动
        </div>

        <div v-else class="space-y-2">
          <div
            v-for="item in dapanAbnormal.data"
            :key="item.id"
            class="bg-surface-dark rounded-card border border-hairline-dark px-4 py-3 flex items-start gap-3 hover:bg-bg-hover transition-colors"
          >
            <span class="text-caption flex-shrink-0 mt-0.5">{{ typeEmoji(item.bk_yi_dong_type) }}</span>
            <div class="flex-1 min-w-0">
              <div class="flex items-center gap-2 flex-wrap">
                <span class="text-body-strong text-white">{{ item.title }}</span>
                <span class="text-caption px-2 py-0.5 rounded-badge font-mono bg-ai/15 text-ai">
                  {{ item.bkj_info?.name || '-' }}
                </span>
                <span class="text-caption font-mono text-white/40">{{ formatTs(item.created_at) }}</span>
              </div>
            </div>
          </div>
        </div>
      </template>
    </div>

    <!-- 本月事件 -->
    <div v-if="activeSubView === 'timeline'">
      <div class="flex items-center gap-3 mb-8 text-caption">
        <span class="text-white/60">
          <b class="text-white">{{ totalEvents }}</b> 条事件 · <b class="text-white">{{ timelineDays.length }}</b> 天
        </span>
      </div>

      <div v-if="timelineDays.length === 0" class="text-center py-12 text-caption text-white/40">暂无时间线数据</div>

      <div class="space-y-8">
        <div v-for="day in timelineDays" :key="day.date">
          <h3 class="text-caption-strong font-mono mb-4 px-2 text-ai">{{ day.date }}</h3>
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
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { timeline, fetchTimeline, dapanAbnormal, fetchDapanAbnormal } from '@/stores/data'

const activeSubView = ref('dapan')

const subViews = [
  { id: 'dapan', label: '今日大盘' },
  { id: 'timeline', label: '本月事件' },
]

function typeEmoji(t: number): string {
  if (t === 1) return '📈'
  if (t === 2) return '📉'
  return '📊'
}

function formatTs(ts: number): string {
  if (!ts) return ''
  const d = new Date(ts * 1000)
  const h = d.getHours().toString().padStart(2, '0')
  const m = d.getMinutes().toString().padStart(2, '0')
  return `${h}:${m}`
}

// ─── Timeline (本月事件) ──────────────────────────────

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

onMounted(() => {
  fetchDapanAbnormal()
  fetchTimeline()
})
</script>
