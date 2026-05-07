# Apple Design System Redesign — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Redesign the Vue 3 SPA frontend with Apple Design System principles — light shell, dark data surfaces, single blue accent, Inter typography, full-width tile layout.

**Architecture:** Tailwind-first approach. Design tokens defined in `tailwind.config.cjs`, components rewritten to use Tailwind utilities instead of inline `style` bindings. Full-width tile layout in `App.vue`. No structural changes to component tree or data flow.

**Tech Stack:** Vue 3 + TypeScript + Tailwind CSS v3 + Pinia + ECharts

**Spec:** `docs/superpowers/specs/2026-05-07-apple-redesign-design.md`

---

## File Manifest

| File | Action | Responsibility |
|------|--------|----------------|
| `tailwind.config.cjs` | Modify | Apple design tokens (colors, fonts, sizes, radii, spacing) |
| `src/src/style.css` | Modify | Base reset, Inter font import, remove old CSS variables |
| `src/index.html` | Modify | Inter font preload, title update |
| `src/src/App.vue` | Modify | Full-width tile layout shell (nav + KPI tile + tab tile + data tile) |
| `src/src/components/TabBar.vue` | Modify | Apple-style tab nav (no emoji, blue underline active) |
| `src/src/components/KpiCards.vue` | Modify | 4-column KPI cards + risk bar, Apple card style |
| `src/src/components/tabs/TabLimitUp.vue` | Modify | Data table in dark tile, field items cards |
| `src/src/components/tabs/TabPlate.vue` | Modify | Plate/stock event cards + dark data table |
| `src/src/components/tabs/TabPromotion.vue` | Modify | Ladder rows + expand detail table + first board table |
| `src/src/components/tabs/TabSentiment.vue` | Modify | Article cards, stock ranking, taoguba articles |
| `src/src/components/tabs/TabTimeline.vue` | Modify | Day-grouped timeline cards |
| `src/src/components/tabs/TabAi.vue` | Modify | Placeholder states |
| `src/src/components/tabs/TabTgb.vue` | Modify | Placeholder states |
| `src/src/components/KlineDrawer.vue` | Modify | Dark drawer with ECharts, restyle to Apple dark palette |
| `src/src/components/HelloWorld.vue` | Delete | Unused Vue template scaffold |

---

### Task 1: Tailwind Config — Apple Design Tokens

**Files:**
- Modify: `tailwind.config.cjs`

- [ ] **Step 1: Replace tailwind.config.cjs with Apple tokens**

```js
const plugin = require('tailwindcss/plugin')

module.exports = {
  content: [
    './src/**/*.{vue,js,ts,jsx,tsx}',
    './index.html',
  ],
  theme: {
    extend: {
      colors: {
        canvas: {
          DEFAULT: '#ffffff',
          parchment: '#f5f5f7',
        },
        'surface-dark': {
          DEFAULT: '#272729',
          2: '#2a2a2c',
          3: '#252527',
        },
        ink: {
          DEFAULT: '#1d1d1f',
          muted: '#86868b',
        },
        primary: {
          DEFAULT: '#0066cc',
          focus: '#0071e3',
          'on-dark': '#2997ff',
        },
        'data-green': '#34c759',
        'data-red': '#ff3b30',
      },
      fontFamily: {
        sans: ['Inter', 'system-ui', '-apple-system', 'BlinkMacSystemFont', 'sans-serif'],
        mono: ['JetBrains Mono', 'monospace'],
      },
      fontSize: {
        'display-lg': ['40px', { lineHeight: '1.1', letterSpacing: '-0.5px', fontWeight: '600' }],
        'display-md': ['28px', { lineHeight: '1.14', letterSpacing: '-0.3px', fontWeight: '600' }],
        body: ['17px', { lineHeight: '1.47', letterSpacing: '-0.3px' }],
        'body-strong': ['17px', { lineHeight: '1.24', letterSpacing: '-0.3px', fontWeight: '600' }],
        caption: ['14px', { lineHeight: '1.43', letterSpacing: '-0.2px' }],
        'caption-strong': ['14px', { lineHeight: '1.29', letterSpacing: '-0.2px', fontWeight: '600' }],
        'nav-link': ['12px', { lineHeight: '1.0', letterSpacing: '-0.1px' }],
      },
      borderRadius: {
        card: '18px',
        button: '9999px',
        input: '18px',
        badge: '5px',
      },
      spacing: {
        'section-y': '80px',
        'card-inner': '24px',
      },
      borderColor: {
        hairline: 'rgba(0, 0, 0, 0.08)',
        'hairline-dark': 'rgba(255, 255, 255, 0.06)',
      },
    },
  },
  plugins: [],
}
```

