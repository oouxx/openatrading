<template>
  <div class="bg-bg-surface rounded-card border border-hairline overflow-hidden">
    <!-- Header -->
    <div class="flex items-center justify-between px-4 py-3 border-b border-hairline">
      <div class="flex items-center gap-2">
        <span class="text-caption-strong text-ink-muted font-mono uppercase tracking-wider">📡 实时资讯</span>
        <select v-model="source" class="bg-bg border border-hairline rounded-input px-2 py-0.5 text-caption text-ink-muted outline-none cursor-pointer font-mono">
          <option value="all">全部</option>
          <option value="cls">财联社</option>
          <option value="jiuyan">韭研公社</option>
        </select>
      </div>
      <span class="text-caption text-ink-muted font-mono">{{ filteredNews.length }} 条</span>
    </div>

    <!-- News List -->
    <div class="max-h-[360px] overflow-y-auto">
      <div v-if="loading" class="text-center py-8 text-caption text-ink-muted">加载中...</div>

      <div v-for="(item, idx) in filteredNews" :key="item.id"
        class="px-4 py-3 border-t border-hairline transition-colors hover:bg-bg-hover cursor-pointer"
        :class="idx === 0 ? 'border-t-0' : ''">
        <div class="flex items-start gap-2">
          <span class="text-caption shrink-0 mt-0.5">{{ item.severity === 'major' ? '🔴' : '⚪' }}</span>
          <div class="flex-1 min-w-0">
            <p class="text-caption text-white/80 leading-relaxed">
              <span class="text-ink-muted font-mono mr-1">{{ item.time }}</span>
              {{ item.title }}
            </p>
            <p v-if="item.aiNote" class="text-caption text-ai/80 mt-1 leading-relaxed">→ {{ item.aiNote }}</p>
          </div>
        </div>
      </div>

      <div v-if="!loading && filteredNews.length === 0" class="text-center py-8 text-caption text-ink-muted">
        暂无资讯
      </div>
    </div>
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
  loading.value = true
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

onMounted(fetchNews)
</script>