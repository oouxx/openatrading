<template>
  <div>
    <!-- 涨停榜 Section -->
    <div class="flex items-center gap-4 mb-6 text-sm">
      <span style="color: var(--text-muted)">
        共 <b style="color: var(--text-primary)">{{ displayStocks.length }}</b> 只涨停
        <template v-if="hideST">（已隐藏ST）</template>
      </span>
      <label class="flex items-center gap-2 cursor-pointer select-none" style="color: var(--text-muted)">
        <input type="checkbox" v-model="hideST" class="cursor-pointer">
        <span class="text-xs">隐藏ST</span>
      </label>
      <span v-if="bombRate > 0.2" style="color: var(--amber); font-family: var(--font-mono); font-size: 13px">
        ⚠ 炸板率 {{ (bombRate * 100).toFixed(1) }}%
      </span>
    </div>

    <div class="glow-card overflow-hidden mb-8">
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
              <span class="px-2 py-0.5 rounded text-xs font-mono"
                :style="{
                  background: stock.limit_up_days >= 3 ? 'rgba(255,171,0,0.15)' : 'rgba(68,138,255,0.1)',
                  color: stock.limit_up_days >= 3 ? 'var(--amber)' : 'var(--blue)'
                }">
                {{ stock.limit_up_days }}板
              </span>
            </td>
            <td style="font-family: var(--font-mono); color: var(--text-primary)">{{ stock.symbol }}</td>
            <td style="font-weight: 500; color: var(--text-primary)">{{ stock.stock_chi_name }}</td>
            <td style="text-align:right; font-family: var(--font-mono); color: var(--green)">+{{ (stock.change_percent * 100).toFixed(2) }}%</td>
            <td style="text-align:right; font-family: var(--font-mono)">{{ (stock.turnover_ratio * 100).toFixed(2) }}%</td>
            <td style="text-align:right; font-family: var(--font-mono)">{{ (stock.total_capital / 1e8).toFixed(1) }}亿</td>
            <td style="color: var(--text-muted); font-size: 12px">
              {{ stock.surge_reason?.related_plates?.map((p: any) => p.plate_name).join(', ') || '-' }}
            </td>
          </tr>
        </tbody>
      </table>
    </div>

    <!-- 韭研异动 Section -->
    <div class="mt-8">
      <div class="flex items-center gap-3 mb-4 flex-wrap">
        <h3 class="text-sm font-bold" style="color: var(--text-primary)">📡 韭研公社异动</h3>
        <input
          type="date"
          :value="fieldDate"
          @change="onDateChange"
          class="px-2 py-1 text-xs rounded border"
          style="background: var(--bg-card); color: var(--text-primary); border-color: var(--border-subtle); font-family: var(--font-mono)"
        />
        <span class="text-xs" style="color: var(--text-muted)">
          {{ fieldItems.length }} 个板块 · {{ totalFieldStocks }} 只股票
        </span>
        <span v-if="fieldLoading" class="text-xs animate-pulse" style="color: var(--blue); font-family: var(--font-mono)">加载中...</span>
      </div>
      <div v-if="fieldItems.length === 0 && !fieldLoading" class="glow-card p-6 text-center text-sm" style="color: var(--text-muted)">该日期无异动数据</div>
      <div v-for="plate in fieldItems" :key="plate.name" class="glow-card p-4 mb-4">
        <div class="flex items-center gap-2 mb-3">
          <span class="text-sm font-medium" style="color: var(--text-primary)">{{ plate.name }}</span>
          <span class="text-xs px-2 py-0.5 rounded font-mono" style="background: var(--border-subtle); color: var(--text-secondary)">
            {{ plate.count }}只
          </span>
          <span class="text-xs" style="color: var(--amber)" v-if="plate.reason">{{ plate.reason }}</span>
        </div>
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
              <td class="font-mono" style="color: var(--text-muted)">{{ stock.time || '-' }}</td>
              <td class="font-mono" style="color: var(--text-primary)">{{ stock.code }}</td>
              <td style="color: var(--text-primary); font-weight: 500">{{ stock.name }}</td>
              <td class="font-mono">{{ stock.num || '-' }}</td>
              <td style="color: var(--text-secondary); font-size: 12px; max-width: 300px" v-html="stock.desc"></td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { useReportStore } from '@/stores/report'

const store = useReportStore()
const hideST = ref(true)
const fieldLoading = ref(false)

// 默认今天
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
    const base = localStorage.getItem('trading-agent-api-url') || 'http://localhost:8080'
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
