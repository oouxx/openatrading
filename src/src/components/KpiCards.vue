<template>
  <div>
    <!-- KPI Grid -->
    <div class="grid grid-cols-2 lg:grid-cols-3 xl:grid-cols-6 gap-3 lg:gap-4 mb-6">
      <!-- Card 1: 情绪温度 (NEW) -->
      <div class="bg-bg-surface rounded-card p-card-inner border border-hairline border-l-2" :class="sentimentBorderClass">
        <p class="text-caption text-ink-muted mb-2">情绪温度</p>
        <p class="font-mono font-semibold" style="font-size: 22px" :class="sentimentColorClass">
          {{ sentimentTemp?.toFixed(0) || '--' }}
        </p>
        <p class="text-caption text-ink-muted mt-1">/100 {{ sentimentLabel }}</p>
      </div>

      <!-- Card 2: 涨停家数 (was 上涨家数 — restructured) -->
      <div class="bg-bg-surface rounded-card p-card-inner border border-hairline border-l-2 border-l-up">
        <p class="text-caption text-ink-muted mb-2">涨停家数</p>
        <p class="font-mono font-semibold text-up" style="font-size: 22px">{{ overview.limit_up_count }}</p>
        <p class="text-caption text-ink-muted mt-1">跌停 {{ overview.limit_down_count }}</p>
      </div>

      <!-- Card 3: 炸板率 (keep existing, add label) -->
      <div class="bg-bg-surface rounded-card p-card-inner border border-hairline">
        <p class="text-caption text-ink-muted mb-2">炸板率</p>
        <p class="font-mono font-semibold" style="font-size: 22px" :class="bombColorClass">
          {{ (overview.bomb_rate * 100).toFixed(1) }}%
        </p>
        <p class="text-caption text-ink-muted mt-1">{{ bombLabel }}</p>
      </div>

      <!-- Card 4: 连板晋级率 (NEW) -->
      <div class="bg-bg-surface rounded-card p-card-inner border border-hairline">
        <p class="text-caption text-ink-muted mb-2">连板晋级率</p>
        <p class="font-mono font-semibold" style="font-size: 22px" :class="promoColorClass">
          {{ promotionRate != null ? (promotionRate * 100).toFixed(1) + '%' : '--' }}
        </p>
        <p class="text-caption text-ink-muted mt-1">{{ promoLabel }}</p>
      </div>

      <!-- Card 5: 涨跌比 (keep existing, add sub-label) -->
      <div class="bg-bg-surface rounded-card p-card-inner border border-hairline border-l-2 border-l-ai">
        <p class="text-caption text-ink-muted mb-2">涨跌比</p>
        <p class="font-mono font-semibold" style="font-size: 22px" :class="ratioColorClass">{{ ratio }}</p>
        <p class="text-caption text-ink-muted mt-1">上{{ overview.rise_count }} 下{{ overview.fall_count }}</p>
      </div>

      <!-- Card 6: 主力净流入 (NEW) -->
      <div class="bg-bg-surface rounded-card p-card-inner border border-hairline">
        <p class="text-caption text-ink-muted mb-2">主力净流入</p>
        <p class="font-mono font-semibold" style="font-size: 22px" :class="flowColorClass">
          {{ flowDisplay }}
        </p>
        <p class="text-caption text-ink-muted mt-1">{{ flowSector }}</p>
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
  // New optional props (backend not ready yet — render '--' when undefined)
  promotionRate?: number
  promotionHistoryAvg?: number
  sentimentTemp?: number
  netFlow?: number
  flowSector?: string
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

const sentimentLabel = computed(() => {
  const t = props.sentimentTemp
  if (t == null) return '暂无数据'
  if (t >= 75) return '偏热'
  if (t >= 50) return '正常'
  return '偏冷'
})

const sentimentColorClass = computed(() => {
  const t = props.sentimentTemp
  if (t == null) return 'text-ink-muted'
  if (t >= 75) return 'text-up'
  if (t >= 50) return 'text-ai'
  return 'text-down'
})

const sentimentBorderClass = computed(() => {
  const t = props.sentimentTemp
  if (t == null) return 'border-l-ink-dim'
  if (t >= 75) return 'border-l-up'
  if (t >= 50) return 'border-l-ai'
  return 'border-l-down'
})

const bombLabel = computed(() => {
  const r = props.overview.bomb_rate
  if (r > 0.3) return '偏高 注意'
  if (r > 0.2) return '正常偏高'
  return '偏低 惜售'
})

const promoColorClass = computed(() => {
  const r = props.promotionRate
  if (r == null) return 'text-ink-muted'
  if (r >= 0.3) return 'text-up'
  return 'text-down'
})

const promoLabel = computed(() => {
  if (props.promotionHistoryAvg == null || props.promotionRate == null) return ''
  return props.promotionRate >= props.promotionHistoryAvg ? '↑ 历史均' : '↓ 历史均'
})

const flowDisplay = computed(() => {
  const f = props.netFlow
  if (f == null) return '--'
  const abs = Math.abs(f)
  const unit = abs >= 1e8 ? '亿' : '万'
  const val = abs >= 1e8 ? (abs / 1e8).toFixed(1) : (abs / 1e4).toFixed(0)
  return (f >= 0 ? '+' : '-') + val + unit
})

const flowColorClass = computed(() => {
  const f = props.netFlow
  if (f == null) return 'text-ink-muted'
  return f >= 0 ? 'text-up' : 'text-down'
})

const flowSector = computed(() => props.flowSector || '--')
</script>
