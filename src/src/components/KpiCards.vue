<template>
  <div>
    <!-- KPI Grid -->
    <div class="grid grid-cols-2 lg:grid-cols-5 gap-3 lg:gap-4 mb-6">
      <!-- Card 1: 情绪温度 -->
      <motion.div
        class="bg-bg-surface rounded-card p-card-inner border border-hairline border-l-2"
        :class="sentimentBorderClass"
        :initial="{ opacity: 0, y: 16 }"
        :animate="{ opacity: 1, y: 0 }"
        :transition="{ duration: 0.35, delay: 0 }"
      >
        <p class="text-caption text-ink-muted mb-2">情绪温度</p>
        <p class="font-mono font-semibold" style="font-size: 22px" :class="sentimentColorClass">
          {{ sentimentTemp.toFixed(0) }}
        </p>
        <p class="text-caption text-ink-muted mt-1">/100 {{ sentimentLabel }}</p>
      </motion.div>

      <!-- Card 2: 涨停家数 -->
      <motion.div
        class="bg-bg-surface rounded-card p-card-inner border border-hairline border-l-2 border-l-up"
        :initial="{ opacity: 0, y: 16 }"
        :animate="{ opacity: 1, y: 0 }"
        :transition="{ duration: 0.35, delay: 0.06 }"
      >
        <p class="text-caption text-ink-muted mb-2">涨停家数</p>
        <p class="font-mono font-semibold text-up" style="font-size: 22px">{{ overview.limit_up_count }}</p>
        <p class="text-caption text-ink-muted mt-1">跌停 {{ overview.limit_down_count }}</p>
      </motion.div>

      <!-- Card 3: 炸板率 -->
      <motion.div
        class="bg-bg-surface rounded-card p-card-inner border border-hairline"
        :initial="{ opacity: 0, y: 16 }"
        :animate="{ opacity: 1, y: 0 }"
        :transition="{ duration: 0.35, delay: 0.12 }"
      >
        <p class="text-caption text-ink-muted mb-2">炸板率</p>
        <p class="font-mono font-semibold" style="font-size: 22px" :class="bombColorClass">
          {{ bombPct.toFixed(1) }}%
        </p>
        <p class="text-caption text-ink-muted mt-1">{{ bombLabel }}</p>
      </motion.div>

      <!-- Card 4: 昨日涨停表现 -->
      <motion.div
        class="bg-bg-surface rounded-card p-card-inner border border-hairline"
        :initial="{ opacity: 0, y: 16 }"
        :animate="{ opacity: 1, y: 0 }"
        :transition="{ duration: 0.35, delay: 0.18 }"
      >
        <p class="text-caption text-ink-muted mb-2">昨日涨停表现</p>
        <p class="font-mono font-semibold" style="font-size: 22px" :class="prevLimitUpClass">
          {{ overview.yesterday_limit_up_avg_pcp != null ? (overview.yesterday_limit_up_avg_pcp * 100).toFixed(2) + '%' : '--' }}
        </p>
        <p class="text-caption text-ink-muted mt-1">{{ prevLimitUpLabel }}</p>
      </motion.div>

      <!-- Card 5: 涨跌比 -->
      <motion.div
        class="bg-bg-surface rounded-card p-card-inner border border-hairline border-l-2 border-l-ai"
        :initial="{ opacity: 0, y: 16 }"
        :animate="{ opacity: 1, y: 0 }"
        :transition="{ duration: 0.35, delay: 0.24 }"
      >
        <p class="text-caption text-ink-muted mb-2">涨跌比</p>
        <p class="font-mono font-semibold" style="font-size: 22px" :class="ratioColorClass">{{ ratio }}</p>
        <p class="text-caption text-ink-muted mt-1">上{{ overview.rise_count }} 下{{ overview.fall_count }}</p>
      </motion.div>
    </div>

    <!-- Risk Bar -->
    <div v-if="decisionSummary" class="bg-bg-surface rounded-card px-5 py-4 border border-hairline flex items-center gap-4">
      <span class="px-3 py-1 rounded-button text-caption-strong font-mono" :class="riskBadgeClass">
        {{ riskLevel }}
      </span>
      <span class="text-body text-ink-muted">{{ decisionSummary }}</span>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { motion } from 'motion-v'

const props = defineProps<{
  overview: {
    rise_count: number
    fall_count: number
    limit_up_count: number
    limit_down_count: number
    bomb_rate: number
    market_temperature: number
    yesterday_limit_up_avg_pcp: number
  }
  riskLevel: string
  decisionSummary: string
}>()

const bombPct = computed(() => {
  return props.overview.bomb_rate * 100
})

const sentimentTemp = computed(() => {
  const t = props.overview.market_temperature
  if (t > 0) return Math.round(t)
  const { rise_count, fall_count, limit_up_count, limit_down_count } = props.overview
  const total = rise_count + fall_count
  if (total === 0) return 50
  const riseRatio = rise_count / total
  const bombFactor = 1 - bombPct.value / 100
  const limitRatio = Math.min(limit_up_count / Math.max(limit_down_count, 1), 10) / 10
  return Math.round(riseRatio * 60 + bombFactor * 20 + limitRatio * 20)
})

const ratio = computed(() => {
  if (props.overview.fall_count === 0) return '∞'
  return (props.overview.rise_count / props.overview.fall_count).toFixed(2)
})

const ratioColorClass = computed(() => {
  const r = props.overview.rise_count / Math.max(props.overview.fall_count, 1)
  if (r >= 2) return 'text-up'
  if (r >= 1) return 'text-ai'
  return 'text-down'
})

const bombColorClass = computed(() => {
  const b = bombPct.value
  if (b > 30) return 'text-up'
  if (b > 20) return 'text-ai'
  return 'text-down'
})

const bombLabel = computed(() => {
  const b = bombPct.value
  if (b > 30) return '偏高 注意'
  if (b > 20) return '正常偏高'
  return '偏低 惜售'
})

const prevLimitUpClass = computed(() => {
  const v = props.overview.yesterday_limit_up_avg_pcp
  if (v == null || v === 0) return 'text-ink-muted'
  return v > 0 ? 'text-up' : 'text-down'
})

const prevLimitUpLabel = computed(() => {
  const v = props.overview.yesterday_limit_up_avg_pcp
  if (v == null) return '暂无数据'
  if (v > 0.03) return '赚钱效应强'
  if (v > 0) return '微利'
  if (v > -0.03) return '微亏'
  return '亏钱效应'
})

const riskBadgeClass = computed(() => {
  switch (props.riskLevel) {
    case '高': return 'bg-up/10 text-up'
    case '中': return 'bg-up/5 text-up'
    default: return 'bg-down/10 text-down'
  }
})

const sentimentLabel = computed(() => {
  const t = sentimentTemp.value
  if (t >= 75) return '偏热'
  if (t >= 50) return '正常'
  return '偏冷'
})

const sentimentColorClass = computed(() => {
  const t = sentimentTemp.value
  if (t >= 75) return 'text-up'
  if (t >= 50) return 'text-ai'
  return 'text-down'
})

const sentimentBorderClass = computed(() => {
  const t = sentimentTemp.value
  if (t >= 75) return 'border-l-up'
  if (t >= 50) return 'border-l-ai'
  return 'border-l-down'
})
</script>