- [ ] **Step 2: Commit**

```bash
git add tailwind.config.cjs
git commit -m "feat: add Apple design tokens to Tailwind config"
```

---

### Task 2: Base Styles & HTML

**Files:**
- Modify: `src/src/style.css`
- Modify: `src/index.html`

- [ ] **Step 1: Rewrite style.css**

```css
@tailwind base;
@tailwind components;
@tailwind utilities;

@import url('https://fonts.googleapis.com/css2?family=Inter:wght@300;400;500;600;700&family=JetBrains+Mono:wght@400;500;700&display=swap');

@layer base {
  * { box-sizing: border-box; margin: 0; padding: 0; }

  body {
    font-family: 'Inter', system-ui, -apple-system, BlinkMacSystemFont, sans-serif;
    background: #ffffff;
    color: #1d1d1f;
    -webkit-font-smoothing: antialiased;
    -moz-osx-font-smoothing: grayscale;
  }

  ::-webkit-scrollbar { width: 6px; }
  ::-webkit-scrollbar-track { background: #f5f5f7; }
  ::-webkit-scrollbar-thumb { background: #86868b; border-radius: 3px; }

  ::selection { background: rgba(0, 102, 204, 0.15); color: #1d1d1f; }
}

@layer components {
  .data-table {
    width: 100%;
    border-collapse: collapse;
  }
  .data-table th {
    text-align: left;
    padding: 10px 16px;
    color: #86868b;
    font-weight: 600;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
  }
  .data-table td {
    padding: 10px 16px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.04);
    color: #a1a1a6;
    font-family: 'JetBrains Mono', monospace;
    font-size: 13px;
  }
  .data-table tr:hover td {
    background: rgba(255, 255, 255, 0.04);
    color: #ffffff;
  }
}
```

- [ ] **Step 2: Update index.html**

```html
<!DOCTYPE html>
<html lang="zh-CN">
  <head>
    <meta charset="UTF-8" />
    <link rel="icon" type="image/svg+xml" href="/vite.svg" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <link rel="preconnect" href="https://fonts.googleapis.com" />
    <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin />
    <title>市场异动追踪</title>
  </head>
  <body>
    <div id="app"></div>
    <script type="module" src="/src/main.ts"></script>
  </body>
</html>
```

- [ ] **Step 3: Commit**

```bash
git add src/src/style.css src/index.html
git commit -m "feat: Apple base styles, Inter font, remove old CSS variables"
```

---

### Task 3: App.vue — Full-Width Tile Layout

**Files:**
- Modify: `src/src/App.vue`

- [ ] **Step 1: Rewrite App.vue template with tile system**

Replace the entire template section:

