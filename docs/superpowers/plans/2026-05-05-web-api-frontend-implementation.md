# Web REST API + Frontend Multi-Source + Dark Trading Desk Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add JSON REST API to web crate, support 3 frontend data sources (env-var switchable), and redesign UI as dark trading-desk theme.

**Architecture:** Web crate auto-fetches data at startup via `core` API clients. Frontend uses `VITE_DATA_SOURCE` env var to pick data source (api/json/wasm). All Vue components rewritten with dark theme (neon green/red, mono data font, Bloomberg-terminal aesthetic).

**Tech Stack:** Rust/Axum (web crate), Vue 3 + TypeScript + Tailwind + Pinia (frontend)

---

### Task 1: Add REST API endpoints to web crate

**Files:**
- Modify: `crates/web/Cargo.toml`
- Modify: `crates/web/src/main.rs`

- [ ] **Step 1: Add serde and serde_json to web crate deps**

Edit `crates/web/Cargo.toml`:

```toml
[package]
name = "trending-agent-web"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "trending-agent-web"
path = "src/main.rs"

[dependencies]
trending-agent-core = { path = "../core" }
tokio = { workspace = true, features = ["rt-multi-thread", "net"] }
axum = "0.7"
anyhow = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
```

- [ ] **Step 2: Rewrite `crates/web/src/main.rs` with new REST API**

```rust
use std::sync::Arc;

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde_json::{json, Value};
use tokio::sync::RwLock;

use trending_agent_core::api::xuangubao::XuanGubaoExtra;
use trending_agent_core::api::Client as ApiClient;
use trending_agent_core::report::report::Report;

#[derive(Clone)]
pub struct WebState {
    pub report: Arc<RwLock<Option<Report>>>,
}

impl Default for WebState {
    fn default() -> Self {
        Self::new()
    }
}

impl WebState {
    pub fn new() -> Self {
        Self {
            report: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn set_report(&self, report: Report) {
        let mut r = self.report.write().await;
        *r = Some(report);
    }

    pub async fn get_report(&self) -> Option<Report> {
        self.report.read().await.clone()
    }
}

async fn refresh_report(state: &WebState) -> anyhow::Result<()> {
    let client = ApiClient::new();

    let (overview, limit_up, limit_down, plate_abnormal) = tokio::join!(
        client.xuangubao.fetch_market_overview(),
        client.xuangubao.fetch_limit_up(),
        client.xuangubao.fetch_limit_down(),
        client.xuangubao.fetch_plate_abnormal(30),
    );

    let overview = overview?;
    let limit_up = limit_up.unwrap_or_default();
    let limit_down = limit_down.unwrap_or_default();
    let plate_abnormal = plate_abnormal.unwrap_or_default();

    let report = Report::new(
        overview,
        limit_up,
        plate_abnormal.clone(),
        vec![],
        vec![],
        vec![],
        None,
        vec![],
        XuanGubaoExtra {
            limit_down,
            plate_abnormal,
        },
    );
    state.set_report(report).await;
    Ok(())
}

async fn start_web_server(port: u16, state: WebState) {
    let app = Router::new()
        .route("/api/report", get(get_report_json))
        .route("/api/report/live", get(get_report_live))
        .with_state(state);

    let addr = format!("0.0.0.0:{}", port);
    println!("🌐 Web API: http://localhost:{}", port);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[derive(serde::Deserialize)]
struct ReportQuery {
    section: Option<String>,
}

async fn get_report_json(
    State(state): State<WebState>,
    Query(q): Query<ReportQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    let report = state.get_report().await.ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    let full = serde_json::to_value(&report).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let result = match q.section {
        Some(sections) => {
            let mut out = json!({});
            let obj = out.as_object_mut().unwrap();
            for key in sections.split(',').map(|s| s.trim()) {
                match key {
                    "summary" => {
                        obj.insert("risk_level".into(), full["risk_level"].clone());
                        obj.insert("opportunity_signals".into(), full["opportunity_signals"].clone());
                        obj.insert("decision_summary".into(), full["decision_summary"].clone());
                    }
                    _ => {
                        if let Some(val) = full.get(key) {
                            obj.insert(key.to_string(), val.clone());
                        }
                    }
                }
            }
            out
        }
        None => full,
    };

    Ok(Json(result))
}

// GET /api/report/live — force refresh, return latest
async fn get_report_live(State(state): State<WebState>) -> Result<impl IntoResponse, StatusCode> {
    refresh_report(&state)
        .await
        .map_err(|e| {
            eprintln!("refresh error: {}", e);
            StatusCode::SERVICE_UNAVAILABLE
        })?;
    let report = state.get_report().await.ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let json = serde_json::to_value(&report).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    let state = WebState::new();

    // 启动时拉取初始数据（失败不阻塞启动）
    if let Err(e) = refresh_report(&state).await {
        eprintln!("⚠️ 初始数据拉取失败: {}，API 将在首次请求时重试", e);
    }

    start_web_server(8080, state).await;
    Ok(())
}
```

