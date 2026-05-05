# Phase A: WASM 数据管道实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让 Vue 前端通过 WASM 调用选股宝公开 API 获取真实数据，替换所有 mock 数据

**Architecture:** Rust crates/core 已有完整的 API 客户端，crates/wasm 是空壳。给 WASM crate 装上 `run_report()` 函数调用选股宝 3 个端点并行拉数据，序列化为 JSON 返回。Vue 端删掉 mock，改调 WASM，修正 6 处字段名不匹配。

**Tech Stack:** Rust wasm32-unknown-unknown, wasm-bindgen, serde_json, Vue 3 + Pinia + TypeScript, Vite + vite-plugin-wasm

---

### Task 1: reqwest 适配 WASM target

**Files:**
- Modify: `Cargo.toml` (workspace root)

- [ ] **Step 1: 给 workspace reqwest 加 rustls-tls feature**

```toml
# Cargo.toml line 11
reqwest = { version = "0.12", features = ["json", "cookies", "stream", "rustls-tls"] }
```

没有 `rustls-tls`，`reqwest` 在 WASM 下没法发起 HTTPS 请求。`native-tls` 依赖 OpenSSL，WASM 没有。

- [ ] **Step 2: 验证 core crate 仍能编译**

Run: `cargo build -p trending-agent-core`
Expected: 编译成功

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "build: add rustls-tls to reqwest for WASM compatibility"
```

---

### Task 2: WASM crate 依赖配置

**Files:**
- Modify: `crates/wasm/Cargo.toml`

- [ ] **Step 1: 更新 Cargo.toml**

```toml
[package]
name = "trending-agent-wasm"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
trending-agent-core = { path = "../core" }
wasm-bindgen = "0.2"
wasm-bindgen-futures = "0.4"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
js-sys = "0.3"
futures = { workspace = true }
```

加 `serde` + `serde_json` 做 JSON 序列化，`js-sys` 提供 `Error` 类型用于错误传递。

- [ ] **Step 2: Commit**

```bash
git add crates/wasm/Cargo.toml
git commit -m "build(wasm): add serde_json, js-sys dependencies"
```

---

### Task 3: 实现 WASM `run_report()` 函数

**Files:**
- Modify: `crates/wasm/src/lib.rs`

- [ ] **Step 1: 重写 lib.rs**

```rust
use serde::Serialize;
use std::sync::OnceLock;
use wasm_bindgen::prelude::*;

static OPENROUTER_KEY: OnceLock<String> = OnceLock::new();

use trending_agent_core::api::xuangubao::XuanguBaoClient;
use trending_agent_core::report::report::Report;

#[derive(Serialize)]
struct WasmReport {
    date: String,
    overview: trending_agent_core::api::xuangubao::MarketOverview,
    limit_up: Vec<trending_agent_core::api::xuangubao::Stock>,
    plate_abnormal: Vec<trending_agent_core::api::xuangubao::PlateAbnormalEvent>,
    risk_level: String,
    opportunity_signals: Vec<String>,
    decision_summary: String,
    // Phase A: 这些字段用空值，Phase B cron 静态文件填充
    field_items: Vec<trending_agent_core::api::jiuyangongshe::FieldPlate>,
    timeline: Vec<trending_agent_core::api::jiuyangongshe::TimelineDay>,
    analyses: Vec<serde_json::Value>,
    sentiment: Option<serde_json::Value>,
    taoguba_users: Vec<trending_agent_core::api::taoguba::TgbUserSummary>,
    xuangubao_extra: trending_agent_core::api::xuangubao::XuanGubaoExtra,
}

#[wasm_bindgen]
pub fn set_openrouter_key(key: &str) {
    let _ = OPENROUTER_KEY.set(key.to_string());
}

