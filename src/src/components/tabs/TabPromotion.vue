<template>
  <div>
    <!-- Loading -->
    <div v-if="loading" class="flex items-center justify-center py-16 gap-3">
      <span class="w-2 h-2 rounded-full bg-primary animate-pulse"></span>
      <span class="text-caption font-mono text-white/40">加载晋级数据...</span>
    </div>

    <!-- Error -->
    <div v-else-if="error" class="bg-surface-dark rounded-card border border-hairline-dark p-card-inner text-center text-caption text-white/40">
      {{ error }}
    </div>

    <template v-else-if="ladder.length > 0">
      <!-- Promotion Ladder Overview -->
      <div class="mb-12">
        <div class="flex items-center gap-3 mb-6">
          <h3 class="text-display-md text-white">连板晋级梯队</h3>
          <span class="text-caption text-white/40">
            昨 {{ totalYesterday }} 只 → 今 {{ totalToday }} 只 · 综合晋级率
            <b :class="overallRate >= 0.5 ? 'text-data-red' : 'text-white/40'">{{ (overallRate * 100).toFixed(1) }}%</b>
          </span>
        </div>

        <div class="bg-surface-dark rounded-card border border-hairline-dark p-card-inner">
          <div
            v-for="row in ladder"
            :key="row.yesterdayDays"
            class="mb-3 last:mb-0 cursor-pointer"
            @click="selectedBoard = selectedBoard === row.yesterdayDays ? null : row.yesterdayDays"
          >
            <div class="flex items-center gap-3 text-caption font-mono">
              <span class="px-2 py-0.5 rounded-badge text-caption bg-white/10 text-white/60" style="min-width: 56px; text-align: center">
                昨{{ row.yesterdayDays }}板
              </span>
              <span class="text-white/40" style="min-width: 40px">{{ row.yesterdayCount }}只</span>
              <span class="text-white/40">→</span>
              <span class="px-2 py-0.5 rounded-badge text-caption" :class="row.rate >= 0.5 ? 'bg-data-red/15 text-data-red' : 'bg-data-green/15 text-data-green'" style="min-width: 56px; text-align: center">
                今{{ row.yesterdayDays + 1 }}板
              </span>
              <span class="text-white" style="min-width: 40px">{{ row.promotedCount }}只</span>
              <span class="text-white/40 text-caption">晋级率</span>
              <b :class="row.rate >= 0.5 ? 'text-data-red' : 'text-white/40'">{{ (row.rate * 100).toFixed(1) }}%</b>
              <div class="flex-1 h-2 rounded-full bg-white/10" style="min-width: 60px">
                <div class="h-full rounded-full transition-all" :style="{ width: (row.rate * 100) + '%' }" :class="row.rate >= 0.5 ? 'bg-data-red' : 'bg-data-green'"></div>
              </div>
              <span class="text-white/40 text-caption">断板 {{ row.brokenCount }}只</span>
            </div>

            <!-- Expanded detail -->
            <div v-if="selectedBoard === row.yesterdayDays" class="mt-3 ml-4">
              <table class="data-table">
                <thead>
                  <tr>
                    <th>代码</th>
                    <th>名称</th>
                    <th>昨板→今板</th>
                    <th style="text-align:right">换手率</th>
                    <th style="text-align:right">流通市值</th>
                    <th>状态</th>
                  </tr>
                </thead>
                <tbody>
                  <tr v-for="s in row.details" :key="s.symbol">
                    <td class="cursor-pointer font-mono hover:underline text-primary" @click="openKline(s.symbol, s.name)"><MiniChartPreview :symbol="s.symbol" :name="s.name">{{ s.symbol }}</MiniChartPreview></td>
                    <td class="font-medium text-white"><MiniChartPreview :symbol="s.symbol" :name="s.name">{{ s.name }}</MiniChartPreview></td>
                    <td class="font-mono">
                      <span :class="s.status === 'promoted' ? 'text-data-red' : 'text-white/40'">
                        {{ s.yesterdayDays }}板 → {{ s.status === 'promoted' ? s.todayDays + '板' : '断板' }}
                      </span>
                    </td>
                    <td class="text-right font-mono">{{ s.turnover ? (s.turnover * 100).toFixed(2) + '%' : '-' }}</td>
                    <td class="text-right font-mono">{{ (s.capital / 1e8).toFixed(1) }}亿</td>
                    <td>
                      <span class="px-2 py-0.5 rounded-badge text-caption font-mono"
                        :class="s.status === 'promoted' ? 'bg-data-red/15 text-data-red' : 'bg-white/10 text-white/40'">
                        {{ s.status === 'promoted' ? '晋级' : '断板' }}
                      </span>
                    </td>
                  </tr>
                </tbody>
              </table>
            </div>
          </div>
        </div>
      </div>

      <!-- First Board (首板新进) -->
      <div v-if="firstBoard.length > 0">
        <div class="flex items-center gap-3 mb-6">
          <h3 class="text-display-md text-white">首板新进</h3>
          <span class="text-caption text-white/40">共 {{ firstBoard.length }} 只</span>
        </div>
        <div class="bg-surface-dark rounded-card border border-hairline-dark overflow-hidden">
          <table class="data-table">
            <thead>
              <tr>
                <th>代码</th>
                <th>名称</th>
                <th style="text-align:right">涨跌幅</th>
                <th style="text-align:right">换手率</th>
                <th style="text-align:right">流通市值</th>
                <th>板块</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="s in firstBoard" :key="s.symbol">
                <td class="cursor-pointer font-mono hover:underline text-primary" @click="openKline(s.symbol, s.name)"><MiniChartPreview :symbol="s.symbol" :name="s.name">{{ s.symbol }}</MiniChartPreview></td>
                <td class="font-medium text-white"><MiniChartPreview :symbol="s.symbol" :name="s.name">{{ s.name }}</MiniChartPreview></td>
                <td class="text-right font-mono text-data-green">+{{ (s.changePercent * 100).toFixed(2) }}%</td>
                <td class="text-right font-mono">{{ (s.turnover * 100).toFixed(2) }}%</td>
                <td class="text-right font-mono">{{ (s.capital / 1e8).toFixed(1) }}亿</td>
                <td class="text-white/40 text-caption">{{ s.reason || '-' }}</td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>
    </template>

    <!-- Empty -->
    <div v-else class="bg-surface-dark rounded-card border border-hairline-dark p-card-inner text-center text-caption text-white/40">
      今日无涨停数据
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, inject } from 'vue'
import { limitUp, fetchLimitUp } from '@/stores/data'
import MiniChartPreview from '@/components/MiniChartPreview.vue'
const openKline = inject<(symbol: string, name: string) => void>('openKline', () => {})