- [ ] **Step 3: Verify compilation**

```bash
cargo build -p trending-agent-web
```

- [ ] **Step 4: Commit**

```bash
git add crates/web/Cargo.toml crates/web/src/main.rs
git commit -m "feat(web): add JSON REST API endpoints /api/report and /api/report/live"
```

---

### Task 2: Update `.env` with VITE_DATA_SOURCE variable

**Files:**
- Modify: `.env`

- [ ] **Step 1: Add VITE_DATA_SOURCE to .env**

Edit `.env`, append:

```dotenv
VITE_DATA_SOURCE=api
```

- [ ] **Step 2: Commit**

```bash
git add .env
git commit -m "chore: add VITE_DATA_SOURCE env var for frontend data source selection"
```

---

### Task 3: Update Vite config for multi-source and proxy

**Files:**
- Modify: `src/vite.config.ts`
- Modify: `src/src/env.d.ts`

- [ ] **Step 1: Update `src/vite.config.ts`**

```ts
import { defineConfig, loadEnv } from 'vite'
import vue from '@vitejs/plugin-vue'
import wasm from 'vite-plugin-wasm'
import topLevelAwait from 'vite-plugin-top-level-await'
import { resolve } from 'path'

export default defineConfig(({ mode }) => {
  const rootEnv = loadEnv(mode, resolve(__dirname, '..'), '')

  return {
    resolve: {
      alias: {
        '@': resolve(__dirname, 'src'),
      },
    },
    plugins: [
      vue(),
      wasm(),
      topLevelAwait(),
    ],
    define: {
      __OPENROUTER_KEY__: JSON.stringify(rootEnv.OPENROUTER_API_KEY || ''),
      __JIUYAN_PHONE__: JSON.stringify(rootEnv.JIUYAN_PHONE || ''),
      __JIUYAN_PASSWORD__: JSON.stringify(rootEnv.JIUYAN_PASSWORD || ''),
      __DATA_SOURCE__: JSON.stringify(rootEnv.VITE_DATA_SOURCE || 'json'),
    },
    server: {
      proxy: {
        '/api/xgb': {
          target: 'https://flash-api.xuangubao.cn',
          changeOrigin: true,
          rewrite: (path) => path.replace(/^\/api\/xgb/, '/api'),
        },
        // 开发时转发 /api 到 web crate
        '/api': {
          target: 'http://localhost:8080',
          changeOrigin: true,
        },
      },
    },
  }
})
```

- [ ] **Step 2: Update `src/src/env.d.ts`**

```ts
/// <reference types="vite/client" />
declare const __OPENROUTER_KEY__: string
declare const __JIUYAN_PHONE__: string
declare const __JIUYAN_PASSWORD__: string
declare const __DATA_SOURCE__: 'api' | 'json' | 'wasm'
```

- [ ] **Step 3: Commit**

```bash
git add src/vite.config.ts src/src/env.d.ts
git commit -m "feat: add multi-source data support and /api proxy to Vite config"
```

