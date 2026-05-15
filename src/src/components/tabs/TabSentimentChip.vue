<template>
  <div>
    <!-- Two-column: emotion cycle + chip structure -->
    <div class="grid grid-cols-1 lg:grid-cols-2 gap-6 mb-8">
      <!-- Left: Market Emotion Cycle -->
      <motion.div
        class="bg-surface-dark rounded-card border border-hairline-dark p-card-inner"
        :initial="{ opacity: 0, x: -20 }"
        :animate="{ opacity: 1, x: 0 }"
        :transition="{ duration: 0.4 }"
      >
        <h3 class="text-caption-strong text-ink-muted mb-4 uppercase tracking-wider">市场情绪周期判断</h3>
        <div v-if="!history.length" class="text-center py-8 text-caption text-ink-muted">加载中...</div>
        <div v-else class="space-y-3">
          <div class="flex items-center gap-3">
            <span class="text-caption text-ink-muted">当前：</span>
            <span class="px-3 py-1 rounded-button text-caption-strong font-mono" :class="currentPhaseClass">{{ currentPhaseLabel }}</span>
          </div>
          <div class="h-2 rounded-full bg-bg overflow-hidden">
            <div class="h-full rounded-full bg-gradient-to-r from-down via-ai to-up" :style="{ width: currentSentimentPct + '%' }"></div>
          </div>
          <div class="flex justify-between text-caption text-ink-muted">
            <span>冷淡</span><span>正常</span><span>偏热</span>
          </div>
          <p class="text-caption text-ink-muted mt-3">{{ phaseAdvice }}</p>
        </div>
      </div>

      <!-- Right: Market Chip Structure -->
      <motion.div
        class="bg-surface-dark rounded-card border border-hairline-dark p-card-inner"
        :initial="{ opacity: 0, x: 20 }"
        :animate="{ opacity: 1, x: 0 }"
        :transition="{ duration: 0.4 }"
      >
        <h3 class="text-caption-strong text-ink-muted mb-4 uppercase tracking-wider">市场筹码结构分析</h3>
        <div v-if="!overviewData" class="text-center py-8 text-caption text-ink-muted">加载中...</div>
        <div v-else class="space-y-3">
          <div class="flex justify-between text-caption">
            <span class="text-ink-muted">涨停家数</span>
            <span class="font-mono text-up">{{ overviewData.limit_up_count }}</span>
          </div>
          <div class="flex justify-between text-caption">
            <span class="text-ink-muted">跌停家数</span>
            <span class="font-mono text-down">{{ overviewData.limit_down_count }}</span>
          </div>
          <div class="flex justify-between text-caption">
            <span class="text-ink-muted">炸板率</span>
            <span class="font-mono" :class="bombRateClass">{{ bombRateDisplay }}</span>
          </div>
          <div class="flex justify-between text-caption">
            <span class="text-ink-muted">涨跌比</span>
            <span class="font-mono" :class="riseRatioClass">{{ riseRatioDisplay }}</span>
          </div>
          <div class="border-t border-hairline-dark pt-3 mt-3">
            <div class="flex justify-between text-caption">
              <span class="text-ink-muted">情绪评价</span>
              <span class="font-mono" :class="sentimentEvalClass">{{ sentimentEval }}</span>
            </div>
            <div class="flex justify-between text-caption mt-1">
              <span class="text-ink-muted">风险提示</span>
              <span class="font-mono text-up">{{ riskHint }}</span>
            </div>
          </div>
        </div>
      </motion.div>
    </div>

    <!-- Sentiment History Table -->
    <motion.div
      class="bg-surface-dark rounded-card border border-hairline-dark overflow-hidden"
      :initial="{ opacity: 0, y: 20 }"
      :animate="{ opacity: 1, y: 0 }"
      :transition="{ duration: 0.4, delay: 0.15 }"
    >
      <div class="p-card-inner border-b border-hairline-dark">
        <h3 class="text-caption-strong text-ink-muted uppercase tracking-wider">近{{ history.length }}日情绪数据</h3>
      </div>
      <div v-if="!history.length" class="text-center py-8 text-caption text-ink-muted">暂无数据</div>
      <table v-else class="data-table">
        <thead>
          <tr>
            <th>日期</th>
            <th style="text-align:right">情绪值</th>
            <th style="text-align:right">涨停数</th>
            <th style="text-align:right">炸板率</th>
            <th style="text-align:right">涨跌比</th>
            <th>市场特征</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="row in history" :key="row.date">
            <td class="font-mono">{{ row.date }}</td>
            <td class="font-mono text-right" :class="row.sentiment >= 75 ? 'text-up' : row.sentiment >= 60 ? 'text-ai' : 'text-down'">{{ row.sentiment.toFixed(1) }}</td>
            <td class="font-mono text-right">{{ row.limit_up_count }}</td>
            <td class="font-mono text-right">{{ row.bomb_rate.toFixed(1) }}%</td>
            <td class="font-mono text-right">{{ row.rise_ratio.toFixed(2) }}</td>
            <td>{{ getFeature(row) }}</td>
          </tr>
        </tbody>
      </table>
    </motion.div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted } from 'vue'
