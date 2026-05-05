<template>
  <div class="grid grid-cols-2 lg:grid-cols-4 gap-4 mb-6">
    <!-- Rise -->
    <div class="glow-card p-5" style="border-left: 3px solid var(--green)">
      <p class="text-xs tracking-wide mb-3" style="color: var(--text-muted)">上涨家数</p>
      <p class="text-3xl font-bold" style="color: var(--green); font-family: var(--font-mono)">{{ overview.rise_count.toLocaleString() }}</p>
    </div>

    <!-- Fall -->
    <div class="glow-card p-5" style="border-left: 3px solid var(--red)">
      <p class="text-xs tracking-wide mb-3" style="color: var(--text-muted)">下跌家数</p>
      <p class="text-3xl font-bold" style="color: var(--red); font-family: var(--font-mono)">{{ overview.fall_count.toLocaleString() }}</p>
    </div>

    <!-- Ratio -->
    <div class="glow-card p-5" style="border-left: 3px solid var(--blue)">
      <p class="text-xs tracking-wide mb-3" style="color: var(--text-muted)">涨跌比</p>
      <p class="text-3xl font-bold" style="font-family: var(--font-mono)" :style="{ color: ratioColor }">{{ ratio }}</p>
      <p class="text-xs mt-2" style="color: var(--text-muted)">{{ overview.limit_up_count }}涨停 / {{ overview.limit_down_count }}跌停</p>
    </div>

    <!-- Bomb Rate -->
    <div class="glow-card p-5" style="border-left: 3px solid var(--amber)">
      <p class="text-xs tracking-wide mb-3" style="color: var(--text-muted)">炸板率</p>
      <p class="text-3xl font-bold" style="font-family: var(--font-mono)" :style="{ color: bombColor }">{{ (overview.bomb_rate * 100).toFixed(1) }}%</p>
    </div>
  </div>

  <!-- Decision Summary Bar -->
  <div class="glow-card px-5 py-3 mb-8 flex items-center gap-6 text-sm" v-if="decisionSummary">
    <span class="px-2 py-0.5 rounded text-xs font-bold font-mono" :style="{ background: riskBadgeBg, color: riskBadgeColor }">
      {{ riskLevel }}
    </span>
    <span style="color: var(--text-secondary)">{{ decisionSummary }}</span>
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

const ratioColor = computed(() => {
  const r = props.overview.rise_count / Math.max(props.overview.fall_count, 1)
  if (r >= 2) return 'var(--green)'
  if (r >= 1) return 'var(--blue)'
  return 'var(--red)'
})

const bombColor = computed(() => {
  if (props.overview.bomb_rate > 0.3) return 'var(--red)'
  if (props.overview.bomb_rate > 0.2) return 'var(--amber)'
  return 'var(--green)'
})

const riskBadgeBg = computed(() => {
  switch (props.riskLevel) {
    case '高': return 'rgba(255,23,68,0.1)'
    case '中': return 'rgba(255,171,0,0.1)'
    default: return 'rgba(0,230,118,0.1)'
  }
})

const riskBadgeColor = computed(() => {
  switch (props.riskLevel) {
    case '高': return 'var(--red)'
    case '中': return 'var(--amber)'
    default: return 'var(--green)'
  }
})
</script>