---

### Task 4: Rewrite frontend store for three data sources

**Files:**
- Modify: `src/src/stores/report.ts`

- [ ] **Step 1: Rewrite `src/src/stores/report.ts`**

```ts
import { reactive } from 'vue'

interface Report {
  date: string
  overview: {
    rise_count: number
    fall_count: number
    limit_up_count: number
    limit_down_count: number
    bomb_rate: number
  }
  limit_up: Array<{
    symbol: string
    stock_chi_name: string
    price: number
    change_percent: number
    limit_up_days: number
    turnover_ratio: number
    total_capital: number
    break_limit_up_times: number
    surge_reason: {
      stock_reason: string
      related_plates: Array<{ plate_name: string }>
    } | null
  }>
  plate_abnormal: Array<any>
  risk_level: string
  opportunity_signals: string[]
  decision_summary: string
}

const API_BASE = import.meta.env.PROD ? '' : 'http://localhost:8080'

const store = reactive({
  report: null as Report | null,
  loading: false,
  error: '',
})

async function fetchFromApi(): Promise<Report> {
  const resp = await fetch(`${API_BASE}/api/report`)
  if (!resp.ok) throw new Error(`API error: HTTP ${resp.status}`)
  return resp.json()
}

async function fetchFromJson(): Promise<Report> {
  const resp = await fetch('/report.json')
  if (!resp.ok) throw new Error(`JSON error: HTTP ${resp.status}`)
  return resp.json()
}

async function fetchFromWasm(): Promise<Report> {
  const mod = await import('../wasm/trending_agent_wasm.js')
  await mod.default()
  const isDev = import.meta.env.DEV
  mod.set_xgb_base_url(isDev ? '/api/xgb' : 'https://flash-api.xuangubao.cn/api')
  const json = await mod.run_report()
  return JSON.parse(json)
}

async function fetchReport() {
  store.loading = true
  store.error = ''
  try {
    switch (__DATA_SOURCE__) {
      case 'api':
        store.report = await fetchFromApi()
        break
      case 'json':
        store.report = await fetchFromJson()
        break
      case 'wasm':
        store.report = await fetchFromWasm()
        break
    }
  } catch (e: any) {
    store.error = e.message || String(e)
    console.error('Fetch failed:', e)
  } finally {
    store.loading = false
  }
}

export function useReportStore() {
  return store
}

export { fetchReport }
```

- [ ] **Step 2: Commit**

```bash
git add src/src/stores/report.ts
git commit -m "refactor: rewrite store for three data source dispatch (api/json/wasm)"
```

---

### Task 5: Dark Trading Desk — App.vue + main.ts + style.css

**Files:**
- Modify: `src/src/App.vue`
- Modify: `src/src/main.ts`
- Modify: `src/src/style.css`

- [ ] **Step 1: Rewrite `src/src/style.css`**