```vue
<template>
  <div class="min-h-screen bg-canvas">
    <!-- Global Nav -->
    <header class="fixed top-0 w-full z-50 bg-black" style="height: 44px">
      <div class="max-w-[1200px] mx-auto px-6 h-full flex items-center justify-between">
        <div class="flex items-center gap-2">
          <h1 class="text-nav-link font-semibold text-white tracking-tight">
            TRENDING<span class="text-primary">.AGENT</span>
          </h1>
        </div>

        <div class="flex items-center gap-4">
          <span class="text-nav-link font-mono px-2 py-0.5 rounded-badge"
            :class="dataSource === 'api'
              ? 'bg-data-green/10 text-data-green'
              : 'bg-primary/10 text-primary'">
            {{ dataSource.toUpperCase() }}
          </span>
          <span v-if="reportStore.report" class="text-nav-link font-mono text-white/60">
            {{ reportStore.report.date }}
          </span>
          <button
            @click="refreshData"
            :disabled="reportStore.loading"
            class="px-4 py-1.5 text-nav-link font-medium rounded-[8px] transition-all active:scale-95"
            :class="reportStore.loading
              ? 'bg-white/10 text-white/30 cursor-not-allowed'
              : 'bg-ink text-white hover:bg-ink/90 cursor-pointer'"
          >
            {{ reportStore.loading ? '刷新中...' : '↻ 刷新' }}
          </button>
        </div>
      </div>
    </header>

    <!-- Main Content -->
    <main class="pt-[44px]">
      <!-- Error -->
      <div v-if="reportStore.error" class="py-4">
        <div class="max-w-[1200px] mx-auto px-6">
          <div class="bg-data-red/5 border border-data-red/20 rounded-card px-5 py-3 flex items-center justify-between">
            <div class="flex items-center gap-3">
              <span class="text-data-red text-sm">&#9888;</span>
              <span class="text-caption text-data-red">{{ reportStore.error }}</span>
            </div>
            <button @click="refreshData" class="text-caption text-data-red underline">重试</button>
          </div>
        </div>
      </div>

      <!-- Loading -->
      <div v-if="reportStore.loading && !reportStore.report" class="flex flex-col items-center justify-center py-32 gap-4">
        <div class="w-3 h-3 rounded-full bg-primary animate-pulse"></div>
        <span class="text-caption font-mono text-ink-muted">加载市场数据...</span>
      </div>

      <!-- Content -->
      <template v-if="reportStore.report">
        <!-- KPI Tile: white canvas -->
        <section class="bg-canvas" style="padding: 80px 0">
          <div class="max-w-[980px] mx-auto px-6">
            <KpiCards
              :overview="reportStore.report.overview"
              :risk-level="reportStore.report.risk_level"
              :decision-summary="reportStore.report.decision_summary"
            />
          </div>
        </section>

        <!-- Tab Tile: parchment -->
        <section class="bg-canvas-parchment sticky z-40" style="top: 44px; padding: 0">
          <div class="max-w-[1200px] mx-auto px-6">
            <TabBar v-model="activeTab" />
          </div>
        </section>

        <!-- Data Tile: dark -->
        <section class="bg-surface-dark" style="padding: 80px 0; min-height: 60vh">
          <div class="max-w-[1200px] mx-auto px-6">
            <component :is="currentTabComponent" />
          </div>
        </section>
      </template>
    </main>

    <!-- K-line Drawer -->
    <KlineDrawer
      v-if="klineSymbol"
      :symbol="klineSymbol"
      :name="klineName"
      @close="klineSymbol = ''"
    />
  </div>
</template>
```

- [ ] **Step 2: Keep script section unchanged**

The `<script setup>` section remains identical — only the template changes.

- [ ] **Step 3: Commit**

```bash
git add src/src/App.vue
git commit -m "feat: full-width tile layout shell with Apple nav"
```

---

### Task 4: TabBar — Apple Tab Navigation

**Files:**
- Modify: `src/src/components/TabBar.vue`

- [ ] **Step 1: Rewrite TabBar.vue**

```vue
<template>
  <div class="flex overflow-x-auto gap-1" style="padding: 12px 0">
    <button
      v-for="tab in tabs"
      :key="tab.id"
      @click="activeTab = tab.id"
      class="flex-shrink-0 px-4 transition-colors duration-200 border-b-2 cursor-pointer"
      style="padding-top: 10px; padding-bottom: 10px"
      :class="activeTab === tab.id
        ? 'text-ink border-primary'
        : 'text-ink-muted border-transparent hover:text-ink/70'"
    >
      <span class="text-caption">{{ tab.label }}</span>
    </button>
  </div>
</template>

<script setup lang="ts">
const activeTab = defineModel<string>({ default: 'limit-up' })

const tabs = [
  { id: 'limit-up', label: '涨停数据' },
  { id: 'plate', label: '板块异动' },
  { id: 'promotion', label: '晋级' },
  { id: 'timeline', label: '时间线' },
  { id: 'sentiment', label: '舆情分析' },
  { id: 'ai', label: 'AI 观点' },
  { id: 'tgb', label: '淘股吧' },
]
</script>
```

- [ ] **Step 2: Commit**