const loading = ref(true)
const error = ref('')
const selectedBoard = ref<number | null>(null)

interface PromotionItem {
  symbol: string
  name: string
  yesterdayDays: number
  todayDays: number
  status: 'promoted' | 'broken' | 'first'
  turnover: number
  capital: number
  changePercent: number
  reason: string
}

function getApiBase(): string {
  return localStorage.getItem('trading-agent-api-url') || __API_BASE_URL__
}

// ─── Fetch ─────────────────────────────────────────

async function fetchYesterdayLimitUp(): Promise<any[] | null> {
  try {
    const resp = await fetch(`${getApiBase()}/api/yesterday-limit-up`)
    if (resp.ok) return await resp.json()
  } catch { /* ignore */ }
  return null
}

// ─── Merge ─────────────────────────────────────────

function mergeData(todayList: any[], yesterdayList: any[]): PromotionItem[] {
  const yesterdayMap = new Map<string, any>()
  for (const s of yesterdayList) yesterdayMap.set(s.symbol, s)

  const result: PromotionItem[] = []

  for (const s of todayList) {
    const prev = yesterdayMap.get(s.symbol)
    const plates = s.surge_reason?.related_plates?.map((p: any) => p.plate_name).join(', ') || ''
    if (prev && prev.limit_up_days > 0) {
      result.push({
        symbol: s.symbol, name: s.stock_chi_name,
        yesterdayDays: prev.limit_up_days, todayDays: s.limit_up_days,
        status: 'promoted', turnover: s.turnover_ratio,
        capital: s.total_capital, changePercent: s.change_percent,
        reason: plates,
      })
      yesterdayMap.delete(s.symbol)
    } else {
      result.push({
        symbol: s.symbol, name: s.stock_chi_name,
        yesterdayDays: 0, todayDays: s.limit_up_days,
        status: 'first', turnover: s.turnover_ratio,
        capital: s.total_capital, changePercent: s.change_percent,
        reason: plates,
      })
    }
  }

  for (const s of yesterdayMap.values()) {
    result.push({
      symbol: s.symbol, name: s.stock_chi_name,
      yesterdayDays: s.limit_up_days, todayDays: 0,
      status: 'broken', turnover: s.turnover_ratio || 0,
      capital: s.total_capital || 0, changePercent: 0,
      reason: '',
    })
  }

  return result
}

