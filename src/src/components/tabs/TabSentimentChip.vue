<template>
  <div>
    <!-- Two-column: emotion cycle + chip structure -->
    <div class="grid grid-cols-1 lg:grid-cols-2 gap-6 mb-8">
      <!-- Left: Market Emotion Cycle -->
      <div class="bg-surface-dark rounded-card border border-hairline-dark p-card-inner">
        <h3 class="text-caption-strong text-ink-muted mb-4 uppercase tracking-wider">市场情绪周期判断</h3>
        <div class="space-y-3">
          <div class="flex items-center gap-3">
            <span class="text-caption text-ink-muted">当前：</span>
            <span class="px-3 py-1 rounded-button text-caption-strong font-mono bg-ai/15 text-ai">偏热区间 第3日</span>
          </div>
          <div class="h-2 rounded-full bg-bg overflow-hidden">
            <div class="h-full rounded-full bg-gradient-to-r from-down via-ai to-up" style="width: 78%"></div>
          </div>
          <div class="flex justify-between text-caption text-ink-muted">
            <span>冷淡</span><span>正常</span><span>偏热</span>
          </div>
          <p class="text-caption text-ink-muted mt-3">历史规律：偏热第3日后，次日大概率出现情绪降温或震荡整理。建议：今日减少新建仓，持仓者注意设好止盈位。</p>
        </div>
      </div>

      <!-- Right: Leader Chip Structure -->
      <div class="bg-surface-dark rounded-card border border-hairline-dark p-card-inner">
        <h3 class="text-caption-strong text-ink-muted mb-4 uppercase tracking-wider">龙头筹码结构分析</h3>
        <div class="space-y-3">
          <div class="flex justify-between text-caption">
            <span class="text-ink-muted">获利盘</span>
            <span class="font-mono text-white">约 38%</span>
          </div>
          <div class="flex justify-between text-caption">
            <span class="text-ink-muted">主力持仓估算</span>
            <span class="font-mono text-white">约 55%</span>
          </div>
          <div class="flex justify-between text-caption">
            <span class="text-ink-muted">散户参与度</span>
            <span class="font-mono text-down">偏低（健康）</span>
          </div>
          <div class="border-t border-hairline-dark pt-3 mt-3">
            <div class="flex justify-between text-caption">
              <span class="text-ink-muted">筹码评价</span>
              <span class="font-mono text-ai">锁定较好</span>
            </div>
            <div class="flex justify-between text-caption mt-1">
              <span class="text-ink-muted">风险提示</span>
              <span class="font-mono text-up">获利盘压力需消化</span>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- Sentiment History Table -->
    <div class="bg-surface-dark rounded-card border border-hairline-dark overflow-hidden">
      <div class="p-card-inner border-b border-hairline-dark">
        <h3 class="text-caption-strong text-ink-muted uppercase tracking-wider">近10日情绪数据</h3>
      </div>
      <table class="data-table">
        <thead>
          <tr>
            <th>日期</th>
            <th style="text-align:right">情绪值</th>
            <th style="text-align:right">涨停数</th>
            <th style="text-align:right">炸板率</th>
            <th style="text-align:right">晋级率</th>
            <th>市场特征</th>
          </tr>
        </thead>
        <!-- Static demo rows — real data will come from /api/sentiment-history -->
        <tbody>
          <tr v-for="row in historyRows" :key="row.date">
            <td class="font-mono">{{ row.date }}</td>
            <td class="font-mono text-right" :class="row.sentiment >= 75 ? 'text-up' : row.sentiment >= 60 ? 'text-ai' : 'text-down'">{{ row.sentiment }}</td>
            <td class="font-mono text-right">{{ row.limitUpCount }}</td>
            <td class="font-mono text-right">{{ row.bombRate }}%</td>
            <td class="font-mono text-right">{{ row.promoRate }}%</td>
            <td>{{ row.feature }}</td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>

<script setup lang="ts">
const historyRows = [
  { date: '4/28', sentiment: 62, limitUpCount: 78, bombRate: 24, promoRate: 28, feature: '正常偏暖' },
  { date: '4/29', sentiment: 71, limitUpCount: 95, bombRate: 19, promoRate: 33, feature: '升温，可积极' },
  { date: '5/06', sentiment: 74, limitUpCount: 102, bombRate: 17, promoRate: 35, feature: '偏热，情绪好' },
  { date: '5/07', sentiment: 76, limitUpCount: 108, bombRate: 16, promoRate: 34, feature: '持续偏热' },
  { date: '5/08', sentiment: 78, limitUpCount: 110, bombRate: 18, promoRate: 31, feature: '偏热第3日，注意节奏' },
]
</script>