```bash
git add src/src/components/TabBar.vue
git commit -m "feat: Apple-style tab bar with blue underline active state"
```

---

### Task 5: KpiCards — Apple KPI Cards + Risk Bar

**Files:**
- Modify: `src/src/components/KpiCards.vue`

- [ ] **Step 1: Rewrite KpiCards.vue**

```vue
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
```

- [ ] **Step 2: Commit**

```bash
git add src/src/components/KpiCards.vue
git commit -m "feat: Apple-style KPI cards with hairline borders and risk bar"
```

---

### Task 6: TabLimitUp — Dark Tile Data Table + Field Items

**Files:**
- Modify: `src/src/components/tabs/TabLimitUp.vue`

- [ ] **Step 1: Rewrite TabLimitUp.vue**

```vue
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
```

- [ ] **Step 2: Commit**

```bash
git add src/src/components/tabs/TabLimitUp.vue
git commit -m "feat: Apple-style limit-up table in dark tile with field items"
```

---

### Task 7: TabPlate — Dark Tile Plate Events

**Files:**
- Modify: `src/src/components/tabs/TabPlate.vue`

- [ ] **Step 1: Rewrite TabPlate.vue**

```vue
<template>
  <div>
    <div class="flex items-center gap-3 mb-8 text-caption">
      <span class="text-white/60">
        <b class="text-white">{{ plateEvents.length }}</b> 板块异动
      </span>
      <span class="text-white/20">|</span>
      <span class="text-white/60">
        <b class="text-white">{{ stockEvents.length }}</b> 个股异动
      </span>
    </div>

    <!-- Plate Events -->
    <div class="mb-10" v-if="plateEvents.length">
      <h3 class="text-caption-strong text-white/40 mb-4 tracking-wider uppercase">板块事件</h3>
      <div class="space-y-3">
        <div v-for="event in plateEvents" :key="event.id"
          class="bg-surface-dark rounded-card border border-hairline-dark pl-4 py-3 pr-4 flex gap-4 items-start">
          <div class="w-1 self-stretch rounded-full flex-shrink-0"
            :class="event.pcp >= 0 ? 'bg-data-red' : 'bg-data-green'"></div>
          <div class="flex-1 min-w-0">
            <div class="flex items-center gap-2 mb-2 flex-wrap">
              <span class="text-body-strong text-white">{{ event.plate_name }}</span>
              <span class="text-caption-strong font-mono" :class="event.pcp >= 0 ? 'text-data-red' : 'text-data-green'">
                {{ formatPcp(event.pcp) }}
              </span>
            </div>
            <div class="flex gap-1 flex-wrap mt-2" v-if="event.stocks.length">
              <span v-for="stock in event.stocks" :key="stock.symbol"
                class="px-2 py-0.5 rounded-badge text-caption font-mono bg-white/10 text-white/60">
                {{ stock.symbol }} {{ stock.name }}
              </span>
            </div>
          </div>
          <span class="text-caption font-mono flex-shrink-0 text-white/40">{{ formatTime(event.event_timestamp) }}</span>
        </div>
      </div>
    </div>

    <!-- Stock Events -->
    <div v-if="stockEvents.length">
      <h3 class="text-caption-strong text-white/40 mb-4 tracking-wider uppercase">个股事件</h3>
      <div class="bg-surface-dark rounded-card border border-hairline-dark overflow-hidden">
        <table class="data-table">
          <thead>
            <tr>
              <th>时间</th>
              <th>代码</th>
              <th>名称</th>
              <th style="text-align:right">涨跌幅</th>
              <th style="text-align:right">动量</th>
              <th>方向</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="event in stockEvents" :key="event.id">
              <td class="font-mono text-white/40">{{ formatTime(event.event_timestamp) }}</td>
              <td class="font-mono cursor-pointer hover:underline text-primary" @click="openKline(event.symbol, event.name)">{{ event.symbol }}</td>
              <td class="font-medium text-white">{{ event.name }}</td>
              <td class="font-mono text-right" :class="event.pcp >= 0 ? 'text-data-red' : 'text-data-green'">{{ formatPcp(event.pcp) }}</td>
              <td class="font-mono text-right text-white/60">{{ event.mtm?.toFixed(2) || '-' }}</td>
              <td>
                <span class="px-2 py-0.5 rounded-badge text-caption font-mono"
                  :class="event.good_or_bad === 1
                    ? 'bg-data-green/15 text-data-green'
                    : 'bg-data-red/15 text-data-red'">
                  {{ event.good_or_bad === 1 ? '利好' : '利空' }}
                </span>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>

    <div v-if="plateEvents.length === 0 && stockEvents.length === 0" class="text-center py-12 text-caption text-white/40">
      暂无异动数据
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, inject } from 'vue'
import { useReportStore } from '@/stores/report'

const store = useReportStore()
const openKline = inject<(symbol: string, name: string) => void>('openKline', () => {})

interface PlateFlatEvent {
  id: number
  plate_name: string
  pcp: number
  event_timestamp: number
  stocks: { symbol: string; name: string }[]
}

interface StockFlatEvent {
  id: number
  symbol: string
  name: string
  pcp: number
  mtm: number | null
  event_timestamp: number
  good_or_bad: number
}

const plateEvents = computed<PlateFlatEvent[]>(() => {
  const events: PlateFlatEvent[] = []
  store.report?.plate_abnormal.forEach((e: any) => {
    const d = e.plate_abnormal_event_data
    if (!d || !d.plate_name) return
    events.push({
      id: e.id, plate_name: d.plate_name, pcp: d.pcp ?? 0,
      event_timestamp: e.event_timestamp,
      stocks: (d.related_stocks || []).map((s: any) => ({ symbol: s.symbol, name: s.name })),
    })
  })
  return events.sort((a, b) => b.event_timestamp - a.event_timestamp)
})

const stockEvents = computed<StockFlatEvent[]>(() => {
  const events: StockFlatEvent[] = []
  store.report?.plate_abnormal.forEach((e: any) => {
    const d = e.stock_abnormal_event_data
    if (!d || !d.name) return
    events.push({
      id: e.id, symbol: d.symbol || '-', name: d.name, pcp: d.pcp ?? 0,
      mtm: d.mtm ?? null, event_timestamp: e.event_timestamp, good_or_bad: e.good_or_bad ?? 0,
    })
  })
  return events.sort((a, b) => b.event_timestamp - a.event_timestamp)
})

function formatTime(ts: number): string {
  return new Date(ts * 1000).toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' })
}
function formatPcp(pcp: number): string {
  return pcp === 0 ? '0%' : (pcp > 0 ? '+' : '') + (pcp * 100).toFixed(1) + '%'
}
</script>
```