#[wasm_bindgen]
pub async fn run_report() -> Result<JsValue, JsValue> {
    let client = XuanguBaoClient::new();

    // 并行拉取选股宝 3 个端点（futures::join! 无需 tokio runtime）
    let (overview, limit_up, plates) = futures::join!(
        client.fetch_market_overview(),
        client.fetch_limit_up(),
        client.fetch_plate_abnormal(50),
    );

    let overview = overview.map_err(|e| JsValue::from_str(&e.to_string()))?;
    let limit_up = limit_up.map_err(|e| JsValue::from_str(&e.to_string()))?;
    let plates = plates.map_err(|e| JsValue::from_str(&e.to_string()))?;

    // 风险等级计算（复用 Report::new 的逻辑）
    let ratio = overview.rise_count as f64 / overview.fall_count.max(1) as f64;
    let risk_level = if ratio < 0.8 && overview.bomb_rate > 30.0 {
        "高"
    } else if ratio < 1.2 || overview.bomb_rate > 20.0 {
        "中"
    } else {
        "低"
    };

    let mut opportunity_signals = Vec::new();
    if let Some(head) = limit_up.iter().max_by_key(|s| s.limit_up_days) {
        opportunity_signals.push(format!("龙头：{}（{}板）", head.stock_chi_name, head.limit_up_days));
    }

    let decision_summary = format!(
        "风险等级：{}；上涨{}家/下跌{}家",
        risk_level, overview.rise_count, overview.fall_count
    );

    let now = js_sys::Date::new_0();
    let date_str = format!(
        "{}-{:02}-{:02} {:02}:{:02}",
        now.get_full_year(),
        now.get_month() + 1,
        now.get_date(),
        now.get_hours(),
        now.get_minutes()
    );

    let report = WasmReport {
        date: date_str,
        overview,
        limit_up,
        plate_abnormal: plates,
        risk_level: risk_level.to_string(),
        opportunity_signals,
        decision_summary,
        field_items: vec![],
        timeline: vec![],
        analyses: vec![],
        sentiment: None,
        taoguba_users: vec![],
        xuangubao_extra: trending_agent_core::api::xuangubao::XuanGubaoExtra {
            limit_down: vec![],
            plate_abnormal: vec![],
        },
    };

    let json = serde_json::to_string(&report)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    Ok(JsValue::from_str(&json))
}
```

`WasmReport` 结构体加上 `#[derive(Serialize)]`，`serde_json::to_string` 直接序列化为 JSON 字符串传给 JS。

- [ ] **Step 2: 检查 core 模块的 pub 导出是否够用**

`crates/core/src/lib.rs` 已经 `pub mod api; pub mod report; pub mod agents;`，选股宝类型需要能被 WASM crate 引用。

Run: `grep -r "pub struct\|pub mod" crates/core/src/api/xuangubao.rs | head -10`
确认 `XuanguBaoClient`、`MarketOverview`、`Stock`、`PlateAbnormalEvent`、`XuanGubaoExtra` 都是 `pub`。

- [ ] **Step 3: Commit**

```bash
git add crates/wasm/src/lib.rs crates/wasm/Cargo.toml
git commit -m "feat(wasm): implement run_report() with xuangubao data fetching"
```

---

### Task 4: WASM 编译验证

**Files:** (无变更，仅验证)

- [ ] **Step 1: 安装 wasm32 target**

```bash
rustup target add wasm32-unknown-unknown
```

- [ ] **Step 2: 安装 wasm-pack**

```bash
curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
# 或者: brew install wasm-pack
```

- [ ] **Step 3: 编译 WASM**

```bash
cd crates/wasm && wasm-pack build --target web --dev
```

Expected: `[INFO]: Done! Your wasm pkg is ready to publish`

- [ ] **Step 4: 检查输出文件**

```bash
ls pkg/
```

Expected: 包含 `trending_agent_wasm_bg.wasm`, `trending_agent_wasm.js`, `trending_agent_wasm.d.ts`

- [ ] **Step 5: Commit**（如果有 Cargo.lock 变更）

```bash
git add crates/wasm/ Cargo.lock
git commit -m "chore(wasm): verify wasm-pack build succeeds"
```

---

### Task 5: Vite WASM 插件配置

**Files:**
- Modify: `src/vite.config.ts`
- Modify: `src/package.json`

- [ ] **Step 1: 安装 Vite WASM 插件**

```bash
cd src && npm install --save-dev vite-plugin-wasm vite-plugin-top-level-await
```

- [ ] **Step 2: 更新 vite.config.ts**

```typescript
import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import wasm from 'vite-plugin-wasm'
import topLevelAwait from 'vite-plugin-top-level-await'

export default defineConfig({
  plugins: [
    vue(),
    wasm(),
    topLevelAwait(),
  ],
  define: {
    __OPENROUTER_KEY__: JSON.stringify(process.env.OPENROUTER_API_KEY || ''),
  },
})
```