import { motion } from 'motion-v'
import { overview, sentimentHistory, fetchSentimentHistory } from '@/stores/data'

type HistoryRow = {
  date: string
  sentiment: number
  limit_up_count: number
  bomb_rate: number
  rise_ratio: number
}

const overviewData = computed(() => overview.data)
const history = computed<HistoryRow[]>(() => sentimentHistory.data)

onMounted(() => {
  if (!history.value.length) fetchSentimentHistory()
})

const currentSentiment = computed(() => {
  const rows = history.value
  return rows.length ? rows[rows.length - 1].sentiment : 50
})

const currentSentimentPct = computed(() => Math.min(Math.max(currentSentiment.value, 0), 100))

const hotDays = computed(() => {
  const rows = history.value
  let count = 0
  for (let i = rows.length - 1; i >= 0; i--) {
    if (rows[i].sentiment >= 75) count++
    else break
  }
  return count
})

const coldDays = computed(() => {
  const rows = history.value
  let count = 0
  for (let i = rows.length - 1; i >= 0; i--) {
    if (rows[i].sentiment < 50) count++
    else break
  }
  return count
})

const currentPhaseLabel = computed(() => {
  const s = currentSentiment.value
  if (s >= 75) return `${hotDays.value > 1 ? '偏热区间 第' + hotDays.value + '日' : '偏热区间'}`
  if (s >= 50) return '正常区间'
  if (coldDays.value > 1) return `偏冷区间 第${coldDays.value}日`
  return '偏冷区间'
})

const currentPhaseClass = computed(() => {
  const s = currentSentiment.value
  if (s >= 75) return 'bg-up/15 text-up'
  if (s >= 50) return 'bg-ai/15 text-ai'
  return 'bg-down/15 text-down'
})

const phaseAdvice = computed(() => {
  const s = currentSentiment.value
  const hot = hotDays.value
  const cold = coldDays.value
  if (s >= 75 && hot >= 3) return '历史规律：偏热第3日后，次日大概率出现情绪降温或震荡整理。建议：今日减少新建仓，持仓者注意设好止盈位。'
  if (s >= 75) return '情绪偏热，短线赚钱效应好，但需警惕连续升温后的回调风险。'
  if (s >= 50) return '情绪正常偏暖，短线操作环境尚可，可适度参与。'
  if (cold >= 3) return '连续偏冷，历史规律显示情绪冰点后可能迎来反弹，关注企稳信号。'
  return '情绪偏冷，建议观望为主，等待回暖信号再参与。'
})

const bombRateDisplay = computed(() => {
  const b = overviewData.value?.bomb_rate ?? 0
  const pct = b > 1 ? b : b * 100
  return pct.toFixed(1) + '%'
})

const bombRateClass = computed(() => {
  const b = overviewData.value?.bomb_rate ?? 0
  const pct = b > 1 ? b : b * 100
  if (pct > 30) return 'text-up'
  if (pct > 20) return 'text-ai'
  return 'text-down'
})

const riseRatioDisplay = computed(() => {
  const r = overviewData.value?.rise_count
  const f = overviewData.value?.fall_count
  if (!r || !f) return '--'
  return (r / f).toFixed(2)
})

const riseRatioClass = computed(() => {
  const r = overviewData.value?.rise_count ?? 0
  const f = Math.max(overviewData.value?.fall_count ?? 1, 1)
  const ratio = r / f
  if (ratio >= 2) return 'text-up'
  if (ratio >= 1) return 'text-ai'
  return 'text-down'
})

const sentimentEval = computed(() => {
  const s = currentSentiment.value
  if (s >= 75) return '过热'
  if (s >= 60) return '偏暖'
  if (s >= 50) return '正常'
  if (s >= 40) return '偏冷'
  return '冰点'
})

const sentimentEvalClass = computed(() => {
  const s = currentSentiment.value
  if (s >= 75) return 'text-up'
  if (s >= 50) return 'text-ai'
  return 'text-down'
})

const riskHint = computed(() => {
  const s = currentSentiment.value
  const bomb = overviewData.value?.bomb_rate ?? 0
  const bombPct = bomb > 1 ? bomb : bomb * 100
  if (s >= 75 && bombPct > 25) return '情绪过热+炸板率高，注意风险'
  if (s >= 75) return '情绪过热，注意回调'
  if (bombPct > 30) return '炸板率偏高，追涨风险大'
  if (s < 50) return '情绪偏冷，观望为主'
  return '暂无明显风险信号'
})

function getFeature(row: HistoryRow): string {
  const s = row.sentiment
  const b = row.bomb_rate
  if (s >= 75 && b < 20) return '偏热，情绪好'
  if (s >= 75) return '偏热，炸板偏高'
  if (s >= 60) return '正常偏暖'
  if (s >= 50) return '正常'
  if (s >= 40) return '偏冷'
  return '冰点'
}
</script>