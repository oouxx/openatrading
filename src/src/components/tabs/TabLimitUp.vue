<template>
  <div>
    <!-- Stats bar -->
    <div class="flex items-center gap-4 mb-8 text-caption">
      <span class="text-white/60">
        共 <b class="text-white">{{ displayStocks.length }}</b> 只涨停
        <template v-if="hideST">（已隐藏ST）</template>
      </span>
      <label class="flex items-center gap-2 cursor-pointer select-none text-white/60">
        <input type="checkbox" v-model="hideST" class="cursor-pointer">
        <span class="text-caption">隐藏ST</span>
      </label>
      <span v-if="bombRate > 0.2" class="text-data-red font-mono text-xs">
        ⚠ 炸板率 {{ (bombRate * 100).toFixed(1) }}%
      </span>
    </div>

    <!-- Limit-up table in dark card -->
    <div class="bg-surface-dark rounded-card border border-hairline-dark overflow-hidden mb-12">
      <table class="data-table">
        <thead>
          <tr>
            <th>连板</th>
            <th>代码</th>
            <th>名称</th>
            <th style="text-align:right">涨跌幅</th>
            <th style="text-align:right">换手率</th>
            <th style="text-align:right">流通市值</th>
            <th>板块</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="stock in displayStocks" :key="stock.symbol">
            <td>
              <span class="px-2 py-0.5 rounded-badge text-xs font-mono"
                :class="stock.limit_up_days >= 3
                  ? 'bg-primary/20 text-primary'
                  : 'bg-white/10 text-white/60'">
                {{ stock.limit_up_days }}板
              </span>
            </td>
            <td class="cursor-pointer font-mono hover:underline text-primary" @click="openKline(stock.symbol, stock.stock_chi_name)">{{ stock.symbol }}</td>
            <td class="font-medium text-white">{{ stock.stock_chi_name }}</td>
            <td class="text-right font-mono text-data-green">+{{ (stock.change_percent * 100).toFixed(2) }}%</td>
            <td class="text-right font-mono">{{ (stock.turnover_ratio * 100).toFixed(2) }}%</td>
            <td class="text-right font-mono">{{ (stock.total_capital / 1e8).toFixed(1) }}亿</td>
            <td class="text-white/40 text-xs">
              {{ stock.surge_reason?.related_plates?.map((p: any) => p.plate_name).join(', ') || '-' }}
            </td>
          </tr>
        </tbody>
      </table>
    </div>

    <!-- Field Items Section -->
    <div>
      <div class="flex items-center gap-3 mb-6 flex-wrap">
        <h3 class="text-display-md text-white">韭研公社异动</h3>
        <input
          type="date"
          :value="fieldDate"
          @change="onDateChange"
          class="px-3 py-1.5 text-caption font-mono rounded-input border border-hairline-dark bg-surface-dark text-white outline-none focus:border-primary"
        />
        <span class="text-caption text-white/40">
          {{ fieldItems.length }} 个板块 · {{ totalFieldStocks }} 只股票
        </span>
        <span v-if="fieldLoading" class="text-caption font-mono text-primary animate-pulse">加载中...</span>
      </div>

      <!-- Field item cards -->
      <div v-if="fieldItems.length === 0 && !fieldLoading" class="text-center py-12 text-caption text-white/40">
        该日期无异动数据
      </div>
      <div v-for="plate in fieldItems" :key="plate.name" class="bg-surface-dark rounded-card border border-hairline-dark p-card-inner mb-4">
        <div class="flex items-center gap-2 mb-4">
          <span class="text-body-strong text-white">{{ plate.name }}</span>
          <span class="text-caption font-mono px-2 py-0.5 rounded-badge bg-white/10 text-white/60">
            {{ plate.count }}只
          </span>
          <span class="text-caption text-primary" v-if="plate.reason">{{ plate.reason }}</span>
        </div>
        <div class="bg-surface-dark-2 rounded-card overflow-hidden">
          <table class="data-table">
            <thead>
              <tr>
                <th>时间</th>
                <th>代码</th>
                <th>名称</th>
                <th>连板</th>
                <th>异动理由</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="stock in plate.list" :key="stock.code">
                <td class="font-mono text-white/40">{{ stock.article?.action_info?.time || '-' }}</td>
                <td class="font-mono text-white">{{ stock.code }}</td>
                <td class="font-medium text-white">{{ stock.name }}</td>
                <td class="font-mono">{{ stock.article?.action_info?.num || '-' }}</td>
                <td class="text-white/60 text-xs" style="max-width: 300px">{{ stock.article?.action_info?.expound || '' }}</td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, inject } from 'vue'
import { useReportStore } from '@/stores/report'

const store = useReportStore()
const openKline = inject<(symbol: string, name: string) => void>('openKline', () => {})
const hideST = ref(true)
const fieldLoading = ref(false)

const todayStr = () => new Date().toISOString().split('T')[0]
const fieldDate = ref(todayStr())

const stocks = computed(() => store.report?.limit_up || [])
const bombRate = computed(() => store.report?.overview.bomb_rate || 0)
const fieldItems = computed(() => store.report?.field_items || [])

const sortedStocks = computed(() =>
  [...stocks.value].sort((a, b) => b.limit_up_days - a.limit_up_days || b.total_capital - a.total_capital)
)

const displayStocks = computed(() => {
  if (!hideST.value) return sortedStocks.value
  return sortedStocks.value.filter(s => !s.stock_chi_name.includes('ST') && !s.stock_chi_name.includes('*'))
})

const totalFieldStocks = computed(() =>
  fieldItems.value.reduce((sum: number, p: any) => sum + (p.list?.length || 0), 0)
)

async function onDateChange(e: Event) {
  const val = (e.target as HTMLInputElement).value
  if (!val) return
  fieldDate.value = val
  fieldLoading.value = true
  try {
    const base = localStorage.getItem('trading-agent-api-url') || __API_BASE_URL__
    const resp = await fetch(`${base}/api/report/live?date=${val}`)
    if (!resp.ok) throw new Error(`HTTP ${resp.status}`)
    store.report = await resp.json()
  } catch (err: any) {
    console.error('Field date change failed:', err)
  } finally {
    fieldLoading.value = false
  }
}
</script>