// ─── Computed ──────────────────────────────────────

const allItems = ref<PromotionItem[]>([])

const firstBoard = computed(() =>
  allItems.value.filter(s => s.status === 'first').sort((a, b) => b.capital - a.capital)
)

const promoted = computed(() => allItems.value.filter(s => s.status === 'promoted'))
const broken = computed(() => allItems.value.filter(s => s.status === 'broken'))

const totalYesterday = computed(() => promoted.value.length + broken.value.length)
const totalToday = computed(() => promoted.value.length + firstBoard.value.length)

const overallRate = computed(() => {
  const denom = totalYesterday.value
  return denom > 0 ? promoted.value.length / denom : 0
})

interface LadderRow {
  yesterdayDays: number
  yesterdayCount: number
  promotedCount: number
  brokenCount: number
  rate: number
  details: PromotionItem[]
}

const ladder = computed(() => {
  const groups = new Map<number, { promoted: PromotionItem[]; broken: PromotionItem[] }>()
  for (const s of promoted.value) {
    const g = groups.get(s.yesterdayDays) || { promoted: [], broken: [] }
    g.promoted.push(s)
    groups.set(s.yesterdayDays, g)
  }
  for (const s of broken.value) {
    const g = groups.get(s.yesterdayDays) || { promoted: [], broken: [] }
    g.broken.push(s)
    groups.set(s.yesterdayDays, g)
  }

  const rows: LadderRow[] = []
  for (const [days, g] of groups) {
    const total = g.promoted.length + g.broken.length
    rows.push({
      yesterdayDays: days,
      yesterdayCount: total,
      promotedCount: g.promoted.length,
      brokenCount: g.broken.length,
      rate: total > 0 ? g.promoted.length / total : 0,
      details: [...g.promoted, ...g.broken].sort((a, b) => {
        if (a.status === 'promoted' && b.status !== 'promoted') return -1
        if (a.status !== 'promoted' && b.status === 'promoted') return 1
        return b.capital - a.capital
      }),
    })
  }
  return rows.sort((a, b) => b.yesterdayDays - a.yesterdayDays)
})

// ─── Init ──────────────────────────────────────────

onMounted(async () => {
  await fetchLimitUp()
  const todayList = limitUp.data || []
  if (todayList.length === 0) {
    loading.value = false
    error.value = '今日无涨停数据'
    return
  }

  const yesterdayList = await fetchYesterdayLimitUp()
  if (!yesterdayList || yesterdayList.length === 0) {
    loading.value = false
    error.value = '未能获取昨日涨停数据'
    return
  }

  allItems.value = mergeData(todayList, yesterdayList)
  loading.value = false
})
</script>
