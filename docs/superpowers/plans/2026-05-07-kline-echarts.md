# K-line Chart: lightweight-charts → ECharts Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace `lightweight-charts` with `echarts` in KlineDrawer.vue, feature-parity, fix 1-min time axis for A-share trading hours.

**Architecture:** Single file rewrite — `KlineDrawer.vue` uses `echarts.init()` directly (no vue-echarts wrapper). Two ECharts grids stacked vertically: candlestick on top, volume bars below. Crosshair via `tooltip` + `axisPointer`. Limit-up/down lines via `markLine`. 1-min period fixes trading hours by filtering data to 9:30-11:30 + 13:00-15:00 segments and constraining `xAxis.min`/`max`.

**Tech Stack:** Vue 3 + TypeScript + echarts

---

### Task 1: Install echarts, remove lightweight-charts

**Files:**
- Modify: `src/package.json`
- Run: `bun install` (updates `bun.lock`)

- [ ] **Step 1: Swap dependencies in package.json**

Replace:
```json
"lightweight-charts": "^5.2.0",
```
with:
```json
"echarts": "^5.6.0",
```

- [ ] **Step 2: Install and update lockfile**

Run: `cd src && bun install`
Expected: `echarts` installed, `lightweight-charts` removed, `bun.lock` updated.

- [ ] **Step 3: Commit**

```bash
git add src/package.json src/bun.lock
git commit -m "deps: swap lightweight-charts for echarts"
```

---

### Task 2: Rewrite KlineDrawer.vue with ECharts

**Files:**
- Modify: `src/src/components/KlineDrawer.vue` (full rewrite)

- [ ] **Step 1: Write the new KlineDrawer.vue**

Replace the entire file content with:

```vue
<template>
  <Teleport to="body">
    <div @click="$emit('close')" class="fixed inset-0 z-40" style="background: rgba(0,0,0,0.4)"></div>

    <div class="fixed top-0 right-0 h-full z-50 overflow-y-auto" style="width: 520px; background: var(--bg-secondary); border-left: 1px solid var(--border-subtle)">
      <!-- Header -->
      <div class="sticky top-0 z-10 p-4" style="background: var(--bg-secondary); border-bottom: 1px solid var(--border-subtle)">
        <div class="flex items-center justify-between mb-2">
          <div class="flex items-center gap-2">
            <span class="text-sm font-bold" style="color: var(--text-primary)">{{ name }}</span>
            <span class="text-xs font-mono" style="color: var(--text-muted)">{{ symbol }}</span>
          </div>
          <button @click="$emit('close')" class="text-lg leading-none p-1" style="color: var(--text-muted)">&times;</button>
        </div>
        <div v-if="quote" class="flex items-center gap-3">
          <span class="text-xl font-bold font-mono" :style="{ color: quote.px_change_rate >= 0 ? 'var(--red)' : 'var(--green)' }">{{ quote.last_px?.toFixed(2) }}</span>
          <span class="text-sm font-mono" :style="{ color: quote.px_change_rate >= 0 ? 'var(--red)' : 'var(--green)' }">{{ quote.px_change_rate >= 0 ? '+' : '' }}{{ quote.px_change_rate?.toFixed(2) }}%</span>
          <span class="text-xs" style="color: var(--text-muted)">成交 {{ formatVolume(quote.turnover_value) }}</span>
        </div>
      </div>

      <!-- Period Toggle -->
      <div class="flex gap-1 px-4 py-2 sticky z-10" style="background: var(--bg-secondary); border-bottom: 1px solid var(--border-subtle); top: 97px">
        <button v-for="p in periods" :key="p.value" @click="period = p.value"
          class="px-3 py-1 text-xs rounded font-mono transition"
          :style="period === p.value ? { background: 'var(--blue)', color: '#fff' } : { background: 'transparent', color: 'var(--text-muted)' }">
          {{ p.label }}
        </button>
      </div>

      <!-- Crosshair Info Bar -->
      <div class="flex items-center justify-between px-4 py-1 font-mono" style="background: #1a2332; border-bottom: 1px solid #1e2a3a; min-height: 30px; font-size: 11px">
        <span style="color: var(--text-primary)">{{ crosshair.time }}</span>
        <div class="flex items-center gap-2" style="color: var(--text-muted)">
          <span>开 <span style="color: var(--text-primary)">{{ crosshair.open }}</span></span>
          <span>高 <span style="color: var(--text-primary)">{{ crosshair.high }}</span></span>
          <span>低 <span style="color: var(--text-primary)">{{ crosshair.low }}</span></span>
          <span>收 <span style="color: var(--text-primary)">{{ crosshair.close }}</span></span>
          <span>量 <span style="color: var(--text-primary)">{{ crosshair.volume }}</span></span>
          <span :style="{ color: crosshair.changeNum >= 0 ? 'var(--red)' : 'var(--green)' }">{{ crosshair.change }}</span>
        </div>
      </div>

      <!-- Chart -->
      <div ref="chartContainer" class="w-full" style="height: 420px"></div>

      <!-- Metrics -->
      <div v-if="quote" class="p-4 grid grid-cols-3 gap-2 text-xs">
        <div v-for="m in metrics" :key="m.label" class="glow-card p-3 text-center">
          <p style="color: var(--text-muted); font-size: 11px">{{ m.label }}</p>
          <p class="font-mono mt-1" style="color: var(--text-primary); font-size: 12px">{{ m.value }}</p>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { ref, watch, onMounted, onUnmounted, computed } from 'vue'
import * as echarts from 'echarts'

const props = defineProps<{ symbol: string; name: string }>()
defineEmits<{ close: [] }>()

const periods = [
  { label: '1分', value: 'minute' },
  { label: '日K', value: 'day' },
  { label: '周K', value: 'week' },
  { label: '月K', value: 'month' },
]

const period = ref('day')
const chartContainer = ref<HTMLElement>()
const quote = ref<any>(null)

const crosshair = ref({ visible: false, time: '', open: '-', high: '-', low: '-', close: '-', volume: '-', change: '', changeNum: 0 })

let chart: echarts.ECharts | null = null
let klineData: any[] = []

// A-share trading sessions (minutes since midnight)
const TRADING_SESSIONS = [[570, 690], [780, 900]] // 9:30-11:30, 13:00-15:00

function isTradingTime(ts: number): boolean {
  const d = new Date(ts * 1000)
  const mins = d.getHours() * 60 + d.getMinutes()
  return TRADING_SESSIONS.some(([s, e]) => mins >= s && mins < e)
}

function formatVolume(v: number) {
  if (!v) return '-'
  if (v > 1e8) return (v / 1e8).toFixed(2) + '亿'
  if (v > 1e4) return (v / 1e4).toFixed(2) + '万'
  return v.toFixed(0)
}

const metrics = computed(() => {
  if (!quote.value) return []
  return [
    { label: '今开', value: quote.value.open_px?.toFixed(2) || '-' },
    { label: '昨收', value: quote.value.preclose_px?.toFixed(2) || '-' },
    { label: '最高', value: quote.value.high_px?.toFixed(2) || '-' },
    { label: '最低', value: quote.value.low_px?.toFixed(2) || '-' },
    { label: '振幅', value: quote.value.amplitude != null ? quote.value.amplitude.toFixed(2) + '%' : '-' },
    { label: '换手率', value: quote.value.turnover_ratio != null ? quote.value.turnover_ratio.toFixed(2) + '%' : '-' },
    { label: '动态PE', value: quote.value.dyn_pe ? quote.value.dyn_pe.toFixed(1) : '-' },
    { label: '市净率', value: quote.value.dyn_pb_rate ? quote.value.dyn_pb_rate.toFixed(2) : '-' },
    { label: '总市值', value: formatVolume(quote.value.market_value) },
    { label: '流通市值', value: formatVolume(quote.value.circulation_value) },
    { label: '量比', value: quote.value.volume_ratio?.toFixed(2) || '-' },
    { label: '成交量', value: formatVolume(quote.value.turnover_volume) + '股' },
  ]
})

function getLimitRate(symbol: string, name: string): number {
  if (name?.includes('ST')) return 0.05
  if (symbol.startsWith('688')) return 0.20
  if (/^30[01]/.test(symbol)) return 0.20
  if (/^8/.test(symbol)) return 0.30
  return 0.10
}

function formatTime(ts: number, p: string): string {
  const d = new Date(ts * 1000)
  const pad = (n: number) => String(n).padStart(2, '0')
  const ymd = `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`
  if (p === 'minute') return `${ymd} ${pad(d.getHours())}:${pad(d.getMinutes())}`
  return ymd
}

function getApiBase(): string {
  return localStorage.getItem('openatrading-api-url') || 'http://localhost:8080'
}

async function fetchRealtime() {
  try {
    const resp = await fetch(`${getApiBase()}/api/realtime/${props.symbol}`)
    if (resp.ok) quote.value = await resp.json()
  } catch (e) { console.error('[KlineDrawer] fetchRealtime error:', e) }
}

async function fetchKline() {
  try {
    const resp = await fetch(`${getApiBase()}/api/kline/${props.symbol}?period=${period.value}&count=256`)
    if (!resp.ok) return
    const data = await resp.json()
    klineData = data.candles || []

    // For 1-min period, filter to trading hours only
    if (period.value === 'minute') {
      klineData = klineData.filter((c: any) => isTradingTime(c.time))
    }

    renderChart()

    // Init crosshair bar with last candle
    if (klineData.length > 0) {
      const last = klineData[klineData.length - 1]
      const preclose = quote.value?.preclose_px
      const changeRate = preclose ? ((last.close - preclose) / preclose * 100) : 0
      crosshair.value = {
        visible: true,
        time: formatTime(last.time, period.value),
        open: last.open?.toFixed(2) || '-',
        high: last.high?.toFixed(2) || '-',
        low: last.low?.toFixed(2) || '-',
        close: last.close?.toFixed(2) || '-',
        volume: formatVolume(last.volume || 0),
        change: changeRate >= 0 ? `+${changeRate.toFixed(2)}%` : `${changeRate.toFixed(2)}%`,
        changeNum: changeRate,
      }
    }
  } catch (e) { console.error('[KlineDrawer] fetchKline error:', e) }
}

function buildMarkLines() {
  if (!quote.value?.preclose_px) return []
  const preclose = quote.value.preclose_px
  const lines: any[] = []

  if (period.value === 'minute') {
    // 昨收零轴线
    lines.push({
      yAxis: preclose,
      lineStyle: { color: 'rgba(255,255,255,0.30)', type: 'dashed', width: 1 },
      label: { show: true, formatter: `昨收 ${preclose.toFixed(2)}`, color: '#8899aa', fontSize: 10 },
      symbol: 'none',
    })

    const rate = getLimitRate(props.symbol, props.name)
    const limitUp = Math.round(preclose * (1 + rate) * 100) / 100
    const limitDown = Math.round(preclose * (1 - rate) * 100) / 100

    lines.push({
      yAxis: limitUp,
      lineStyle: { color: '#ff1744', type: 'dashed', width: 1 },
      label: { show: true, formatter: `涨停 ${limitUp.toFixed(2)}`, color: '#ff1744', fontSize: 10 },
      symbol: 'none',
    })
    lines.push({
      yAxis: limitDown,
      lineStyle: { color: '#00e676', type: 'dashed', width: 1 },
      label: { show: true, formatter: `跌停 ${limitDown.toFixed(2)}`, color: '#00e676', fontSize: 10 },
      symbol: 'none',
    })
  }
  return lines
}

function renderChart() {
  if (!chartContainer.value) return

  if (!chart) {
    chart = echarts.init(chartContainer.value, 'dark')
    chart.on('updateAxisPointer', (event: any) => {
      const dataIndex = event.dataIndex
      if (dataIndex == null || dataIndex < 0 || dataIndex >= klineData.length) return
      const c = klineData[dataIndex]
      const preclose = quote.value?.preclose_px
      const changeRate = preclose ? ((c.close - preclose) / preclose * 100) : 0
      crosshair.value = {
        visible: true,
        time: formatTime(c.time, period.value),
        open: c.open?.toFixed(2) || '-',
        high: c.high?.toFixed(2) || '-',
        low: c.low?.toFixed(2) || '-',
        close: c.close?.toFixed(2) || '-',
        volume: formatVolume(c.volume || 0),
        change: changeRate >= 0 ? `+${changeRate.toFixed(2)}%` : `${changeRate.toFixed(2)}%`,
        changeNum: changeRate,
      }
    })
  }

  const timestamps = klineData.map((c: any) => c.time)
  const candleData = klineData.map((c: any) => [c.open, c.close, c.low, c.high])
  const volumeData = klineData.map((c: any, i: number) => [i, c.volume, c.close >= c.open ? 1 : -1])
  const preclose = quote.value?.preclose_px

  // 1-min: compute y-axis range centered on preclose
  let yMin: any = 'dataMin'
  let yMax: any = 'dataMax'
  if (period.value === 'minute' && preclose && klineData.length > 0) {
    let maxUpper = 0, maxLower = 0
    for (const c of klineData) {
      if (c.high > preclose) maxUpper = Math.max(maxUpper, c.high - preclose)
      if (c.low < preclose) maxLower = Math.max(maxLower, preclose - c.low)
    }
    const range = Math.max(maxUpper, maxLower) * 1.15 || preclose * 0.02
    yMin = preclose - range
    yMax = preclose + range
  }

  const option: echarts.EChartsOption = {
    backgroundColor: '#111827',
    grid: [
      { left: 8, right: 50, top: 8, height: '65%' },
      { left: 8, right: 50, top: '75%', height: '15%' },
    ],
    xAxis: [
      {
        type: 'category',
        data: timestamps,
        gridIndex: 0,
        axisLine: { lineStyle: { color: '#1e2a3a' } },
        axisTick: { show: false },
        axisLabel: {
          color: '#8899aa',
          fontSize: 10,
          formatter: (value: number) => formatTime(value, period.value),
          interval: Math.max(Math.floor(klineData.length / 6), 1),
        },
        splitLine: { show: false },
      },
      {
        type: 'category',
        data: timestamps,
        gridIndex: 1,
        axisLine: { lineStyle: { color: '#1e2a3a' } },
        axisTick: { show: false },
        axisLabel: { show: false },
        splitLine: { show: false },
      },
    ],
    yAxis: [
      {
        type: 'value',
        gridIndex: 0,
        scale: true,
        min: yMin,
        max: yMax,
        axisLine: { lineStyle: { color: '#1e2a3a' } },
        axisLabel: { color: '#8899aa', fontSize: 10 },
        splitLine: { lineStyle: { color: '#1e2a3a' } },
      },
      {
        type: 'value',
        gridIndex: 1,
        axisLine: { lineStyle: { color: '#1e2a3a' } },
        axisLabel: { color: '#8899aa', fontSize: 10, formatter: (v: number) => formatVolume(v) },
        splitLine: { show: false },
        min: 0,
      },
    ],
    tooltip: { show: false },
    axisPointer: {
      link: [{ xAxisIndex: 'all' }],
      triggerOn: 'mousemove',
    },
    series: [
      {
        type: 'candlestick',
        xAxisIndex: 0,
        yAxisIndex: 0,
        data: candleData,
        itemStyle: {
          color: '#ff1744',
          color0: '#00e676',
          borderColor: '#ff1744',
          borderColor0: '#00e676',
        },
        markLine: {
          silent: true,
          symbol: 'none',
          data: buildMarkLines(),
        },
      },
      {
        type: 'bar',
        xAxisIndex: 1,
        yAxisIndex: 1,
        data: volumeData,
        itemStyle: {
          color: (params: any) => params.data?.[2] >= 0 ? 'rgba(255,23,68,0.3)' : 'rgba(0,230,118,0.3)',
        },
      },
    ],
  }

  chart.setOption(option, true)
}

watch(period, () => fetchKline())

onMounted(async () => {
  await fetchRealtime()
  fetchKline()
})

onUnmounted(() => {
  chart?.dispose()
  chart = null
})
</script>
```