从 CommonJS `require` 改为 ESM `import`，同时加 `define` 注入 API key。

- [ ] **Step 3: 更新 package.json scripts**

```json
{
  "scripts": {
    "wasm:build": "cd ../crates/wasm && wasm-pack build --target web --out-dir ../../src/src/wasm",
    "dev": "npm run wasm:build && vite",
    "build": "npm run wasm:build && vue-tsc -b && vite build",
    "preview": "vite preview"
  }
}
```

- [ ] **Step 4: 运行 wasm:build 验证**

```bash
cd src && npm run wasm:build
```

Expected: WASM 输出到 `src/src/wasm/`

- [ ] **Step 5: Commit**

```bash
git add src/vite.config.ts src/package.json src/package-lock.json
git commit -m "build(vite): add WASM plugin support and build scripts"
```

---

### Task 6: 更新 WASM 加载器 `api/web.ts`

**Files:**
- Modify: `src/api/web.ts`

- [ ] **Step 1: 重写 web.ts**

```typescript
let wasmModule: any = null

async function loadWasm() {
  if (!wasmModule) {
    wasmModule = await import('../wasm/trending_agent_wasm')
    // 注入 OpenRouter API key（如果配置了）
    if (typeof __OPENROUTER_KEY__ !== 'undefined' && __OPENROUTER_KEY__) {
      wasmModule.set_openrouter_key(__OPENROUTER_KEY__)
    }
  }
  return wasmModule
}

declare const __OPENROUTER_KEY__: string

export async function runReport(): Promise<string> {
  const wasm = await loadWasm()
  const result = await wasm.run_report()
  return result as string
}
```

- [ ] **Step 2: 添加 `__OPENROUTER_KEY__` 类型声明**

在 `src/src/` 下可能没有 `env.d.ts`，检查并确保类型不报错：

```bash
ls src/src/env.d.ts src/src/vite-env.d.ts 2>/dev/null
```

如果不存在，创建 `src/src/env.d.ts`：

```typescript
/// <reference types="vite/client" />
declare const __OPENROUTER_KEY__: string
```

- [ ] **Step 3: Commit**

```bash
git add src/api/web.ts src/src/env.d.ts
git commit -m "feat(api): wire up WASM loader with API key injection"
```

---

### Task 7: 重写 Pinia Store——删除 mock，调 WASM

**Files:**
- Modify: `src/stores/report.ts`

- [ ] **Step 1: 重写 report.ts**

```typescript
import { defineStore } from 'pinia'
import { ref, type Ref } from 'vue'
import { runReport } from '../api/web'

// Types matching Rust serde_json output (snake_case)
interface PlateReason {
  plate_id: number
  plate_name: string
  plate_reason: string | null
}

interface SurgeReason {
  symbol: string
  stock_reason: string
  related_plates: PlateReason[]
}

interface Stock {
  symbol: string
  stock_chi_name: string
  price: number
  change_percent: number
  limit_up_days: number
  break_limit_up_times: number
  turnover_ratio: number
  total_capital: number
  surge_reason: SurgeReason | null
}

interface MarketOverview {
  rise_count: number
  fall_count: number
  limit_up_count: number
  limit_down_count: number
  bomb_rate: number
}

interface RelatedStock {
  symbol: string
  name: string
  mtm: number
  pcp: number
}

interface PlateAbnormalEventData {
  plate_id: number | null
  plate_name: string | null
  pcp: number | null
  related_stocks: RelatedStock[]
}

interface PlateAbnormalEvent {
  id: number
  target: string
  event_type: number
  event_timestamp: number
  plate_abnormal_event_data: PlateAbnormalEventData | null
  good_or_bad: number
}

interface CommunityArticle {
  article_id: string
  title: string
  create_time: string
  content: string | null
  like_count: number
  comment_count: number
  forward_count: number
  collect_count: number
  user: { user_id: string; nickname: string; avatar: string | null }
  stock_list: { code: string; name: string }[]
}

interface SentimentResult {
  hot_stocks: [string, string, number][]
  hot_articles: [string, number][]
  trend: string
  summary: string
  news_summary: string
  articles: CommunityArticle[]
}

interface RoleAnalysis {
  role: string
  content: string
  elapsed_ms: number
}

interface TgbUserSummary {
  user_id: string
  nickname: string
  post_count: number
  top_post_title: string
  last_update: string
}

interface XuanGubaoExtra {
  limit_down: Stock[]
  plate_abnormal: PlateAbnormalEvent[]
}

interface FieldPlate {
  name: string
  count: number
  reason: string
  list: { code: string; name: string }[]
}

interface TimelineDay {
  date: string
  list: { title: string; content: string }[]
}

interface Report {
  date: string
  overview: MarketOverview
  limit_up: Stock[]
  plate_abnormal: PlateAbnormalEvent[]
  field_items: FieldPlate[]
  timeline: TimelineDay[]
  analyses: RoleAnalysis[]
  sentiment: SentimentResult | null
  risk_level: string
  opportunity_signals: string[]
  decision_summary: string
  taoguba_users: TgbUserSummary[]
  xuangubao_extra: XuanGubaoExtra
}

export const useReportStore = defineStore('report', () => {
  const report: Ref<Report | null> = ref(null)
  const loading = ref(false)
  const error: Ref<string | null> = ref(null)

  async function fetchReport() {
    loading.value = true
    error.value = null
    try {
      const jsonStr = await runReport()
      report.value = JSON.parse(jsonStr) as Report
    } catch (e) {
      console.error('Failed to fetch report:', e)
      error.value = String(e)
    } finally {
      loading.value = false
    }
  }

  return { report, loading, error, fetchReport }
})
```