- [ ] **Step 2: Commit**

```bash
git add src/src/components/tabs/TabPlate.vue
git commit -m "feat: Apple-style plate events in dark tile"
```

---

### Task 8: TabPromotion — Ladder + First Board in Dark Tile

**Files:**
- Modify: `src/src/components/tabs/TabPromotion.vue`

- [ ] **Step 1: Rewrite Template with Apple styling**

Replace the template. Keep the script unchanged (logic is already correct, only presentation changes).

Template changes summary: Replace `style` bindings with Tailwind classes, use `bg-surface-dark`/`border-hairline-dark`/`rounded-card`/`p-card-inner` for cards, `text-white`/`text-white/60`/`text-white/40` for text hierarchy on dark surfaces. Ladder rows use `bg-surface-dark` card, progress bars use `bg-white/10` track with `bg-data-green` or `bg-white/20` fill. First board table uses `.data-table` component class.

- [ ] **Step 2: Commit**

```bash
git add src/src/components/tabs/TabPromotion.vue
git commit -m "feat: Apple-style promotion ladder in dark tile"
```

---

### Task 9: TabSentiment — Article Cards on Dark

**Files:**
- Modify: `src/src/components/tabs/TabSentiment.vue`

- [ ] **Step 1: Rewrite Template with Apple styling**

Replace all inline `style` bindings with Tailwind classes. Cards use `bg-surface-dark rounded-card border border-hairline-dark p-card-inner`. Article rows use `bg-white/[0.04]` hover. Stock rankings use `text-primary` for links. Hot article heat badges use `bg-primary/20 text-primary`. Keep script unchanged.

- [ ] **Step 2: Commit**

