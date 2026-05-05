<template>
  <div>
    <div class="flex items-center gap-3 mb-6 text-sm">
      <span style="color: var(--text-muted)">
        热度趋势: <b :style="{ color: trendColor }">{{ trendText }}</b>
      </span>
    </div>

    <div v-if="!hasData" class="text-center py-12 text-sm" style="color: var(--text-muted)">暂无语情数据 — 需配置 JIUYAN_PHONE</div>

    <template v-else>
      <!-- Hot Stocks -->
      <div class="glow-card p-5 mb-6">
        <h3 class="text-xs font-medium mb-3" style="color: var(--text-muted); letter-spacing: 0.5px">热门股票</h3>
        <div class="space-y-1" v-if="hotStocks.length">
          <div v-for="(s, idx) in hotStocks" :key="s[0]"
            class="flex items-center gap-3 py-1.5 px-2 rounded hover:bg-opacity-10 hover:bg-blue-500 transition-colors">
            <span class="text-xs font-mono w-6" style="color: var(--text-muted)">#{{ idx + 1 }}</span>
            <span class="text-sm font-mono" style="color: var(--text-primary)">{{ s[0] }}</span>
            <span class="text-sm" style="color: var(--text-secondary)">{{ s[1] }}</span>
            <span class="text-xs font-mono ml-auto px-2 py-0.5 rounded" style="background: rgba(68,138,255,0.1); color: var(--blue)">{{ s[2] }}次</span>
          </div>
        </div>
        <div v-else class="text-center py-4 text-xs" style="color: var(--text-muted)">今日暂无数据</div>
      </div>

      <!-- Hot Articles -->
      <div class="glow-card p-5 mb-6">
        <h3 class="text-xs font-medium mb-3" style="color: var(--text-muted); letter-spacing: 0.5px">热门文章</h3>
        <div class="space-y-1" v-if="hotArticles.length">
          <div v-for="(a, idx) in hotArticles" :key="a[0]"
            class="flex items-center gap-3 py-1.5 px-2 rounded hover:bg-opacity-10 hover:bg-blue-500 transition-colors">
            <span class="text-xs font-mono w-6" style="color: var(--text-muted)">#{{ idx + 1 }}</span>
            <span class="text-sm flex-1 truncate" style="color: var(--text-primary)">{{ a[0] }}</span>
            <span class="text-xs font-mono px-2 py-0.5 rounded" style="background: rgba(255,171,0,0.1); color: var(--amber)">热度 {{ a[1] }}</span>
          </div>
        </div>
        <div v-else class="text-center py-4 text-xs" style="color: var(--text-muted)">今日暂无数据</div>
      </div>

      <!-- Summary -->
      <div class="glow-card p-5" v-if="sentiment?.summary">
        <h3 class="text-xs font-medium mb-2" style="color: var(--text-muted); letter-spacing: 0.5px">舆情总结</h3>
        <p class="text-sm leading-relaxed" style="color: var(--text-secondary)">{{ sentiment.summary }}</p>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useReportStore } from '@/stores/report'

const store = useReportStore()

const sentiment = computed(() => store.report?.sentiment as any)
const hasData = computed(() => !!(sentiment.value && (sentiment.value.hot_stocks?.length || sentiment.value.hot_articles?.length)))

const hotStocks = computed(() => sentiment.value?.hot_stocks || [])
const hotArticles = computed(() => sentiment.value?.hot_articles || [])

const trendText = computed(() => {
  const t = sentiment.value?.trend
  if (!t) return '暂无数据'
  return t === '偏多' ? '📈 偏多' : t === '中性' ? '➡️ 中性' : '📉 偏弱'
})

const trendColor = computed(() => {
  const t = sentiment.value?.trend
  if (t === '偏多') return 'var(--green)'
  if (t === '中性') return 'var(--amber)'
  return 'var(--red)'
})
</script>