核心变化：
- 删除 `mockReport` 常量
- 删除 `setTimeout` 模拟
- `fetchReport` 改调 `runReport()` 从 `../api/web`
- TypeScript 类型完整匹配 Rust `serde_json` 输出的 `snake_case` 字段
- 新增 `error` 状态供 UI 显示错误

- [ ] **Step 2: 检查 App.vue 的 store 引用路径**

当前 `App.vue:48` 用 `./stores/report`，需要确认路径正确（`../../stores/report` 或 `./stores/report`）。

Run: `grep "from.*stores" src/src/App.vue`
如果不对，修正 import 路径。

- [ ] **Step 3: Commit**

```bash
git add src/stores/report.ts
git commit -m "feat(store): replace mock data with WASM runReport() call"
```

---

### Task 8: 修正 KpiCards.vue 字段名

**Files:**
- Modify: `src/src/components/KpiCards.vue`

- [ ] **Step 1: 更新 template——修正指标字段**

KPI 卡片当前用 `rise_count` / `fall_count` 表示涨跌停家数，但 Rust `MarketOverview` 中：
- `rise_count` / `fall_count` = 上涨家数 / 下跌家数
- `limit_up_count` / `limit_down_count` = 涨停家数 / 跌停家数

修正 template：

```vue
<template>
  <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4 mb-6">
    <div class="bg-white rounded-xl shadow p-4 border-l-4 border-green-500">
      <div class="flex justify-between items-start">
        <div>
          <p class="text-gray-500 text-sm">📈 上涨家数</p>
          <p class="text-3xl font-bold mt-2">{{ overview.rise_count }}</p>
        </div>
        <span class="text-green-500 text-sm">今日</span>
      </div>
    </div>

    <div class="bg-white rounded-xl shadow p-4 border-l-4 border-red-500">
      <div class="flex justify-between items-start">
        <div>
          <p class="text-gray-500 text-sm">📉 下跌家数</p>
          <p class="text-3xl font-bold mt-2">{{ overview.fall_count }}</p>
        </div>
        <span class="text-red-500 text-sm">今日</span>
      </div>
    </div>

    <div class="bg-white rounded-xl shadow p-4 border-l-4 border-blue-500">
      <div class="flex justify-between items-start">
        <div>
          <p class="text-gray-500 text-sm">📊 涨跌比</p>
          <p class="text-3xl font-bold mt-2">{{ ratio }}</p>
        </div>
        <span class="text-gray-500 text-sm">{{ overview.limit_up_count }}涨停 / {{ overview.limit_down_count }}跌停</span>
      </div>
    </div>

    <div class="bg-white rounded-xl shadow p-4 border-l-4" :class="sentimentBorder">
      <div class="flex justify-between items-start">
        <div>
          <p class="text-gray-500 text-sm">⚠️ 炸板率</p>
          <p class="text-2xl font-bold mt-2" :class="bombRateColor">{{ (overview.bomb_rate).toFixed(1) }}%</p>
        </div>
        <span class="text-gray-500 text-sm">今日</span>
      </div>
    </div>
  </div>
</template>
```