- [ ] **Step 2: Type check**

Run: `cd src && npx vue-tsc --noEmit`
Expected: No errors.

- [ ] **Step 3: Build**

Run: `cd src && bun run build`
Expected: Build succeeds.

- [ ] **Step 4: Commit**

```bash
git add src/src/components/KlineDrawer.vue
git commit -m "refactor: replace lightweight-charts with echarts for K-line chart"
```

---

### Task 3: Integration test with backend

- [ ] **Step 1: Start backend**

Run: `cd /Users/user/Desktop/github/openatrading && cargo run -- run --web`
Expected: Backend starts on port 8080, initial report fetched.

- [ ] **Step 2: Start frontend dev server**

Run: `cd src && bun run dev`
Expected: Vite dev server starts (usually port 5173).

- [ ] **Step 3: Open browser and test**

Open `http://localhost:5173` and verify:
1. Click a stock symbol in 涨停板 tab → K-line drawer opens
2. Daily K-line shows candlestick + volume
3. Crosshair info bar updates on mouse move
4. Switch to 1分 → chart shows, time axis only shows 9:30-15:00 range
5. 涨跌停线 visible on 1-min
6. 昨收零轴线 visible on 1-min
7. Switch 日K → 周K → 月K, each loads correctly
8. Close drawer, reopen with another stock

- [ ] **Step 4: Commit any fixes if needed**
