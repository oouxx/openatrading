<template>
  <div>
    <!-- Sub-view toggle buttons -->
    <div class="flex items-center gap-1 mb-6">
      <button
        v-for="v in subViews"
        :key="v.id"
        @click="activeSubView = v.id"
        class="px-4 py-1.5 rounded-button text-caption font-mono transition-colors cursor-pointer"
        :class="activeSubView === v.id ? 'bg-ai/20 text-ai' : 'text-ink-muted hover:text-white/80'"
      >
        {{ v.label }} {{ v.count }}
      </button>
    </div>

    <!-- Sub-view A: 涨停股 (ALL existing template content goes here) -->
    <div v-if="activeSubView === 'limit-up'">
      <!-- Stats bar -->
      <div class="flex items-center gap-4 mb-8 text-caption">
        <span class="text-white/60">
          共 <b class="text-white">{{ filteredStocks.length }}</b> 只涨停
          <template v-if="hideST">（已隐藏ST）</template>
        </span>
        <!-- Filter pills -->
        <div class="flex items-center gap-1">
          <button v-for="f in filters" :key="f.id" @click="activeFilter = f.id"
            class="px-2 py-0.5 rounded-badge text-caption font-mono transition-colors cursor-pointer"
            :class="activeFilter === f.id ? 'bg-ai/20 text-ai' : 'text-ink-muted hover:text-white/80'">
            {{ f.label }}
          </button>
        </div>
        <label class="flex items-center gap-2 cursor-pointer select-none text-white/60">
          <input type="checkbox" v-model="hideST" class="cursor-pointer">
          <span class="text-caption">隐藏ST</span>
        </label>
        <span v-if="bombRate > 20" class="text-up font-mono text-xs">
          &#9888; 炸板率 {{ bombRate.toFixed(1) }}%
        </span>
      </div>

      <!-- Limit-up table in dark card -->
      <motion.div
        class="bg-surface-dark rounded-card border border-hairline-dark overflow-hidden mb-12"
        :initial="{ opacity: 0, y: 16 }"
        :animate="{ opacity: 1, y: 0 }"
        :transition="{ duration: 0.35 }"
      >
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
            <tr v-for="stock in filteredStocks" :key="stock.symbol">
              <td>
                <span class="px-2 py-0.5 rounded-badge text-xs font-mono"
                  :class="stock.limit_up_days >= 3
                    ? 'bg-ai/20 text-ai'
                    : 'bg-white/10 text-white/60'">
                  {{ stock.limit_up_days }}板
                </span>
              </td>
              <td class="cursor-pointer font-mono hover:underline text-ai" @click="openKline(stock.symbol, stock.stock_chi_name)"><MiniChartPreview :symbol="stock.symbol" :name="stock.stock_chi_name">{{ stock.symbol }}</MiniChartPreview></td>
              <td class="font-medium text-white"><MiniChartPreview :symbol="stock.symbol" :name="stock.stock_chi_name">{{ stock.stock_chi_name }}</MiniChartPreview></td>
              <td class="text-right font-mono text-up">+{{ (stock.change_percent * 100).toFixed(2) }}%</td>
              <td class="text-right font-mono">{{ (stock.turnover_ratio * 100).toFixed(2) }}%</td>
              <td class="text-right font-mono">{{ (stock.total_capital / 1e8).toFixed(1) }}亿</td>
              <td class="text-white/40 text-xs">
                {{ stock.surge_reason?.related_plates?.map((p: any) => p.plate_name).join(', ') || '-' }}
              </td>
            </tr>
          </tbody>
        </table>
      </motion.div>

      <!-- Field Items Section -->
      <div>
        <div class="flex items-center gap-3 mb-6 flex-wrap">
          <h3 class="text-display-md text-white">韭研公社异动</h3>
          <input
            type="date"
            :value="fieldDate"
            @change="onDateChange"
            class="px-3 py-1.5 text-caption font-mono rounded-input border border-hairline-dark bg-surface-dark text-white outline-none focus:border-ai"
          />
          <span class="text-caption text-white/40">
            {{ fieldPlates.length }} 个板块 · {{ totalFieldStocks }} 只股票
          </span>
          <span v-if="fieldItems.loading" class="text-caption font-mono text-ai animate-pulse">加载中...</span>
        </div>

        <!-- Field item cards -->
        <div v-if="fieldPlates.length === 0 && !fieldItems.loading" class="text-center py-12 text-caption text-white/40">
          该日期无异动数据
        </div>
        <div v-for="plate in fieldPlates" :key="plate.name" class="bg-surface-dark rounded-card border border-hairline-dark p-card-inner mb-4">
          <div class="flex items-center gap-2 mb-4">
            <span class="text-body-strong text-white">{{ plate.name }}</span>
            <span class="text-caption font-mono px-2 py-0.5 rounded-badge bg-white/10 text-white/60">
              {{ plate.count }}只
            </span>
            <span class="text-caption text-ai" v-if="plate.reason">{{ plate.reason }}</span>
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
                  <td class="font-mono text-white"><MiniChartPreview :symbol="stock.code" :name="stock.name">{{ stock.code }}</MiniChartPreview></td>
                  <td class="font-medium text-white"><MiniChartPreview :symbol="stock.code" :name="stock.name">{{ stock.name }}</MiniChartPreview></td>
                  <td class="font-mono">{{ stock.article?.action_info?.num || '-' }}</td>
                  <td class="text-white/60 text-xs" style="max-width: 300px">{{ stock.article?.action_info?.expound || '' }}</td>
                </tr>
              </tbody>
            </table>
          </div>
        </div>
      </div>
    </div>

    <!-- Sub-view B: 强势股池 -->
    <div v-else>
      <div class="flex items-center gap-3 mb-6 flex-wrap">
        <span class="text-caption text-ink-muted">
          共 <b class="text-white">{{ displayStrongStocks.length }}</b> 只强势股
        </span>
        <div class="flex items-center gap-1">
          <button v-for="s in sortOptions" :key="s.id" @click="strongSort = s.id"
            class="px-2 py-0.5 rounded-badge text-caption font-mono transition-colors cursor-pointer"
            :class="strongSort === s.id ? 'bg-ai/20 text-ai' : 'text-ink-muted hover:text-white/80'">
            {{ s.label }}
          </button>
        </div>
        <label class="flex items-center gap-2 cursor-pointer select-none text-ink-muted">
          <input type="checkbox" v-model="strongHideST" class="cursor-pointer"><span class="text-caption">隐藏ST</span>
        </label>
      </div>

      <div v-if="strongStock.loading" class="text-center py-8 text-caption text-ink-muted">加载中...</div>

      <div v-else-if="displayStrongStocks.length === 0" class="text-center py-8 text-caption text-ink-muted">
        暂无强势股数据
      </div>

      <div v-else class="bg-surface-dark rounded-card border border-hairline-dark overflow-hidden">
        <table class="data-table">
          <thead>
            <tr>
              <th>代码</th><th>名称</th><th style="text-align:right">涨跌幅</th>
              <th style="text-align:right">换手率</th><th style="text-align:right">流通市值</th><th>板块</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="s in displayStrongStocks" :key="s.symbol">
              <td class="cursor-pointer font-mono hover:underline text-ai" @click="openKline(s.symbol, s.stock_chi_name)">
                <MiniChartPreview :symbol="s.symbol" :name="s.stock_chi_name">{{ s.symbol }}</MiniChartPreview>
              </td>
              <td class="font-medium text-white">
                <MiniChartPreview :symbol="s.symbol" :name="s.stock_chi_name">{{ s.stock_chi_name }}</MiniChartPreview>
              </td>
              <td class="text-right font-mono" :class="s.change_percent >= 0 ? 'text-up' : 'text-down'">
                {{ s.change_percent >= 0 ? '+' : '' }}{{ (s.change_percent * 100).toFixed(2) }}%
              </td>
              <td class="text-right font-mono">{{ (s.turnover_ratio * 100).toFixed(2) }}%</td>
              <td class="text-right font-mono">{{ (s.total_capital / 1e8).toFixed(1) }}亿</td>
              <td class="text-white/40 text-xs">
                {{ s.surge_reason?.related_plates?.map((p: any) => p.plate_name).join(', ') || '-' }}
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, inject, onMounted, watch } from 'vue'
import { motion } from 'motion-v'
import { limitUp, fieldItems, overview, superStock as strongStock, fetchLimitUp, fetchFieldItems, fetchSuperStock } from '@/stores/data'
import MiniChartPreview from '@/components/MiniChartPreview.vue'