- [ ] **Step 2: 更新 script——计算涨跌比**

```typescript
const ratio = computed(() => {
  if (props.overview.fall_count === 0) return '∞'
  return (props.overview.rise_count / props.overview.fall_count).toFixed(2)
})
```

删除不再使用的 `sentiment` 相关计算（`sentimentBorder`、`sentimentDot`），因为炸板率卡片不再用 sentiment 字段。

- [ ] **Step 3: Commit**

```bash
git add src/src/components/KpiCards.vue
git commit -m "fix(ui): correct KpiCards field names to match Rust MarketOverview"
```

---

### Task 9: 修正 TabLimitUp.vue——涨停榜字段 + 热门板块来源

**Files:**
- Modify: `src/src/components/tabs/TabLimitUp.vue`

- [ ] **Step 1: 修正 template 字段名**

`stock.stock_name` → `stock.stock_chi_name`

```vue
<td class="px-4 py-2 font-medium">{{ stock.stock_chi_name }}</td>
```

- [ ] **Step 2: 修正热门板块数据来源**

当前用 `plate_abnormal` 统计板块——这是错的。涨停股的板块信息在 `stock.surge_reason.related_plates` 中：

```typescript
const plates = computed(() => {
  const counts: Record<string, number> = {}
  store.report?.limit_up.forEach(stock => {
    stock.surge_reason?.related_plates.forEach(p => {
      counts[p.plate_name] = (counts[p.plate_name] || 0) + 1
    })
  })
  return Object.entries(counts)
    .map(([name, count]) => ({ name, count }))
    .sort((a, b) => b.count - a.count)
    .slice(0, 15)
})
```

- [ ] **Step 3: 修正炸板率取值**

Rust 的 `bomb_rate` 是 f64 小数（如 0.25），前端直接 ×100 显示百分比：

```typescript
const bombRate = computed(() => store.report?.overview.bomb_rate || 0)
```

已经在 KpiCards 中处理，TabLimitUp 中 `bombRate > 0.3` 的判断保持不变（0.3 = 30%）。

- [ ] **Step 4: Commit**

```bash
git add src/src/components/tabs/TabLimitUp.vue
git commit -m "fix(ui): correct TabLimitUp field names and hot plates data source"
```

---

### Task 10: 修正 TabPlate.vue——板块异动字段

**Files:**
- Modify: `src/src/components/tabs/TabPlate.vue`

- [ ] **Step 1: 完全重写 TabPlate.vue**

当前用 `event.description` 和 `event.related_stocks` 这些不存在的字段。Rust `PlateAbnormalEvent` 结构：

```rust
pub struct PlateAbnormalEvent {
    pub id: i64,
    pub event_timestamp: i64,
    pub plate_abnormal_event_data: Option<PlateAbnormalEventData>,
    // ...
}
pub struct PlateAbnormalEventData {
    pub plate_name: Option<String>,
    pub pcp: Option<f64>,
    pub related_stocks: Vec<RelatedStock>,
}
```