```css
@tailwind base;
@tailwind components;
@tailwind utilities;

@import url('https://fonts.googleapis.com/css2?family=JetBrains+Mono:wght@400;500;700&family=DM+Sans:wght@400;500;700;900&display=swap');

@layer base {
  :root {
    --bg-primary: #0a0e17;
    --bg-secondary: #111827;
    --bg-card: #141c2b;
    --border-subtle: #1e2a3a;
    --border-active: #2a3a50;
    --text-primary: #e2e8f0;
    --text-secondary: #8899aa;
    --text-muted: #566477;
    --green: #00e676;
    --green-dim: #00c853;
    --red: #ff1744;
    --red-dim: #d50000;
    --amber: #ffab00;
    --amber-dim: #ff8f00;
    --blue: #448aff;
    --blue-dim: #2962ff;
    --font-mono: 'JetBrains Mono', monospace;
    --font-sans: 'DM Sans', -apple-system, sans-serif;
  }

  * { box-sizing: border-box; margin: 0; padding: 0; }

  body {
    font-family: var(--font-sans);
    background: var(--bg-primary);
    color: var(--text-primary);
    -webkit-font-smoothing: antialiased;
    -moz-osx-font-smoothing: grayscale;
  }

  ::-webkit-scrollbar { width: 6px; }
  ::-webkit-scrollbar-track { background: var(--bg-primary); }
  ::-webkit-scrollbar-thumb { background: var(--border-active); border-radius: 3px; }

  ::selection { background: rgba(0, 230, 118, 0.2); color: var(--green); }
}

@layer components {
  .glow-card {
    background: var(--bg-card);
    border: 1px solid var(--border-subtle);
    border-radius: 8px;
    transition: border-color 0.3s, box-shadow 0.3s;
  }
  .glow-card:hover {
    border-color: var(--border-active);
    box-shadow: 0 0 20px rgba(68, 138, 255, 0.05);
  }

  .data-table {
    width: 100%;
    border-collapse: collapse;
    font-family: var(--font-mono);
    font-size: 13px;
  }
  .data-table th {
    text-align: left;
    padding: 10px 16px;
    color: var(--text-muted);
    font-weight: 500;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    border-bottom: 1px solid var(--border-subtle);
  }
  .data-table td {
    padding: 10px 16px;
    border-bottom: 1px solid rgba(30, 42, 58, 0.5);
    color: var(--text-secondary);
  }
  .data-table tr:hover td {
    background: rgba(68, 138, 255, 0.03);
    color: var(--text-primary);
  }

  .tab-btn {
    padding: 12px 24px;
    font-size: 13px;
    font-weight: 500;
    color: var(--text-muted);
    background: none;
    border: none;
    border-bottom: 2px solid transparent;
    cursor: pointer;
    transition: all 0.2s;
    font-family: var(--font-sans);
  }
  .tab-btn:hover { color: var(--text-secondary); }
  .tab-btn.active {
    color: var(--blue);
    border-bottom-color: var(--blue);
  }

  .pulse-dot {
    display: inline-block;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--green);
    animation: pulse 2s infinite;
  }
  @keyframes pulse {
    0%, 100% { opacity: 0.4; }
    50% { opacity: 1; }
  }
}
```

- [ ] **Step 2: Rewrite `src/src/main.ts`**

```ts
import { createApp } from 'vue'
import { createPinia } from 'pinia'
import App from './App.vue'
import './style.css'

const app = createApp(App)
app.use(createPinia())
app.mount('#app')
```

- [ ] **Step 3: Rewrite `src/src/App.vue`**

