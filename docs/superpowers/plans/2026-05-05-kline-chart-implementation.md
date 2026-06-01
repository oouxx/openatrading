# K-Line Chart Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add K-line candlestick charts with real-time quotes — click any stock to open a slide-out drawer with interactive chart + headline data + valuation metrics.

**Architecture:** Core crate fetches K-line and real-time data from XuanguBao ddc-wscn API. Web crate exposes `/api/kline/:symbol` and `/api/realtime/:symbol`. Frontend KlineDrawer.vue uses lightweight-charts for rendering. App.vue provides provide/inject for openKline trigger.

**Tech Stack:** Rust (axum/reqwest), Vue 3 + TypeScript, lightweight-charts v4

---

### Task 1: Add K-line + real-time API to core crate

**Files:**
- Modify: `crates/core/src/api/xuangubao.rs`

- [ ] **Step 1: Add K-line and real-time data structures and fetch methods**

Append to `crates/core/src/api/xuangubao.rs`:

```rust
// ─── K线数据 ─────────────────────────────────────────────

const DDC_BASE_URL: &str = "https://api-ddc-wscn.xuangubao.com.cn/market";

#[derive(Debug, Clone, Copy)]
pub enum KlinePeriod {
    Minute,
    Day,
    Week,
    Month,
}

impl KlinePeriod {
    pub fn as_seconds(&self) -> u32 {
        match self {
            KlinePeriod::Minute => 60,
            KlinePeriod::Day => 86400,
            KlinePeriod::Week => 604800,
            KlinePeriod::Month => 2592000,
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            KlinePeriod::Minute => "minute",
            KlinePeriod::Day => "day",
            KlinePeriod::Week => "week",
            KlinePeriod::Month => "month",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KlineCandle {
    pub time: i64,
    pub open: f64,
    pub close: f64,
    pub high: f64,
    pub low: f64,
    pub volume: f64,
    pub change_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KlineData {
    pub symbol: String,
    pub period: String,
    pub candles: Vec<KlineCandle>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealtimeQuote {
    pub symbol: String,
    pub name: String,
    pub last_px: f64,
    pub px_change: f64,
    pub px_change_rate: f64,
    pub open_px: f64,
    pub high_px: f64,
    pub low_px: f64,
    pub preclose_px: f64,
    pub amplitude: f64,
    pub turnover_ratio: f64,
    pub turnover_volume: f64,
    pub turnover_value: f64,
    pub market_value: f64,
    pub circulation_value: f64,
    pub pe_rate: f64,
    pub dyn_pe: f64,
    pub dyn_pb_rate: f64,
    pub volume_ratio: f64,
    pub circulation_shares: f64,
    pub total_shares: f64,
    pub trade_status: String,
    pub update_time: i64,
}

impl XuanguBaoClient {
    // ... existing methods ...

    pub async fn fetch_kline(
        &self,
        symbol: &str,
        period: KlinePeriod,
        count: u32,
    ) -> Result<KlineData> {
        let url = format!(
            "{DDC_BASE_URL}/kline?prod_code={symbol}&period_type={period_sec}&tick_count={count}&adjust_price_type=forward&fields=tick_at,open_px,close_px,high_px,low_px,turnover_volume,px_change_rate",
            period_sec = period.as_seconds()
        );

        let resp: serde_json::Value = self.client.get(&url).send().await?.json().await?;
        anyhow::ensure!(resp["code"] == 20000, "Kline API error");

        let data = &resp["data"]["candle"][symbol]["lines"];
        let lines = data.as_array().context("missing lines array")?;

        let candles: Vec<KlineCandle> = lines
            .iter()
            .filter_map(|line| {
                let arr = line.as_array()?;
                if arr.len() < 7 { return None; }
                Some(KlineCandle {
                    time: arr[0].as_i64()?,
                    open: arr[1].as_f64()?,
                    close: arr[2].as_f64()?,
                    high: arr[3].as_f64()?,
                    low: arr[4].as_f64()?,
                    volume: arr[5].as_f64()?,
                    change_rate: arr[6].as_f64()?,
                })
            })
            .collect();

        Ok(KlineData {
            symbol: symbol.to_string(),
            period: period.as_str().to_string(),
            candles,
        })
    }

    pub async fn fetch_realtime(&self, symbol: &str) -> Result<RealtimeQuote> {
        let url = format!(
            "{DDC_BASE_URL}/real?fields=prod_name,trade_status,update_time,last_px,px_change,px_change_rate,preclose_px,open_px,high_px,low_px,amplitude,turnover_ratio,pe_rate,dyn_pe,dyn_pb_rate,market_value,circulation_value,turnover_volume,turnover_value,volume_ratio,circulation_shares,total_shares&prod_code={symbol}"
        );

        let resp: serde_json::Value = self.client.get(&url).send().await?.json().await?;
        anyhow::ensure!(resp["code"] == 20000, "Realtime API error");

        let snap = &resp["data"]["snapshot"][symbol];
        let arr = snap.as_array().context("missing snapshot array")?;

        let get = |i: usize, default: f64| -> f64 {
            arr.get(i).and_then(|v| v.as_f64()).unwrap_or(default)
        };
        let get_str = |i: usize, default: &str| -> String {
            arr.get(i).and_then(|v| v.as_str()).unwrap_or(default).to_string()
        };

        Ok(RealtimeQuote {
            symbol: symbol.to_string(),
            name: get_str(0, ""),
            trade_status: get_str(1, ""),
            update_time: arr.get(2).and_then(|v| v.as_i64()).unwrap_or(0),
            last_px: get(3, 0.0),
            px_change: get(4, 0.0),
            px_change_rate: get(5, 0.0),
            preclose_px: get(6, 0.0),
            open_px: get(7, 0.0),
            high_px: get(8, 0.0),
            low_px: get(9, 0.0),
            amplitude: get(10, 0.0),
            turnover_ratio: get(11, 0.0),
            pe_rate: get(12, 0.0),
            dyn_pe: get(13, 0.0),
            dyn_pb_rate: get(14, 0.0),
            market_value: get(15, 0.0),
            circulation_value: get(16, 0.0),
            turnover_volume: get(17, 0.0),
            turnover_value: get(18, 0.0),
            volume_ratio: get(21, 0.0),
            circulation_shares: get(22, 0.0),
            total_shares: get(23, 0.0),
        })
    }
}
```

