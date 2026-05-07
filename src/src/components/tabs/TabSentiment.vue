<template>
  <div>
    <div class="flex items-center gap-3 mb-8 text-caption">
      <span class="text-white/60">
        热度趋势: <b :class="trendColor">{{ trendText }}</b>
      </span>
    </div>

    <div v-if="!hasData" class="text-center py-12 text-caption text-white/40">暂无语情数据 — 需配置 JIUYAN_PHONE</div>

    <template v-else>
      <!-- Hot Stocks -->
      <div class="bg-surface-dark rounded-card border border-hairline-dark p-card-inner mb-6">
        <h3 class="text-caption-strong text-white/40 mb-4 tracking-wider uppercase">热门股票</h3>
        <div class="space-y-1" v-if="hotStocks.length">
          <div v-for="(s, idx) in hotStocks" :key="s[0]"
            class="flex items-center gap-3 py-1.5 px-2 rounded-card transition-colors hover:bg-white/[0.04]">
            <span class="text-caption font-mono w-6 text-white/40">#{{ idx + 1 }}</span>
            <span class="text-caption font-mono text-white">{{ s[0] }}</span>
            <span class="text-caption text-white/60">{{ s[1] }}</span>
            <span class="text-caption font-mono ml-auto px-2 py-0.5 rounded-badge bg-primary/15 text-primary">{{ s[2] }}次</span>
          </div>
        </div>
        <div v-else class="text-center py-4 text-caption text-white/40">今日暂无数据</div>
      </div>

      <!-- Hot Articles -->
      <div class="bg-surface-dark rounded-card border border-hairline-dark p-card-inner mb-6">
        <h3 class="text-caption-strong text-white/40 mb-4 tracking-wider uppercase">热门文章 · {{ hotArticles.length }}篇</h3>
        <div class="space-y-2" v-if="hotArticles.length">
          <div v-for="(a, idx) in hotArticles" :key="a.article_id"
            class="p-card-inner rounded-card border border-hairline-dark bg-white/[0.04] cursor-pointer transition-colors"
            @click="toggleArticle(a.article_id)">
            <div class="flex items-start justify-between gap-2">
              <div class="flex-1 min-w-0">
                <div class="flex items-center gap-1.5 mb-1">
                  <span class="text-caption font-mono shrink-0 text-white/40">#{{ idx + 1 }}</span>
                  <a :href="`https://www.jiuyangongshe.com/a/${a.article_id}`" target="_blank"
                    class="text-caption font-medium truncate hover:underline text-white"
                    @click.stop>{{ a.title }}</a>
                </div>
                <div class="flex items-center gap-3 text-caption text-white/40">
                  <span>{{ a.nickname }}</span>
                  <span>👍 {{ a.like_count }}</span>
                  <span>💬 {{ a.comment_count }}</span>
                  <span>🔄 {{ a.forward_count }}</span>
                  <span class="text-caption font-mono px-1 py-0.5 rounded-badge bg-primary/15 text-primary">{{ a.heat_score }}热度</span>
                </div>
              </div>
              <span class="text-caption shrink-0 transition-transform text-white/40" :style="{ transform: expandedId === a.article_id ? 'rotate(180deg)' : '' }">▼</span>
            </div>
            <div v-if="expandedId === a.article_id" class="mt-3 pt-3 border-t border-hairline-dark">
              <div class="text-caption leading-relaxed text-white/60" v-html="a.content"></div>
              <a :href="`https://www.jiuyangongshe.com/a/${a.article_id}`" target="_blank"
                class="inline-block mt-2 text-caption hover:underline text-primary">📎 查看原文</a>
            </div>
          </div>
        </div>
        <div v-else class="text-center py-4 text-caption text-white/40">今日暂无数据</div>
      </div>

      <!-- 淘股吧推荐 -->
      <div class="bg-surface-dark rounded-card border border-hairline-dark p-card-inner mb-6" v-if="taogubaArticles.length">
        <h3 class="text-caption-strong text-white/40 mb-4 tracking-wider uppercase">淘股吧推荐 · {{ taogubaArticles.length }}篇</h3>
        <div class="space-y-2">
          <div v-for="(a, idx) in taogubaArticles" :key="a.topic_id"
            class="p-card-inner rounded-card border border-hairline-dark bg-white/[0.04] cursor-pointer transition-colors"
            @click="toggleTgbArticle(a.topic_id)">
            <div class="flex items-start justify-between gap-2">
              <div class="flex-1 min-w-0">
                <div class="flex items-center gap-1.5 mb-1">
                  <span class="text-caption font-mono shrink-0 text-white/40">#{{ idx + 1 }}</span>
                  <span class="text-caption font-medium truncate text-white">{{ a.subject }}</span>
                </div>
                <div class="flex items-center gap-3 text-caption text-white/40">
                  <span>{{ a.user_name }}</span>
                  <span>👁️ {{ a.view_count }}</span>
                  <span>💬 {{ a.reply_count }}</span>
                  <span>{{ a.date_time }}</span>
                </div>
              </div>
              <span class="text-caption shrink-0 transition-transform text-white/40" :style="{ transform: expandedTgbId === a.topic_id ? 'rotate(180deg)' : '' }">▼</span>
            </div>
            <div v-if="expandedTgbId === a.topic_id" class="mt-3 pt-3 border-t border-hairline-dark">
              <div class="text-caption leading-relaxed text-white/60" v-html="a.content"></div>
              <a :href="`https://www.tgb.cn/article/${a.new_topic_id}`" target="_blank"
                class="inline-block mt-2 text-caption hover:underline text-primary">📎 查看原文</a>
            </div>
          </div>
        </div>
      </div>

      <!-- Summary -->
      <div class="bg-surface-dark rounded-card border border-hairline-dark p-card-inner" v-if="sentiment?.summary">
        <h3 class="text-caption-strong text-white/40 mb-3 tracking-wider uppercase">舆情总结</h3>
        <p class="text-body leading-relaxed text-white/60">{{ sentiment.summary }}</p>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import { useReportStore } from '@/stores/report'

const store = useReportStore()
const expandedId = ref<string | null>(null)
const expandedTgbId = ref<number | null>(null)

function toggleArticle(id: string) {
  expandedId.value = expandedId.value === id ? null : id
}

function toggleTgbArticle(id: number) {
  expandedTgbId.value = expandedTgbId.value === id ? null : id
}

const sentiment = computed(() => store.report?.sentiment as any)
const hasData = computed(() => !!(sentiment.value && (
  sentiment.value.hot_stocks?.length ||
  sentiment.value.hot_articles?.length ||
  sentiment.value.taoguba_articles?.length
)))

const hotStocks = computed(() => sentiment.value?.hot_stocks || [])
const hotArticles = computed(() => sentiment.value?.hot_articles || [])
const taogubaArticles = computed(() => sentiment.value?.taoguba_articles || [])

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