```vue
<template>
  <div class="min-h-screen" style="background: var(--bg-primary)">
    <!-- Top Bar -->
    <header class="fixed top-0 w-full z-50 border-b" style="background: var(--bg-secondary); border-color: var(--border-subtle)">
      <div class="max-w-7xl mx-auto px-6 h-14 flex items-center justify-between">
        <div class="flex items-center gap-3">
          <span class="pulse-dot"></span>
          <h1 class="text-sm font-bold tracking-wider" style="color: var(--text-primary); font-family: var(--font-mono)">
            TRENDING<span style="color: var(--blue)">.AGENT</span>
          </h1>
        </div>
        <div class="flex items-center gap-4">
          <span v-if="reportStore.report" class="text-xs" style="color: var(--text-muted); font-family: var(--font-mono)">
            {{ reportStore.report.date }}
          </span>
          <button
            @click="refreshData"
            :disabled="reportStore.loading"
            class="px-4 py-1.5 text-xs font-medium rounded border transition-all"
            :style="{
              background: reportStore.loading ? 'transparent' : 'rgba(68,138,255,0.1)',
              color: reportStore.loading ? 'var(--text-muted)' : 'var(--blue)',
              borderColor: reportStore.loading ? 'var(--border-subtle)' : 'rgba(68,138,255,0.3)'
            }"
          >
            {{ reportStore.loading ? '刷新中...' : '↻ 刷新' }}
          </button>
        </div>
      </div>
    </header>

    <!-- Main -->
    <main class="max-w-7xl mx-auto px-6 pt-20 pb-12">
      <!-- Error -->
      <div v-if="reportStore.error" class="mb-6 p-4 rounded-lg flex items-center justify-between" style="background: rgba(255,23,68,0.08); border: 1px solid rgba(255,23,68,0.2)">
        <div class="flex items-center gap-3">
          <span style="color: var(--red); font-size: 16px">⚠</span>
          <span class="text-sm" style="color: var(--red)">{{ reportStore.error }}</span>
        </div>
        <button @click="refreshData" class="text-xs underline" style="color: var(--red)">重试</button>
      </div>

      <!-- Loading -->
      <div v-if="reportStore.loading && !reportStore.report" class="flex flex-col items-center justify-center py-32 gap-4">
        <div class="pulse-dot" style="width: 12px; height: 12px"></div>
        <span class="text-sm" style="color: var(--text-muted); font-family: var(--font-mono)">加载市场数据...</span>
      </div>

      <!-- Content -->
      <template v-if="reportStore.report">
        <KpiCards :overview="reportStore.report.overview" :risk-level="reportStore.report.risk_level" :decision-summary="reportStore.report.decision_summary" />

        <TabBar v-model="activeTab" />

        <div class="mt-6">
          <component :is="currentTabComponent" />
        </div>
      </template>
    </main>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import { useReportStore, fetchReport } from '@/stores/report'
import KpiCards from './components/KpiCards.vue'
import TabBar from './components/TabBar.vue'
import TabLimitUp from './components/tabs/TabLimitUp.vue'
import TabPlate from './components/tabs/TabPlate.vue'
import TabSentiment from './components/tabs/TabSentiment.vue'
import TabAi from './components/tabs/TabAi.vue'
import TabTgb from './components/tabs/TabTgb.vue'

const reportStore = useReportStore()
const activeTab = ref('limit-up')

const tabComponents: Record<string, any> = {
  'limit-up': TabLimitUp,
  'plate': TabPlate,
  'sentiment': TabSentiment,
  'ai': TabAi,
  'tgb': TabTgb,
}

const currentTabComponent = computed(() => tabComponents[activeTab.value])

const refreshData = () => fetchReport()

fetchReport()
</script>
```

- [ ] **Step 4: Commit**

```bash
git add src/src/style.css src/src/main.ts src/src/App.vue
git commit -m "feat: dark trading desk theme — App shell with terminal-style header"
```

---

### Task 6: Dark Trading Desk — KPI Cards

**Files:**
- Modify: `src/src/components/KpiCards.vue`

- [ ] **Step 1: Rewrite `src/src/components/KpiCards.vue`**