```bash
git add src/src/components/tabs/TabSentiment.vue
git commit -m "feat: Apple-style sentiment cards in dark tile"
```

---

### Task 10: TabTimeline — Day-Grouped Event Cards

**Files:**
- Modify: `src/src/components/tabs/TabTimeline.vue`

- [ ] **Step 1: Rewrite Template with Apple styling**

Replace all inline `style` bindings with Tailwind classes. Day headers use `text-caption-strong text-primary font-mono`. Event cards use `bg-surface-dark rounded-card border border-hairline-dark`. Event content uses `text-white/60` on dark. Keep script unchanged.

- [ ] **Step 2: Commit**

```bash
git add src/src/components/tabs/TabTimeline.vue
git commit -m "feat: Apple-style timeline cards in dark tile"
```

---

### Task 11: TabAi + TabTgb — Placeholder States

**Files:**
- Modify: `src/src/components/tabs/TabAi.vue`
- Modify: `src/src/components/tabs/TabTgb.vue`

- [ ] **Step 1: Rewrite TabAi.vue**

```vue
<template>
  <div class="text-center py-16">
    <span class="text-caption font-mono text-white/40">AI 角色分析 — 需配置 OPENROUTER_API_KEY</span>
  </div>
</template>
```

- [ ] **Step 2: Rewrite TabTgb.vue**

```vue
<template>
  <div class="text-center py-16">
    <span class="text-caption font-mono text-white/40">淘股吧 — 需配置 TAOGUBA_USER</span>
  </div>
</template>
```

- [ ] **Step 3: Commit**

```bash
git add src/src/components/tabs/TabAi.vue src/src/components/tabs/TabTgb.vue
git commit -m "feat: Apple-style placeholder states for TabAi and TabTgb"
```

---

### Task 12: KlineDrawer — Dark Drawer Restyle

**Files:**
- Modify: `src/src/components/KlineDrawer.vue`

- [ ] **Step 1: Update template styling**

Replace inline `style` bindings to use Tailwind classes and Apple dark palette:
- Drawer background: `bg-surface-dark`
- Header: `bg-surface-dark border-b border-hairline-dark`
- Crosshair bar: `bg-surface-dark-2 border-b border-hairline-dark`
- Period toggle active: `bg-primary text-white` (pill shape, `rounded-button`)
- Period toggle inactive: `bg-transparent text-white/40`
- Metrics cards: `bg-surface-dark rounded-card border border-hairline-dark`

- [ ] **Step 2: Update ECharts theme colors**

Replace hardcoded ECharts colors in `renderChart()`:
- `backgroundColor: '#272729'`
- axisLine/splitLine: `'rgba(255,255,255,0.06)'`
- axisLabel: `'#86868b'`
- candlestick up color: `'#34c759'`, down: `'#ff3b30'`
- volume bars: `'rgba(52,199,89,0.3)'` / `'rgba(255,59,48,0.3)'`
- mark line colors: `'rgba(255,255,255,0.20)'` (preclose), `'#ff3b30'` (limit down), `'#34c759'` (limit up)

- [ ] **Step 3: Commit**

```bash
git add src/src/components/KlineDrawer.vue
git commit -m "feat: Apple dark palette for K-line drawer and ECharts"
```

---

### Task 13: Delete HelloWorld.vue

**Files:**
- Delete: `src/src/components/HelloWorld.vue`

- [ ] **Step 1: Remove unused scaffold**

```bash
rm src/src/components/HelloWorld.vue
```

- [ ] **Step 2: Commit**

```bash
git add src/src/components/HelloWorld.vue
git commit -m "chore: remove unused HelloWorld scaffold"
```

---

### Task 14: Final Verification

- [ ] **Step 1: Verify dev server starts cleanly**

```bash
cd src && npm run dev
```

Check that the page renders without console errors.

- [ ] **Step 2: Verify no old CSS variable references remain**

```bash
grep -rn "var(--" src/src/ --include="*.vue" --include="*.css"
```

Expected: no matches (all old `var(--bg-primary)`, `var(--text-muted)`, etc. should be gone).

- [ ] **Step 3: Commit any remaining changes**

```bash
git status
# If clean, done. If not:
git add -A && git commit -m "chore: final polish for Apple redesign"
```