```vue
<template>
  <div>
    <h2 class="text-xl font-bold mb-4">🗺️ 板块异动</h2>

    <!-- 板块热度排行 -->
    <div class="mb-6">
      <h3 class="text-lg font-medium mb-2">板块热度</h3>
      <div class="space-y-2">
        <div v-for="(item, idx) in plateHeat" :key="idx" class="flex items-center gap-3">
          <span class="w-24 text-sm text-gray-600 truncate">{{ item.name }}</span>
          <div class="flex-1 bg-gray-200 rounded-full h-6 overflow-hidden">
            <div
              class="h-full bg-blue-500 rounded-full transition-all duration-500"
              :style="{ width: item.percent + '%' }"
            ></div>
          </div>
          <span class="w-12 text-sm text-right text-gray-700">{{ item.count }}</span>
        </div>
      </div>
    </div>

    <!-- 异动事件列表 -->
    <div>
      <h3 class="text-lg font-medium mb-2">异动事件</h3>
      <div v-if="sortedEvents.length === 0" class="text-gray-400 py-8 text-center">
        暂无板块异动数据
      </div>
      <div class="space-y-3">
        <div
          v-for="event in sortedEvents"
          :key="event.id"
          class="border-l-4 border-blue-500 pl-4 py-2"
        >
          <div class="flex items-center gap-2 mb-1">
            <span class="text-sm font-medium text-blue-600">
              {{ event.plate_name || '未知板块' }}
            </span>
            <span class="text-xs text-gray-500">{{ formatTime(event.event_timestamp) }}</span>
            <span
              class="text-xs px-1.5 py-0.5 rounded"
              :class="event.pcp >= 0 ? 'bg-red-100 text-red-600' : 'bg-green-100 text-green-600'"
            >
              {{ formatPcp(event.pcp) }}
            </span>
          </div>
          <div class="flex gap-1 mt-1 flex-wrap">
            <span
              v-for="stock in event.stocks"
              :key="stock.symbol"
              class="px-2 py-0.5 bg-gray-100 rounded text-xs font-mono"
            >
              {{ stock.name }}
            </span>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useReportStore } from '../../stores/report'

const store = useReportStore()

interface FlatEvent {
  id: number
  plate_name: string
  pcp: number
  event_timestamp: number
  stocks: { symbol: string; name: string }[]
}

const flatEvents = computed<FlatEvent[]>(() => {
  const events: FlatEvent[] = []
  store.report?.plate_abnormal.forEach(e => {
    const d = e.plate_abnormal_event_data
    if (!d) return
    events.push({
      id: e.id,
      plate_name: d.plate_name || '未知板块',
      pcp: d.pcp ?? 0,
      event_timestamp: e.event_timestamp,
      stocks: d.related_stocks.map(s => ({ symbol: s.symbol, name: s.name })),
    })
  })
  return events
})

const sortedEvents = computed(() =>
  [...flatEvents.value].sort((a, b) => b.event_timestamp - a.event_timestamp)
)

const plateHeat = computed(() => {
  const counts: Record<string, number> = {}
  flatEvents.value.forEach(e => {
    counts[e.plate_name] = (counts[e.plate_name] || 0) + 1
  })
  const max = Math.max(...Object.values(counts), 1)
  return Object.entries(counts)
    .map(([name, count]) => ({
      name,
      count,
      percent: (count / max) * 100,
    }))
    .sort((a, b) => b.count - a.count)
    .slice(0, 10)
})

function formatTime(ts: number): string {
  const d = new Date(ts * 1000)
  return d.toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' })
}

function formatPcp(pcp: number): string {
  if (pcp === 0) return '0%'
  return (pcp > 0 ? '+' : '') + (pcp * 100).toFixed(1) + '%'
}
</script>
```

关键变化：
- 从 `event.plate_abnormal_event_data` 提取板块名/涨跌幅/关联股票
- `pcp` 是 0-1 小数，显示时 ×100
- `event_timestamp` 是 Unix 秒，用 `new Date(ts * 1000)` 转换

- [ ] **Step 2: Commit**

```bash
git add src/src/components/tabs/TabPlate.vue
git commit -m "fix(ui): rewrite TabPlate to match PlateAbnormalEvent Rust structure"
```

---

### Task 11: 修正 TabSentiment.vue——舆情分析字段

**Files:**
- Modify: `src/src/components/tabs/TabSentiment.vue`

- [ ] **Step 1: 修正 template 字段名**

Rust `CommunityArticle` 没有 `summary`、`author`、`heat` 字段：

