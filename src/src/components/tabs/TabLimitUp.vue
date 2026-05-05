<template>
  <div>
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

    <div class="glow-card overflow-hidden">
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
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { useReportStore } from '@/stores/report'

const store = useReportStore()
const hideST = ref(true)

const stocks = computed(() => store.report?.limit_up || [])
const bombRate = computed(() => store.report?.overview.bomb_rate || 0)

const sortedStocks = computed(() =>
  [...stocks.value].sort((a, b) => b.limit_up_days - a.limit_up_days || b.total_capital - a.total_capital)
)

const displayStocks = computed(() => {
  if (!hideST.value) return sortedStocks.value
  return sortedStocks.value.filter(s => !s.stock_chi_name.includes('ST') && !s.stock_chi_name.includes('*'))
})
</script>
