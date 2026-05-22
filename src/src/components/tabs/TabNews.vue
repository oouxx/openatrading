<template>
  <div>
    <!-- Header Stats -->
    <div class="flex items-center gap-3 mb-6 text-caption">
      <span class="text-white/60">
        <b class="text-white">{{ filteredNews.length }}</b> 条资讯
        <span v-if="source === 'all'" class="ml-2">
          · <b class="text-up">{{ majorCount }}</b> 重要
        </span>
      </span>
      <select
        v-model="source"
        class="bg-bg border border-hairline rounded-input px-2 py-0.5 text-caption text-ink-muted outline-none cursor-pointer font-mono"
      >
        <option value="all">全部来源</option>
        <option value="cls">财联社</option>
        <option value="jiuyan">韭研公社</option>
      </select>
    </div>

    <!-- Loading -->
    <div v-if="loading" class="text-center py-12 text-caption text-ink-muted">加载中...</div>

    <!-- News Grid -->
    <div v-else-if="filteredNews.length > 0" class="space-y-2">
      <div
        v-for="item in filteredNews"
        :key="item.id"
        class="bg-surface-dark rounded-card border border-hairline-dark px-4 py-3 flex items-start gap-3 hover:bg-bg-hover transition-colors cursor-pointer group"
      >
        <span class="text-caption shrink-0 mt-0.5">{{ item.severity === 'major' ? '🔴' : '⚪' }}</span>
        <div class="flex-1 min-w-0">
          <p class="text-caption text-white/80 leading-relaxed group-hover:text-white transition-colors">
            <span class="text-ink-muted font-mono mr-2 text-xs">{{ item.time }}</span>
            {{ item.title }}
          </p>
          <p v-if="item.aiNote" class="text-caption text-ai/70 mt-1 leading-relaxed text-xs">
            → {{ item.aiNote }}
          </p>
        </div>
      </div>
    </div>

    <!-- Empty -->
    <div v-else class="text-center py-12 text-caption text-ink-muted">暂无资讯</div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { apiFetch } from '@/stores/data'

const source = ref('all')
const loading = ref(false)

interface NewsItem {
  id: number
  severity: 'major' | 'normal'
  time: string
  title: string
  aiNote: string
  source: string
}

const newsItems = ref<NewsItem[]>([])

async function fetchNews() {
  try {
    const resp = await apiFetch('/api/telegraph')
    if (resp.ok) {
      const data: any[] = await resp.json()
      newsItems.value = data.map((item: any, idx: number) => ({
        id: item.id || idx,
        severity: item.level === 'R' || item.level === 'B' ? 'major' : 'normal',
        time: formatTime(item.ctime || item.modified_time),
        title: item.brief || item.title || (item.content || '').substring(0, 60),
        aiNote: item.title || '',
        source: 'cls',
      }))
    }
  } catch {
    // API not available — keep empty
  } finally {
    loading.value = false
  }
}

function formatTime(ts: number | string): string {
  if (!ts) return ''
  const t = typeof ts === 'string' ? parseInt(ts) : ts
  if (isNaN(t) || t === 0) return ''
  const d = new Date(t * 1000)
  const h = d.getHours().toString().padStart(2, '0')
  const m = d.getMinutes().toString().padStart(2, '0')
  return `${h}:${m}`
}

const filteredNews = computed(() => {
  if (source.value === 'all') return newsItems.value
  return newsItems.value.filter(n => n.source === source.value)
})

const majorCount = computed(() => newsItems.value.filter(n => n.severity === 'major').length)

onMounted(fetchNews)
</script>