```vue
<template>
  <div>
    <h2 class="text-xl font-bold mb-4">📰 舆情分析</h2>

    <!-- 空状态 -->
    <div v-if="!sentiment" class="text-gray-400 py-12 text-center">
      <p class="text-lg">暂无舆情数据</p>
      <p class="text-sm mt-2">数据将通过定时任务生成静态文件后加载</p>
    </div>

    <template v-else>
      <!-- 趋势标签 -->
      <div class="mb-6">
        <div class="flex items-center gap-2 mb-2">
          <span class="text-lg font-medium">趋势：</span>
          <span class="px-3 py-1 rounded-full text-sm" :class="trendColor">
            {{ trend }}
          </span>
        </div>
        <p class="text-sm text-gray-600">{{ sentiment.summary }}</p>
      </div>

      <!-- 热门股票 TOP10 -->
      <div class="mb-6">
        <h3 class="text-lg font-medium mb-2">热门股票 TOP10</h3>
        <div v-if="hotStocks.length === 0" class="text-gray-400 text-sm">暂无</div>
        <div class="space-y-2">
          <div
            v-for="(item, idx) in hotStocks"
            :key="idx"
            class="flex items-center gap-3 p-3 bg-white rounded-lg shadow-sm"
          >
            <span class="w-6 text-gray-400 font-bold">{{ idx + 1 }}</span>
            <div class="flex-1">
              <div class="font-mono font-medium">{{ item[0] }}</div>
              <div class="text-sm text-gray-600">{{ item[1] }}</div>
            </div>
            <span class="text-sm text-gray-700">{{ item[2] }}次</span>
          </div>
        </div>
      </div>

      <!-- 热门文章列表 -->
      <div>
        <h3 class="text-lg font-medium mb-2">热门文章</h3>
        <div class="space-y-3">
          <details
            v-for="article in articles"
            :key="article.article_id"
            class="p-4 bg-white rounded-lg shadow-sm"
          >
            <summary class="cursor-pointer font-medium">{{ article.title }}</summary>
            <div class="mt-2 text-sm text-gray-700">
              <p>{{ article.content || '暂无内容' }}</p>
              <div class="flex gap-2 mt-2 flex-wrap">
                <span class="px-2 py-0.5 bg-gray-100 rounded text-xs">{{ article.user.nickname }}</span>
                <span class="px-2 py-0.5 bg-gray-100 rounded text-xs">❤️ {{ article.like_count }}</span>
                <span class="px-2 py-0.5 bg-gray-100 rounded text-xs">💬 {{ article.comment_count }}</span>
                <span class="px-2 py-0.5 bg-gray-100 rounded text-xs">🔄 {{ article.forward_count }}</span>
              </div>
            </div>
          </details>
        </div>
      </div>
    </template>
  </div>
</template>
```

- [ ] **Step 2: 更新 script——直接用 store 数据**

```typescript
import { computed } from 'vue'
import { useReportStore } from '../../stores/report'

const store = useReportStore()
const sentiment = computed(() => store.report?.sentiment)
const trend = computed(() => sentiment.value?.trend || '暂无')
const hotStocks = computed(() => sentiment.value?.hot_stocks || [])
const articles = computed(() => sentiment.value?.articles || [])

const trendColor = computed(() => {
  const t = trend.value
  if (t.includes('涨') || t.includes('多') || t.includes('强')) return 'bg-red-100 text-red-700'
  if (t.includes('跌') || t.includes('空') || t.includes('弱')) return 'bg-green-100 text-green-700'
  return 'bg-yellow-100 text-yellow-700'
})
```

- [ ] **Step 3: Commit**

```bash
git add src/src/components/tabs/TabSentiment.vue
git commit -m "fix(ui): correct TabSentiment field names to match CommunityArticle"
```

---

### Task 12: 修正 TabAi.vue——AI 观点空状态

**Files:**
- Modify: `src/src/components/tabs/TabAi.vue`

- [ ] **Step 1: 加空状态**

Phase A 没有 LLM key，AI 观点为空。需要加空状态而不是显示空卡片：

```vue
<template>
  <div>
    <h2 class="text-xl font-bold mb-4">🤖️ AI 三角色观点</h2>

    <div v-if="analyses.length === 0" class="text-gray-400 py-12 text-center">
      <p class="text-lg">未配置 AI 分析</p>
      <p class="text-sm mt-2">设置 OPENROUTER_API_KEY 环境变量后重新构建即可启用</p>
    </div>

    <div v-else class="grid grid-cols-1 md:grid-cols-3 gap-4">
      <!-- 现有角色卡片代码保持不变 -->
    </div>
  </div>
</template>
```

- [ ] **Step 2: Commit**

```bash
git add src/src/components/tabs/TabAi.vue
git commit -m "fix(ui): add empty state for TabAi when no LLM key configured"
```

---

### Task 13: 修正 TabTgb.vue——淘股吧字段对齐 + 空状态

**Files:**
- Modify: `src/src/components/tabs/TabTgb.vue`

- [ ] **Step 1: 更新字段名和空状态**

Rust `TgbUserSummary` 有 `nickname`、`top_post_title` 字段：

