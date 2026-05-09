<template>
  <div>
    <!-- KPI Grid -->
    <div class="grid grid-cols-2 lg:grid-cols-4 gap-4 lg:gap-6 mb-6">
      <!-- Rise -->
      <div class="bg-bg-surface rounded-card p-card-inner border border-hairline border-l-2 border-l-up">
        <p class="text-caption text-ink-muted mb-3">上涨家数</p>
        <p class="font-mono font-semibold text-up" style="font-size: 28px">{{ overview.rise_count.toLocaleString() }}</p>
      </div>

      <!-- Fall -->
      <div class="bg-bg-surface rounded-card p-card-inner border border-hairline border-l-2 border-l-down">
        <p class="text-caption text-ink-muted mb-3">下跌家数</p>
        <p class="font-mono font-semibold text-down" style="font-size: 28px">{{ overview.fall_count.toLocaleString() }}</p>
      </div>

      <!-- Ratio -->
      <div class="bg-bg-surface rounded-card p-card-inner border border-hairline border-l-2 border-l-ai">
        <p class="text-caption text-ink-muted mb-3">涨跌比</p>
        <p class="font-mono font-semibold" style="font-size: 28px" :class="ratioColorClass">{{ ratio }}</p>
        <p class="text-caption text-ink-muted mt-2">{{ overview.limit_up_count }}涨停 / {{ overview.limit_down_count }}跌停</p>
      </div>

      <!-- Bomb Rate -->
      <div class="bg-bg-surface rounded-card p-card-inner border border-hairline">
        <p class="text-caption text-ink-muted mb-3">炸板率</p>
        <p class="font-mono font-semibold" style="font-size: 28px" :class="bombColorClass">{{ (overview.bomb_rate * 100).toFixed(1) }}%</p>
      </div>
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

const props = defineProps<{
  overview: {
    rise_count: number
    fall_count: number
    limit_up_count: number
    limit_down_count: number
    bomb_rate: number
  }
  riskLevel: string
  decisionSummary: string
}>()

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
  if (props.overview.bomb_rate > 0.3) return 'text-up'
  if (props.overview.bomb_rate > 0.2) return 'text-ai'
  return 'text-down'
})

const riskBadgeClass = computed(() => {
  switch (props.riskLevel) {
    case '高': return 'bg-up/10 text-up'
    case '中': return 'bg-up/5 text-up'
    default: return 'bg-down/10 text-down'
  }
})
</script>