```vue
<template>
  <div class="grid grid-cols-2 lg:grid-cols-4 gap-4 mb-8">
    <!-- Rise -->
    <div class="glow-card p-5" style="border-left: 3px solid var(--green)">
      <p class="text-xs tracking-wide mb-3" style="color: var(--text-muted)">上涨家数</p>
      <p class="text-3xl font-bold" style="color: var(--green); font-family: var(--font-mono)">{{ overview.rise_count.toLocaleString() }}</p>
    </div>

    <!-- Fall -->
    <div class="glow-card p-5" style="border-left: 3px solid var(--red)">
      <p class="text-xs tracking-wide mb-3" style="color: var(--text-muted)">下跌家数</p>
      <p class="text-3xl font-bold" style="color: var(--red); font-family: var(--font-mono)">{{ overview.fall_count.toLocaleString() }}</p>
    </div>

    <!-- Ratio -->
    <div class="glow-card p-5" style="border-left: 3px solid var(--blue)">
      <p class="text-xs tracking-wide mb-3" style="color: var(--text-muted)">涨跌比</p>
      <p class="text-3xl font-bold" style="font-family: var(--font-mono)" :style="{ color: ratioColor }">{{ ratio }}</p>
      <p class="text-xs mt-2" style="color: var(--text-muted)">{{ overview.limit_up_count }}涨停 / {{ overview.limit_down_count }}跌停</p>
    </div>

    <!-- Bomb Rate -->
    <div class="glow-card p-5" style="border-left: 3px solid var(--amber)">
      <p class="text-xs tracking-wide mb-3" style="color: var(--text-muted)">炸板率</p>
      <p class="text-3xl font-bold" style="font-family: var(--font-mono)" :style="{ color: bombColor }">{{ (overview.bomb_rate * 100).toFixed(1) }}%</p>
    </div>
  </div>

  <!-- Decision Summary Bar -->
  <div class="glow-card px-5 py-3 mb-8 flex items-center gap-6 text-sm" v-if="decisionSummary">
    <span class="px-2 py-0.5 rounded text-xs font-bold font-mono" :style="{ background: riskBadgeBg, color: riskBadgeColor }">
      {{ riskLevel }}
    </span>
    <span style="color: var(--text-secondary)">{{ decisionSummary }}</span>
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

const ratioColor = computed(() => {
  const r = props.overview.rise_count / Math.max(props.overview.fall_count, 1)
  if (r >= 2) return 'var(--green)'
  if (r >= 1) return 'var(--blue)'
  return 'var(--red)'
})

const bombColor = computed(() => {
  if (props.overview.bomb_rate > 0.3) return 'var(--red)'
  if (props.overview.bomb_rate > 0.2) return 'var(--amber)'
  return 'var(--green)'
})

const riskBadgeBg = computed(() => {
  switch (props.riskLevel) {
    case '高': return 'rgba(255,23,68,0.1)'
    case '中': return 'rgba(255,171,0,0.1)'
    default: return 'rgba(0,230,118,0.1)'
  }
})

const riskBadgeColor = computed(() => {
  switch (props.riskLevel) {
    case '高': return 'var(--red)'
    case '中': return 'var(--amber)'
    default: return 'var(--green)'
  }
})
</script>
```

- [ ] **Step 2: Commit**

```bash
git add src/src/components/KpiCards.vue
git commit -m "feat: dark theme KPI cards with glow borders and risk badge"
```

---

### Task 7: Dark Trading Desk — TabBar

**Files:**
- Modify: `src/src/components/TabBar.vue`

- [ ] **Step 1: Rewrite `src/src/components/TabBar.vue`**

```vue
<template>
  <div class="glow-card overflow-hidden">
    <div class="flex" style="border-bottom: 1px solid var(--border-subtle)">
      <button
        v-for="tab in tabs"
        :key="tab.id"
        @click="activeTab = tab.id"
        class="tab-btn"
        :class="{ active: activeTab === tab.id }"
      >
        {{ tab.label }}
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
const activeTab = defineModel<string>({ default: 'limit-up' })

const tabs = [
  { id: 'limit-up', label: '涨停榜' },
  { id: 'plate', label: '板块异动' },
  { id: 'sentiment', label: '舆情分析' },
  { id: 'ai', label: 'AI 观点' },
  { id: 'tgb', label: '淘股吧' },
]
</script>
```

- [ ] **Step 2: Commit**

```bash
git add src/src/components/TabBar.vue
git commit -m "feat: dark theme tab bar with glow card wrapper"
```

---

### Task 8: Dark Trading Desk — TabLimitUp

**Files:**
- Modify: `src/src/components/tabs/TabLimitUp.vue`

- [ ] **Step 1: Rewrite `src/src/components/tabs/TabLimitUp.vue`**