```vue
<template>
  <div>
    <h2 class="text-xl font-bold mb-4">📝 淘股吧</h2>

    <div v-if="users.length === 0" class="text-gray-400 py-12 text-center">
      <p class="text-lg">暂无淘股吧数据</p>
      <p class="text-sm mt-2">数据将通过定时任务生成静态文件后加载</p>
    </div>

    <div v-else class="space-y-3">
      <div
        v-for="user in users"
        :key="user.user_id"
        class="p-4 bg-white rounded-lg shadow-sm hover:shadow-md transition"
      >
        <div class="flex items-center justify-between mb-3">
          <div class="flex items-center gap-3">
            <span class="w-10 h-10 rounded-full bg-blue-500 flex items-center justify-center text-white font-bold">
              {{ user.nickname.slice(0, 2) }}
            </span>
            <div>
              <h3 class="font-bold">{{ user.nickname }}</h3>
              <p class="text-sm text-gray-500">{{ user.post_count }} 篇帖子</p>
            </div>
          </div>
          <span class="text-sm text-gray-400">{{ user.last_update }}</span>
        </div>
        <p class="text-sm text-gray-600 mt-2">置顶：{{ user.top_post_title }}</p>
      </div>
    </div>
  </div>
</template>
```

- [ ] **Step 2: 更新 script**

```typescript
import { computed } from 'vue'
import { useReportStore } from '../../stores/report'

const store = useReportStore()
const users = computed(() => store.report?.taoguba_users || [])
```

- [ ] **Step 3: Commit**

```bash
git add src/src/components/tabs/TabTgb.vue
git commit -m "fix(ui): align TabTgb fields with TgbUserSummary and add empty state"
```

---

### Task 14: App.vue 加错误提示和 loading 状态

**Files:**
- Modify: `src/src/App.vue`

- [ ] **Step 1: 在 main content 顶部加错误/loading 提示**

```vue
<!-- 在 <main> 内部、KpiCards 上方添加 -->
<div v-if="reportStore.error" class="bg-red-50 border border-red-200 rounded-lg p-4 mb-4">
  <p class="text-red-700 text-sm">⚠️ 数据获取失败：{{ reportStore.error }}</p>
  <button @click="refreshData" class="mt-2 text-sm text-red-600 underline">重试</button>
</div>

<div v-if="reportStore.loading && !reportStore.report" class="text-center py-20 text-gray-400">
  <p class="text-lg animate-pulse">加载中...</p>
</div>
```

- [ ] **Step 2: 确认 App.vue 的 import 路径**

```typescript
import { useReportStore } from '../stores/report'
```

- [ ] **Step 3: Commit**

```bash
git add src/src/App.vue
git commit -m "feat(ui): add error and loading states to App.vue"
```

---

### Task 15: 端到端验证

**Files:** (无变更，仅验证)

- [ ] **Step 1: 重新编译 WASM**

```bash
cd src && npm run wasm:build
```
Expected: 成功，`src/src/wasm/` 下有新的 `.wasm` 和 `.js` 文件

- [ ] **Step 2: 启动 Vite dev server**

```bash
cd src && npm run dev
```
Expected: Vite 启动无报错

- [ ] **Step 3: 浏览器验证**

打开 `http://localhost:5173`：
- KPI 卡片显示真实数据（上涨/下跌家数、涨跌比、炸板率），不是 mock 的 120/30
- 涨停榜 Tab 显示当天真实涨停股票，不是贵州茅台/平安银行/招商银行
- 板块异动 Tab 显示真实板块事件，带时间+涨跌幅
- AI 观点 Tab 显示"未配置 AI 分析"
- 淘股吧 Tab 显示"暂无淘股吧数据"
- 舆情分析 Tab 显示"暂无舆情数据"

- [ ] **Step 4: 检查浏览器 console**

Expected: 无 CORS 报错，无 JS 异常。
如果选股宝 API 有 CORS 限制，在 `vite.config.ts` 加 dev proxy：

```typescript
server: {
  proxy: {
    '/api/xuangubao': {
      target: 'https://flash-api.xuangubao.cn',
      changeOrigin: true,
      rewrite: (path) => path.replace(/^\/api\/xuangubao/, '/api'),
    },
  },
},
```

同时 WASM 里的 `BASE_URL` 需要在 dev 模式改用 `/api/xuangubao`。如果遇到 CORS 问题才处理，先跳过。

- [ ] **Step 5: 确认刷新按钮可用**

点击"刷新数据" → loading 状态显示 → 数据重新加载

- [ ] **Step 6: Commit**（如果有修复）

```bash
git commit -m "chore: end-to-end verification fixes"
```