- [ ] **Step 2: Verify compilation**

```bash
cargo build -p openatrading-core
```

Expected: compiles without errors.

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/api/xuangubao.rs
git commit -m "feat(core): add fetch_kline and fetch_realtime to XuanguBaoClient"
```

---

### Task 2: Add K-line + real-time endpoints to web crate

**Files:**
- Modify: `crates/web/src/main.rs`

- [ ] **Step 1: Add `/api/kline/:symbol` and `/api/realtime/:symbol` routes**

In `crates/web/src/main.rs`, add imports at top:

```rust
use axum::extract::Path;
use openatrading_core::api::xuangubao::{KlinePeriod, XuanguBaoClient};
```

In `start_web_server()`, add routes before `.layer(cors)`:

```rust
.route("/api/kline/{symbol}", get(get_kline))
.route("/api/realtime/{symbol}", get(get_realtime))
```

Add query struct and handler functions:

```rust
#[derive(serde::Deserialize)]
struct KlineQuery {
    #[serde(default = "default_period")]
    period: String,
    #[serde(default = "default_count")]
    count: u32,
}
fn default_period() -> String { "day".into() }
fn default_count() -> u32 { 256 }

async fn get_kline(
    Path(symbol): Path<String>,
    Query(q): Query<KlineQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    let period = match q.period.as_str() {
        "week" => KlinePeriod::Week,
        "month" => KlinePeriod::Month,
        "minute" => KlinePeriod::Minute,
        _ => KlinePeriod::Day,
    };
    let client = XuanguBaoClient::new();
    let data = client.fetch_kline(&symbol, period, q.count).await
        .map_err(|e| { eprintln!("kline error: {}", e); StatusCode::SERVICE_UNAVAILABLE })?;
    let json = serde_json::to_value(&data).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json))
}