```vue
<template>
  <div>
    <!-- Summary Bar -->
    <div class="flex items-center gap-4 mb-6 text-sm">
      <span style="color: var(--text-muted)">
        共 <b style="color: var(--text-primary)">{{ stocks.length }}</b> 只涨停
      </span>
      <span v-if="bombRate > 0.2" style="color: var(--amber); font-family: var(--font-mono)">
        ⚠ 炸板率 {{ (bombRate * 100).toFixed(1) }}%
      </span>
    </div>

    <!-- Data Table -->
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
          <tr v-for="stock in sortedStocks" :key="stock.symbol">
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
            <td style="text-align:right; font-family: var(--font-mono); color: var(--green)">+{{ stock.change_percent.toFixed(2) }}%</td>
            <td style="text-align:right; font-family: var(--font-mono)">{{ stock.turnover_ratio.toFixed(2) }}%</td>
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
import { computed } from 'vue'
import { useReportStore } from '@/stores/report'

const store = useReportStore()

const stocks = computed(() => store.report?.limit_up || [])
const bombRate = computed(() => store.report?.overview.bomb_rate || 0)

const sortedStocks = computed(() =>
  [...stocks.value].sort((a, b) => b.limit_up_days - a.limit_up_days || b.total_capital - a.total_capital)
)
</script>
```

- [ ] **Step 2: Commit**

```bash
git add src/src/components/tabs/TabLimitUp.vue
git commit -m "feat: dark theme limit-up table with mono font and color-coded boards"
```

---

### Task 9: Dark Trading Desk — TabPlate

**Files:**
- Modify: `src/src/components/tabs/TabPlate.vue`

- [ ] **Step 1: Rewrite `src/src/components/tabs/TabPlate.vue`**

```vue
<template>
  <div>
    <div class="flex items-center gap-4 mb-6 text-sm">
      <span style="color: var(--text-muted)">
        <b style="color: var(--text-primary)">{{ flatEvents.length }}</b> 条异动事件
      </span>
    </div>

    <!-- Heat Map -->
    <div class="glow-card p-5 mb-6">
      <h3 class="text-xs font-medium mb-4" style="color: var(--text-muted); letter-spacing: 0.5px">板块热度</h3>
      <div class="space-y-2">
        <div v-for="(item, idx) in plateHeat" :key="idx" class="flex items-center gap-3">
          <span class="w-24 text-xs truncate" style="color: var(--text-secondary)">{{ item.name }}</span>
          <div class="flex-1 h-5 rounded-full overflow-hidden" style="background: var(--border-subtle)">
            <div
              class="h-full rounded-full transition-all duration-700"
              :style="{ width: item.percent + '%', background: `linear-gradient(90deg, var(--blue-dim), var(--blue))` }"
            ></div>
          </div>
          <span class="w-10 text-xs text-right font-mono" style="color: var(--text-secondary)">{{ item.count }}</span>
        </div>
        <div v-if="plateHeat.length === 0" class="text-center py-8 text-sm" style="color: var(--text-muted)">暂无板块异动数据</div>
      </div>
    </div>

    <!-- Event Timeline -->
    <div class="space-y-3">
      <div v-for="event in sortedEvents" :key="event.id" class="glow-card pl-4 py-3 pr-4 flex gap-4 items-start">
        <div class="w-1 self-stretch rounded-full flex-shrink-0"
          :style="{ background: event.pcp >= 0 ? 'var(--red)' : 'var(--green)' }"></div>
        <div class="flex-1 min-w-0">
          <div class="flex items-center gap-2 mb-1 flex-wrap">
            <span class="text-sm font-medium" style="color: var(--text-primary)">{{ event.plate_name || '未知板块' }}</span>
            <span class="text-xs font-mono" :style="{ color: event.pcp >= 0 ? 'var(--red)' : 'var(--green)' }">{{ formatPcp(event.pcp) }}</span>
          </div>
          <div class="flex gap-1 flex-wrap mt-2">
            <span v-for="stock in event.stocks" :key="stock.symbol"
              class="px-2 py-0.5 rounded text-xs font-mono"
              style="background: var(--border-subtle); color: var(--text-secondary)">
              {{ stock.symbol }} {{ stock.name }}
            </span>
          </div>
        </div>
        <span class="text-xs flex-shrink-0 font-mono" style="color: var(--text-muted)">{{ formatTime(event.event_timestamp) }}</span>
      </div>
      <div v-if="sortedEvents.length === 0" class="text-center py-12 text-sm" style="color: var(--text-muted)">暂无板块异动数据</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useReportStore } from '@/stores/report'

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
  store.report?.plate_abnormal.forEach((e: any) => {
    const d = e.plate_abnormal_event_data
    if (!d) return
    events.push({
      id: e.id,
      plate_name: d.plate_name || '未知板块',
      pcp: d.pcp ?? 0,
      event_timestamp: e.event_timestamp,
      stocks: (d.related_stocks || []).map((s: any) => ({ symbol: s.symbol, name: s.name })),
    })
  })
  return events
})

const sortedEvents = computed(() =>
  [...flatEvents.value].sort((a, b) => b.event_timestamp - a.event_timestamp)
)

const plateHeat = computed(() => {
  const counts: Record<string, number> = {}
  flatEvents.value.forEach(e => { counts[e.plate_name] = (counts[e.plate_name] || 0) + 1 })
  const max = Math.max(...Object.values(counts), 1)
  return Object.entries(counts)
    .map(([name, count]) => ({ name, count, percent: Math.round((count / max) * 100) }))
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

- [ ] **Step 2: Commit**

```bash
git add src/src/components/tabs/TabPlate.vue
git commit -m "feat: dark theme plate abnormal with heat map and event timeline"
```

---

### Task 10: Dark Trading Desk — TabSentiment, TabAi, TabTgb

**Files:**
- Modify: `src/src/components/tabs/TabSentiment.vue`
- Modify: `src/src/components/tabs/TabAi.vue`
- Modify: `src/src/components/tabs/TabTgb.vue`

- [ ] **Step 1: Rewrite `src/src/components/tabs/TabSentiment.vue`**

```vue
<template>
  <div>
    <div class="glow-card p-8 text-center">
      <span class="text-sm" style="color: var(--text-muted); font-family: var(--font-mono)">
        舆情分析需要在 web crate 中配置 OPENROUTER_API_KEY 后启用
      </span>
    </div>
  </div>
