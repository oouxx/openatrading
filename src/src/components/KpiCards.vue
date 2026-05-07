<template>
  <div>
    <!-- KPI Grid -->
    <div class="grid grid-cols-2 lg:grid-cols-4 gap-6 mb-6">
      <!-- Rise -->
      <div class="bg-canvas rounded-card p-card-inner border border-hairline" style="border-left: 3px solid #34c759">
        <p class="text-caption text-ink-muted mb-3">上涨家数</p>
        <p class="font-mono font-semibold text-data-green" style="font-size: 28px">{{ overview.rise_count.toLocaleString() }}</p>
      </div>

      <!-- Fall -->
      <div class="bg-canvas rounded-card p-card-inner border border-hairline" style="border-left: 3px solid #ff3b30">
        <p class="text-caption text-ink-muted mb-3">下跌家数</p>
        <p class="font-mono font-semibold text-data-red" style="font-size: 28px">{{ overview.fall_count.toLocaleString() }}</p>
      </div>

      <!-- Ratio -->
      <div class="bg-canvas rounded-card p-card-inner border border-hairline" style="border-left: 3px solid #0066cc">
        <p class="text-caption text-ink-muted mb-3">涨跌比</p>
        <p class="font-mono font-semibold" style="font-size: 28px" :class="ratioColorClass">{{ ratio }}</p>
        <p class="text-caption text-ink-muted mt-2">{{ overview.limit_up_count }}涨停 / {{ overview.limit_down_count }}跌停</p>
      </div>

      <!-- Bomb Rate -->
      <div class="bg-canvas rounded-card p-card-inner border border-hairline">
        <p class="text-caption text-ink-muted mb-3">炸板率</p>
        <p class="font-mono font-semibold" style="font-size: 28px" :class="bombColorClass">{{ (overview.bomb_rate * 100).toFixed(1) }}%</p>
      </div>
    </div>

    <!-- Risk Bar -->
    <div v-if="decisionSummary" class="bg-canvas rounded-card px-5 py-4 border border-hairline flex items-center gap-4">
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
  if (r >= 2) return 'text-data-green'
  if (r >= 1) return 'text-primary'
  return 'text-data-red'
})

const bombColorClass = computed(() => {
  if (props.overview.bomb_rate > 0.3) return 'text-data-red'
  if (props.overview.bomb_rate > 0.2) return 'text-primary'
  return 'text-data-green'
})

const riskBadgeClass = computed(() => {
  switch (props.riskLevel) {
    case '高': return 'bg-data-red/10 text-data-red'
    case '中': return 'bg-data-red/5 text-data-red'
    default: return 'bg-data-green/10 text-data-green'
  }
})
</script>