async fn get_realtime(
    Path(symbol): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let client = XuanguBaoClient::new();
    let data = client.fetch_realtime(&symbol).await
        .map_err(|e| { eprintln!("realtime error: {}", e); StatusCode::SERVICE_UNAVAILABLE })?;
    let json = serde_json::to_value(&data).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json))
}
```

- [ ] **Step 2: Verify compilation**

```bash
cargo build -p openatrading-web
```

Expected: compiles without errors.

- [ ] **Step 3: Commit**

```bash
git add crates/web/src/main.rs
git commit -m "feat(web): add /api/kline/:symbol and /api/realtime/:symbol endpoints"
```

---

### Task 3: Install lightweight-charts and proxy config

**Files:**
- Modify: `src/package.json`
- Modify: `src/vite.config.ts`

- [ ] **Step 1: Install lightweight-charts**

```bash
cd src && npm install lightweight-charts
```

- [ ] **Step 2: Add K-line API proxy to Vite config**

In `src/vite.config.ts`, in `server.proxy` add:

```ts
'/kline': {
  target: 'https://api-ddc-wscn.xuangubao.com.cn',
  changeOrigin: true,
  rewrite: (path: string) => path.replace(/^\/kline/, '/market/kline'),
},
'/realtime': {
  target: 'https://api-ddc-wscn.xuangubao.com.cn',
  changeOrigin: true,
  rewrite: (path: string) => path.replace(/^\/realtime/, '/market/real'),
},
```

- [ ] **Step 3: Commit**

```bash
git add src/package.json src/package-lock.json src/vite.config.ts
git commit -m "feat: add lightweight-charts and K-line/realtime API proxy"
```

---

### Task 4: Create KlineDrawer.vue

**Files:**
- Create: `src/src/components/KlineDrawer.vue`

- [ ] **Step 1: Create KlineDrawer.vue**

```vue
<template>
  <Teleport to="body">
    <!-- Overlay -->
    <div @click="$emit('close')" class="fixed inset-0 z-40" style="background: rgba(0,0,0,0.4)"></div>

    <!-- Drawer -->
    <div class="fixed top-0 right-0 h-full z-50 overflow-y-auto" style="width: 520px; background: var(--bg-secondary); border-left: 1px solid var(--border-subtle)">
      <!-- Header -->
      <div class="sticky top-0 z-10 p-4 flex items-center justify-between" style="background: var(--bg-secondary); border-bottom: 1px solid var(--border-subtle)">
        <div>
          <div class="flex items-center gap-2">
            <span class="text-sm font-bold" style="color: var(--text-primary)">{{ name }}</span>
            <span class="text-xs font-mono" style="color: var(--text-muted)">{{ symbol }}</span>
          </div>
          <div v-if="quote" class="flex items-center gap-3 mt-1">
            <span class="text-lg font-bold font-mono" :style="{ color: quote.px_change_rate >= 0 ? 'var(--red)' : 'var(--green)' }">{{ formatPrice(quote.last_px) }}</span>
            <span class="text-sm font-mono" :style="{ color: quote.px_change_rate >= 0 ? 'var(--red)' : 'var(--green)' }">{{ quote.px_change_rate >= 0 ? '+' : '' }}{{ (quote.px_change_rate * 100).toFixed(2) }}%</span>
            <span class="text-xs" style="color: var(--text-muted)">成交 {{ formatVolume(quote.turnover_value) }}</span>
          </div>
        </div>
        <button @click="$emit('close')" class="text-lg" style="color: var(--text-muted)">&times;</button>
      </div>

      <!-- Period Toggle -->
      <div class="flex gap-1 px-4 py-2" style="border-bottom: 1px solid var(--border-subtle)">
        <button v-for="p in periods" :key="p.value" @click="period = p.value"
          class="px-3 py-1 text-xs rounded font-mono transition"
          :style="period === p.value ? { background: 'var(--blue)', color: '#fff' } : { background: 'transparent', color: 'var(--text-muted)' }">
          {{ p.label }}
        </button>
      </div>

      <!-- Chart -->
      <div ref="chartContainer" class="w-full" style="height: 360px"></div>

      <!-- Metrics -->
      <div v-if="quote" class="p-4 grid grid-cols-3 gap-3 text-xs">
        <div v-for="m in metrics" :key="m.label" class="glow-card p-3 text-center">
          <p style="color: var(--text-muted)">{{ m.label }}</p>
          <p class="font-mono font-bold mt-1" style="color: var(--text-primary)">{{ m.value }}</p>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { ref, watch, onMounted, onUnmounted, computed } from 'vue'