</template>
```

- [ ] **Step 2: Rewrite `src/src/components/tabs/TabAi.vue`**

```vue
<template>
  <div>
    <div class="glow-card p-8 text-center">
      <span class="text-sm" style="color: var(--text-muted); font-family: var(--font-mono)">
        AI 角色分析需要配置 OPENROUTER_API_KEY
      </span>
    </div>
  </div>
</template>
```

- [ ] **Step 3: Rewrite `src/src/components/tabs/TabTgb.vue`**

```vue
<template>
  <div>
    <div class="glow-card p-8 text-center">
      <span class="text-sm" style="color: var(--text-muted); font-family: var(--font-mono)">
        淘股吧数据需要在 web crate 中配置 TAOGUBA_USER 后启用
      </span>
    </div>
  </div>
</template>
```

- [ ] **Step 4: Commit**

```bash
git add src/src/components/tabs/TabSentiment.vue src/src/components/tabs/TabAi.vue src/src/components/tabs/TabTgb.vue
git commit -m "feat: dark theme placeholder tabs for sentiment, AI, and TGB"
```

---

### Task 11: Final Verification

- [ ] **Step 1: Verify Rust compilation**

```bash
cargo build
```
Expected: all crates compile without errors.

- [ ] **Step 2: Verify Vite build**

```bash
cd src && npm run wasm:build && npx vite build
```
Expected: production build succeeds.

- [ ] **Step 3: Test web crate API**

Start web crate in background, call endpoint:
```bash
cargo run --bin trending-agent-web &
sleep 5
curl -s http://localhost:8080/api/report | python3 -c "import json,sys; d=json.load(sys.stdin); print('keys:', list(d.keys())[:5], '...', 'limit_up:', len(d.get('limit_up',[])))"
kill %1
```
Expected: JSON response with report keys and data.

- [ ] **Step 4: Final commit (if any fixes)**

```bash
git add -A && git diff --cached --stat
# commit if needed
```
