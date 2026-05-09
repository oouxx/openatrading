<template>
  <div class="bg-bg-surface rounded-card border border-hairline">
    <!-- Header -->
    <div class="flex items-center justify-between p-card-inner pb-2">
      <div class="flex items-center gap-2">
        <span class="text-caption-strong text-ink-muted font-mono uppercase tracking-wider">📡 实时资讯</span>
        <select v-model="source"
          class="bg-bg border border-hairline rounded-input px-2 py-0.5 text-caption text-ink-muted outline-none cursor-pointer font-mono">
          <option value="all">全部</option>
          <option value="jiuyan">韭研公社</option>
          <option value="tgb">淘股吧</option>
        </select>
      </div>
      <span class="text-caption text-ink-muted font-mono">{{ filteredNews.length }} 条</span>
    </div>
    <p class="text-caption text-ink-muted px-card-inner pb-2">数据源：韭研公社 · 淘股吧</p>

    <!-- News List -->
    <div class="max-h-[360px] overflow-y-auto">
      <div v-for="(item, idx) in filteredNews" :key="item.id"
        class="px-card-inner py-3 border-t border-hairline transition-colors hover:bg-bg-hover cursor-pointer"
        :class="idx === 0 ? 'border-t-0' : ''">
        <div class="flex items-start gap-2">
          <span class="text-caption shrink-0 mt-0.5">{{ item.severity === 'major' ? '🔴' : '⚪' }}</span>
          <div class="flex-1 min-w-0">
            <p class="text-caption text-white/80 leading-relaxed">
              <span class="text-ink-muted font-mono mr-1">{{ item.time }}</span>
              {{ item.title }}
            </p>
            <p class="text-caption text-ai/80 mt-1 leading-relaxed">→ AI标注：{{ item.aiNote }}</p>
          </div>
        </div>
      </div>
    </div>

    <!-- Load More -->
    <button
      class="w-full py-3 text-caption text-ai hover:text-ai/focus transition-colors cursor-pointer border-t border-hairline"
      @click="loadMore">
      加载更多历史新闻 ↓
    </button>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'

const source = ref('all')

interface NewsItem {
  id: number
  severity: 'major' | 'normal'
  time: string
  title: string
  aiNote: string
  source: string
}

const newsItems = ref<NewsItem[]>([
  {
    id: 1, severity: 'major', time: '15:42',
    title: '【重磅】某部委召开算力专项会议，云计算板块迎来政策催化',
    aiNote: '直接利好云计算/AI算力',
    source: 'jiuyan',
  },
  {
    id: 2, severity: 'normal', time: '14:55',
    title: '福达合金公告：中标政府采购项目，合同金额 2.3亿元',
    aiNote: '强化龙头逻辑，关注次日溢价',
    source: 'jiuyan',
  },
  {
    id: 3, severity: 'normal', time: '13:30',
    title: '半导体景气度报告：Q3预计回暖，国产替代进程加速',
    aiNote: '中线利好，明日可关注首板',
    source: 'jiuyan',
  },
  {
    id: 4, severity: 'normal', time: '11:20',
    title: '韭研公社：云计算连板情绪分析，散户乐观度达近期高点',
    aiNote: '情绪高位，注意兑现风险',
    source: 'jiuyan',
  },
  {
    id: 5, severity: 'normal', time: '10:05',
    title: '淘股吧热帖：福达合金4板讨论量暴增180%，多空开始分歧',
    aiNote: '关注信号，见「市场情报」Tab',
    source: 'tgb',
  },
])

const filteredNews = computed(() => {
  if (source.value === 'all') return newsItems.value
  return newsItems.value.filter(n => n.source === source.value)
})

function loadMore() {
  // Placeholder — will fetch older news from API in the future
}
</script>