import { createChart, ColorType } from 'lightweight-charts'

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

let chart: any = null
let candleSeries: any = null

function formatPrice(v: number) { return v?.toFixed(2) || '-' }
function formatVolume(v: number) {
  if (!v) return '-'
  if (v > 1e8) return (v / 1e8).toFixed(2) + '亿'
  if (v > 1e4) return (v / 1e4).toFixed(2) + '万'
  return v.toFixed(0)
}

const metrics = computed(() => {
  if (!quote.value) return []
  return [
    { label: '今开', value: formatPrice(quote.value.open_px) },
    { label: '昨收', value: formatPrice(quote.value.preclose_px) },
    { label: '最高', value: formatPrice(quote.value.high_px) },
    { label: '最低', value: formatPrice(quote.value.low_px) },
    { label: '振幅', value: (quote.value.amplitude * 100).toFixed(2) + '%' },
    { label: '换手率', value: (quote.value.turnover_ratio * 100).toFixed(2) + '%' },
    { label: '动态PE', value: quote.value.dyn_pe ? quote.value.dyn_pe.toFixed(1) : '-' },
    { label: '市净率', value: quote.value.dyn_pb_rate ? quote.value.dyn_pb_rate.toFixed(2) : '-' },
    { label: '总市值', value: formatVolume(quote.value.market_value) },
    { label: '流通市值', value: formatVolume(quote.value.circulation_value) },
    { label: '量比', value: quote.value.volume_ratio?.toFixed(2) || '-' },
    { label: '成交量', value: formatVolume(quote.value.turnover_volume) + '股' },
  ]
})

async function loadChart() {
  if (!chartContainer.value) return
  // 销毁旧图
  if (chart) { chart.remove(); chart = null }

  chart = createChart(chartContainer.value, {
    layout: {
      background: { type: ColorType.Solid, color: '#111827' },
      textColor: '#8899aa',
    },
    grid: { vertLines: { color: '#1e2a3a' }, horzLines: { color: '#1e2a3a' } },
    width: chartContainer.value.clientWidth,
    height: 360,
    crosshair: { mode: 0 },
    timeScale: { borderColor: '#1e2a3a', timeVisible: true },
    rightPriceScale: { borderColor: '#1e2a3a' },
  })

  candleSeries = chart.addCandlestickSeries({
    upColor: '#00e676',
    downColor: '#ff1744',
    borderUpColor: '#00e676',
    borderDownColor: '#ff1744',
    wickUpColor: '#00e676',
    wickDownColor: '#ff1744',
  })

  await fetchKline()
}

async function fetchRealtime() {
  try {
    const resp = await fetch(`http://localhost:8080/api/realtime/${props.symbol}`)
    if (resp.ok) quote.value = await resp.json()
  } catch (e) { /* ignore */ }
}