const openKline = inject<(symbol: string, name: string) => void>('openKline', () => {})
const setSubView = inject<(v: string) => void>('setSubView', () => {})
const hideST = ref(true)

const fieldDate = computed({
  get: () => fieldItems.date,
  set: (val: string) => fetchFieldItems(val),
})

const stocks = computed(() => limitUp.data)
const bombRate = computed(() => overview.data?.bomb_rate || 0)
const fieldPlates = computed(() => fieldItems.data)

const sortedStocks = computed(() =>
  [...stocks.value].sort((a, b) => b.limit_up_days - a.limit_up_days || b.total_capital - a.total_capital)
)

const displayStocks = computed(() => {
  if (!hideST.value) return sortedStocks.value
  return sortedStocks.value.filter(s => !s.stock_chi_name.includes('ST') && !s.stock_chi_name.includes('*'))
})

const totalFieldStocks = computed(() =>
  fieldPlates.value.reduce((sum: number, p: any) => sum + (p.list?.length || 0), 0)
)

// Sub-view toggle
const activeSubView = ref<'limit-up' | 'strong-pool'>('limit-up')

const strongCount = computed(() => displayStrongStocks.value.length)

const subViews = computed(() => [
  { id: 'limit-up' as const, label: '涨停股', count: `${displayStocks.value.length}只` },
  { id: 'strong-pool' as const, label: '强势股池', count: `${strongCount.value}只` },
])

