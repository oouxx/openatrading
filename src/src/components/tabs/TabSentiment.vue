<template>
  <div>
    <h2 class="text-xl font-bold mb-4">📰 舆情分析</h2>

    <div v-if="!sentiment" class="text-gray-400 py-12 text-center">
      <p class="text-lg">暂无舆情数据</p>
      <p class="text-sm mt-2">数据将通过定时任务生成静态文件后加载</p>
    </div>
    <template v-else>
      <!-- 趋势标签 -->
      <div class="mb-6">
        <div class="flex items-center gap-2 mb-2">
          <span class="text-lg font-medium">趋势：</span>
          <span
            class="px-3 py-1 rounded-full text-sm"
            :class="trendColor"
          >
            {{ trend }}
          </span>
        </div>
      </div>

      <!-- 热门股票 TOP10 -->
      <div class="mb-6">
        <h3 class="text-lg font-medium mb-2">热门股票 TOP10</h3>
        <div class="space-y-2">
          <div
            v-for="(item, idx) in hotStocks"
            :key="idx"
            class="flex items-center gap-3 p-3 bg-white rounded-lg shadow-sm"
          >
            <span class="w-6 text-gray-400 font-bold">{{ idx + 1 }}</span>
            <div class="flex-1">
              <div class="font-mono font-medium">{{ item[0] }}</div>
              <div class="text-sm text-gray-600">{{ item[1] }}</div>
            </div>
            <div class="w-24">
              <div class="bg-blue-100 rounded-full h-2">
                <div
                  class="bg-blue-500 h-full rounded-full"
                  :style="{ width: (item[2] / maxMention * 100) + '%' }"
                ></div>
              </div>
            </div>
            <span class="text-sm text-gray-700 w-12 text-right">{{ item[2] }}次</span>
          </div>
        </div>
      </div>

      <!-- 热门文章列表 -->
      <div>
        <h3 class="text-lg font-medium mb-2">热门文章</h3>
        <div class="space-y-3">
          <details
            v-for="(article, idx) in articles"
            :key="idx"
            class="p-4 bg-white rounded-lg shadow-sm"
          >
            <summary class="cursor-pointer font-medium">{{ article.title || '文章' + (idx + 1) }}</summary>
            <div class="mt-2 text-sm text-gray-700">
              <p>{{ article.content || '暂无内容' }}</p>
              <div class="flex gap-2 mt-2">
                <span class="px-2 py-0.5 bg-gray-100 rounded text-xs">{{ article.user.nickname }}</span>
                <span class="px-2 py-0.5 bg-gray-100 rounded text-xs">❤️ {{ article.like_count }}</span>
                <span class="px-2 py-0.5 bg-gray-100 rounded text-xs">💬 {{ article.comment_count }}</span>
              </div>
            </div>
          </details>
        </div>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useReportStore } from '../../stores/report'

const store = useReportStore()

const sentiment = computed(() => store.report?.sentiment)
const trend = computed(() => sentiment.value?.trend || '暂无')
const hotStocks = computed(() => sentiment.value?.hot_stocks || [])
const articles = computed(() => sentiment.value?.articles || [])

const maxMention = computed(() => {
  if (hotStocks.value.length === 0) return 1
  return Math.max(...hotStocks.value.map(s => s[2]))
})

const trendColor = computed(() => {
  const t = trend.value
  if (t.includes('涨') || t.includes('多')) return 'bg-red-100 text-red-700'
  if (t.includes('跌') || t.includes('空')) return 'bg-green-100 text-green-700'
  return 'bg-yellow-100 text-yellow-700'
})
</script>