async function fetchKline() {
  try {
    const resp = await fetch(`http://localhost:8080/api/kline/${props.symbol}?period=${period.value}&count=256`)
    if (!resp.ok) return
    const data = await resp.json()
    candleSeries?.setData(data.candles.map((c: any) => ({
      time: c.time,
      open: c.open,
      close: c.close,
      high: c.high,
      low: c.low,
    })))
  } catch (e) { /* ignore */ }
}

watch(period, fetchKline)
onMounted(async () => {
  await fetchRealtime()
  loadChart()
})
onUnmounted(() => { chart?.remove() })
</script>
```

- [ ] **Step 2: Commit**

```bash
git add src/src/components/KlineDrawer.vue
git commit -m "feat: KlineDrawer slide-out panel with lightweight-charts + real-time quote"
```

---

### Task 5: Wire up click → open drawer in App.vue and tabs

**Files:**
- Modify: `src/src/App.vue`
- Modify: `src/src/components/tabs/TabLimitUp.vue`
- Modify: `src/src/components/tabs/TabPlate.vue`

- [ ] **Step 1: Update App.vue — add KlineDrawer with provide/inject**

At top of `<script setup>` in `src/src/App.vue`, add:

```ts
import { ref, computed, provide } from 'vue'
import KlineDrawer from './components/KlineDrawer.vue'

const klineSymbol = ref('')
const klineName = ref('')

function openKline(symbol: string, name: string) {
  klineSymbol.value = symbol
  klineName.value = name
}

provide('openKline', openKline)
```

In template, add at end of `<main>`:

```vue
<KlineDrawer
  v-if="klineSymbol"
  :symbol="klineSymbol"
  :name="klineName"
  @close="klineSymbol = ''"
/>
```

- [ ] **Step 2: Update TabLimitUp.vue — make stock name clickable**

Change `<td style="font-weight: 500; color: var(--text-primary)">{{ stock.stock_chi_name }}</td>` to:

```vue
<td style="font-weight: 500">
  <span @click="onStockClick(stock.symbol, stock.stock_chi_name)"
    class="cursor-pointer hover:underline"
    style="color: var(--blue)">{{ stock.stock_chi_name }}</span>
</td>
```

In `<script>`, add:

```ts
import { inject } from 'vue'
const openKline = inject<(symbol: string, name: string) => void>('openKline')
function onStockClick(symbol: string, name: string) {
  openKline?.(symbol, name)
}
```

- [ ] **Step 3: Update TabPlate.vue — same click treatment for stock names**

For plate events stocks: change to clickable span with `onStockClick`.
For stock events table: change name column to clickable.

- [ ] **Step 4: Verify build**

```bash
cd src && npx vite build
```

Expected: builds without errors.

- [ ] **Step 5: Commit**

```bash
git add src/src/App.vue src/src/components/tabs/TabLimitUp.vue src/src/components/tabs/TabPlate.vue
git commit -m "feat: wire up stock click → KlineDrawer in all tabs"
```

---

### Task 6: End-to-end verification

- [ ] **Step 1: Start web crate**

```bash
cargo run --bin openatrading-web
```

- [ ] **Step 2: Test API endpoints**

```bash
curl -s "http://localhost:8080/api/kline/000609.SZ?period=day&count=5" | python3 -c "import json,sys; d=json.load(sys.stdin); print('candles:', len(d.get('candles',[])))"
curl -s "http://localhost:8080/api/realtime/688256.SS" | python3 -c "import json,sys; d=json.load(sys.stdin); print('name:', d.get('name'))"
```

Expected: Kline returns 5 candles, realtime returns "寒武纪".

- [ ] **Step 3: Build frontend**

```bash
cd src && npx vite build
```

Expected: builds successfully.

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "chore: finalize K-line chart feature"
```