// Report sub-view to AiSidebar
watch(activeSubView, (v) => setSubView(v), { immediate: true })

// Filter pills
const activeFilter = ref('all')
const filters = [
  { id: 'all', label: '全部' },
  { id: 'first', label: '首板' },
  { id: 'multi', label: '2板+' },
  { id: 'leader', label: '龙头' },
]

const filteredStocks = computed(() => {
  if (activeFilter.value === 'first') return displayStocks.value.filter(s => s.limit_up_days === 1)
  if (activeFilter.value === 'multi') return displayStocks.value.filter(s => s.limit_up_days >= 2)
  if (activeFilter.value === 'leader') return displayStocks.value.filter(s => s.limit_up_days >= 3)
  return displayStocks.value
})

// Strong pool sort options
const strongSort = ref('change')
const sortOptions = [
  { id: 'change', label: '按涨幅' },
  { id: 'turnover', label: '按换手' },
  { id: 'capital', label: '按市值' },
]
const strongHideST = ref(true)

const displayStrongStocks = computed(() => {
  let list = [...strongStock.data]
  if (strongHideST.value) {
    list = list.filter(s => !s.stock_chi_name.includes('ST') && !s.stock_chi_name.includes('*'))
  }
  if (strongSort.value === 'change') {
    list.sort((a, b) => b.change_percent - a.change_percent)
  } else if (strongSort.value === 'turnover') {
    list.sort((a, b) => b.turnover_ratio - a.turnover_ratio)
  } else if (strongSort.value === 'capital') {
    list.sort((a, b) => b.total_capital - a.total_capital)
  }
  return list
})

async function onDateChange(e: Event) {
  const val = (e.target as HTMLInputElement).value
  if (!val) return
  await fetchFieldItems(val)
}

onMounted(() => {
  fetchLimitUp()
  fetchFieldItems()
  fetchSuperStock()
})
</script>