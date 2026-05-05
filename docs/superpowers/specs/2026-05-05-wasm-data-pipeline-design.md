# WASM 数据管道：Vue 前端真实数据填充设计

**Date:** 2026-05-05
**Status:** Draft
**目标:** Rust 编译成 WASM，Vue 在浏览器直接调用 WASM 函数获取真实数据，不经过 HTTP 服务器

---

## 1. 问题诊断

当前链路：
```
Vue → stores/report.ts → fetchReport() → setTimeout → mockReport (3只假股票)
```

目标链路：
```
Vue → stores/report.ts → api/web.ts → wasm.run_report() → crates/core → 选股宝/韭研公社 API
```

核心差距：
- `crates/wasm/src/lib.rs` 是空壳（`run_report()` 只返回 `JsValue::NULL`）
- `stores/report.ts` 用硬编码 mock 数据
- TypeScript interface 字段名和 Rust struct 不对齐

---

## 2. 架构总览

```
┌─────────────────────────────────────────────────────┐
│  Browser                                             │
│  ┌──────────────────────────────────────────────┐   │
│  │  Vue 3 App (src/)                             │   │
│  │  ┌──────────┐  ┌──────────┐  ┌───────────┐   │   │
│  │  │ KpiCards  │  │ TabLimitUp│  │ TabPlate  │   │   │
│  │  └────┬─────┘  └────┬─────┘  └─────┬─────┘   │   │
│  │       └──────────────┼──────────────┘         │   │
│  │                      ▼                         │   │
│  │            ┌─────────────────┐                 │   │
│  │            │ reportStore     │  Pinia          │   │
│  │            │ fetchReport()   │                 │   │
│  │            └───────┬─────────┘                 │   │
│  │                    │                            │   │
│  │            ┌───────▼─────────┐                 │   │
│  │            │ api/web.ts      │  WASM loader    │   │
│  │            │ load + call     │                 │   │
│  │            └───────┬─────────┘                 │   │
│  └────────────────────┼───────────────────────────┘   │
│                       │                                │
│  ┌────────────────────▼───────────────────────────┐   │
│  │  WASM Module (trending-agent-wasm)              │   │
│  │  run_report() → serde_json::to_string(&Report)  │   │
│  │  ┌──────────────────────────────────────────┐  │   │
│  │  │  crates/core (browser fetch via reqwest)  │  │   │
│  │  │  fetch_limit_up() ──► flash-api.xuangubao │  │   │
│  │  │  fetch_overview() ──► flash-api.xuangubao │  │   │
│  │  │  fetch_plates()  ──► flash-api.xuangubao  │  │   │
│  │  │  fetch_sentiment()──► jiuyangongshe       │  │   │
│  │  │  fetch_taoguba()  ──► taoguba.com         │  │   │
│  │  └──────────────────────────────────────────┘  │   │
│  └────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────┘
```

无 Vue↔Server HTTP 请求。WASM 在浏览器内通过 `reqwest`（底层是 `fetch`）直接调用外部数据源。

**Phase A 实际数据覆盖范围：**
- 选股宝 3 个端点 → WASM 实时拉取
- 其余数据源 → fallback 策略（见第 4 节）

架构图显示的是**最终态**，Phase A 只实现粗线部分。

---

## 3. WASM 层设计 (`crates/wasm`)

### 3.1 对外暴露的函数

```rust
// crates/wasm/src/lib.rs

#[wasm_bindgen]
pub async fn run_report() -> Result<JsValue, JsValue> {
    // 1. 并行拉取所有数据源
    // 2. 运行 AI 分析（如果 API key 可用）
    // 3. 聚合为 Report
    // 4. 序列化为 JSON 返回
}

#[wasm_bindgen]
pub async fn fetch_overview() -> Result<JsValue, JsValue> {
    // 单独拉取市场概览（用于轻量刷新）
}
```

### 3.2 实现策略

`run_report()` 调用 core 中已有的函数：

```
run_report() {
    // 并行拉取（tokio::join!）
    overview    ← XuanguBaoClient::fetch_overview()
    limit_up    ← XuanguBaoClient::fetch_limit_up()
    plates      ← XuanguBaoClient::fetch_plate_abnormal()
    field_items ← JiuYanSession::fetch_field_items()    // 需要 auth
    timeline    ← JiuYanSession::fetch_timeline()        // 需要 auth
    sentiment   ← SentimentAgent::analyze()               // 需要 LLM
    analyses    ← MainAgent::analyze()                    // 需要 LLM
    tgb_users   ← TgbClient::fetch_users()               // 需要 scrape

    // 聚合
    Report::new(overview, limit_up, plates, ...)

    // 返回 JSON
    serde_json::to_string(&report)
}
```

### 3.3 Cargo.toml 调整

```toml
# crates/wasm/Cargo.toml
[dependencies]
trending-agent-core = { path = "../core" }
wasm-bindgen = "0.2"
wasm-bindgen-futures = "0.4"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde-wasm-bindgen = "0.6"    # 高效 Rust→JS 序列化（可选）
```

### 3.4 reqwest WASM 兼容

workspace 级别的 `reqwest` 需要添加 `rustls-tls` feature 以支持 WASM：

```toml
# 根 Cargo.toml
[workspace.dependencies]
reqwest = { version = "0.12", features = ["json", "cookies", "stream", "rustls-tls"] }
```

`reqwest` 在 WASM target 下自动使用浏览器 `fetch` API，无需 native TLS。

---

## 4. 数据源分类与 CORS 策略

| 数据源 | 端点 | 需要 Auth | CORS 风险 |
|--------|------|-----------|-----------|
| 选股宝 市场概览 | `flash-api.xuangubao.cn/api/overview` | 否 | 低（公开 API） |
| 选股宝 涨停列表 | `flash-api.xuangubao.cn/api/limit_up` | 否 | 低 |
| 选股宝 板块异动 | `flash-api.xuangubao.cn/api/plate_abnormal` | 否 | 低 |
| 韭研公社 异动 | `app.jiuyangongshe.com/.../action/field` | 是（手机登录） | 高 |
| 韭研公社 文章 | `app.jiuyangongshe.com/.../community` | 是 | 高 |
| 淘股吧 用户帖 | `taoguba.com.cn` | 否（web scrape） | 高 |
| LLM 分析 | `openrouter.ai` | 是（API Key） | 未知 |

### 4.1 分阶段策略

**Phase A（本次实现）：** 聚焦选股宝公开数据
- `overview`、`limit_up`、`plate_abnormal` → WASM 直接请求选股宝 API
- 韭研公社 field_items/timeline → WASM 请求（auth 可能失败）→ fallback 空数组
- 韭研公社 sentiment → `fetch('/data/sentiment.json')` 同源加载（Phase B cron 定时生成）
- 淘股吧 → `fetch('/data/taoguba.json')` 同源加载（Phase B cron 定时生成）
- LLM 分析 → 检测 `window.__TRENDING_AGENT_OPENROUTER_KEY__`（通过 Vite `define` 注入），无 key 则跳过并显示 "未配置 AI Key"

**Phase B（后续）：** 定时生成 + 静态加载
- 在自己的服务器上用 cron 定时运行 `cargo run -- run --format json` 拉取全量数据
- 输出为静态 JSON 文件，放到 `dist/data/` 目录
- Vue 用 `fetch('/data/report.json')` 同源加载
- 整体部署到 Vercel/Cloudflare，不引入任何后端服务器

当前实现以 **Phase A** 为目标。

---

## 5. 前端 Store 改造 (`src/stores/report.ts`)

### 5.1 TypeScript 类型对齐

Rust struct 字段是 `snake_case`，`serde_json` 序列化后保持原名。TS 接口需匹配：

```typescript
// 删除现有手动 interface，改为匹配 Rust serde 输出

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

interface Stock {
  symbol: string
  stock_chi_name: string    // 注意：不是 stock_name
  price: number
  change_percent: number
  limit_up_days: number
  break_limit_up_times: number
  turnover_ratio: number     // 注意：不是 turnover
  total_capital: number      // 注意：不是 capital
  surge_reason: SurgeReason | null
}

interface MarketOverview {
  rise_count: number
  fall_count: number
  limit_up_count: number
  limit_down_count: number
  bomb_rate: number          // 注意：bomb_rate 是 f64，不是 bomb_rate 百分比
}
```

### 5.2 fetchReport 改造

```typescript
async function fetchReport() {
  loading.value = true
  try {
    const wasm = await loadWasm()           // 动态 import WASM
    const jsonStr = await wasm.run_report() // 返回 JSON 字符串
    report.value = JSON.parse(jsonStr) as Report
  } catch (error) {
    console.error('Failed to fetch report:', error)
    error.value = String(error)
  } finally {
    loading.value = false
  }
}
```

### 5.3 组件适配

各 Tab 组件中的字段名需要和 Rust struct 对齐：

| 组件 | 当前字段（错误） | 正确字段 |
|------|-----------------|---------|
| KpiCards | `overview.rise_count` | `overview.limit_up_count`（涨停家数） |
| KpiCards | `overview.fall_count` | `overview.limit_down_count`（跌停家数） |
| KpiCards | `overview.bomb_rate` | `overview.bomb_rate`（✅ 正确，但值是 0-1 小数，显示需×100） |
| TabLimitUp | `stock.stock_name` | `stock.stock_chi_name` |
| TabLimitUp | `stock.turnover_ratio` | ✅ 正确 |
| TabLimitUp | `plate_name` from `plate_abnormal` | ❌ 涨停榜不应读 `plate_abnormal`，应读涨停股 `surge_reason.related_plates[].plate_name` |
| TabPlate | `event.description` | `event.plate_abnormal_event_data?.plate_name` + `pcp`（涨跌幅） |
| TabPlate | `event.related_stocks` | `event.plate_abnormal_event_data?.related_stocks[].name` + `symbol` |
| TabSentiment | `article.summary` | `article.content`（文章内容） 或 `article.title`（标题） |
| TabSentiment | `article.author` | `article.user.nickname` |
| TabSentiment | `article.heat` | `article.like_count` + `article.comment_count` 组合热度 |

---

## 6. Vite + WASM 构建管线

### 6.1 依赖安装

```bash
cd src
npm install vite-plugin-wasm vite-plugin-top-level-await
```

### 6.2 vite.config.ts 更新

```typescript
import wasm from 'vite-plugin-wasm'
import topLevelAwait from 'vite-plugin-top-level-await'

export default {
  plugins: [
    wasm(),
    topLevelAwait(),
    require('@vitejs/plugin-vue')(),
  ],
}
```

### 6.3 WASM 构建脚本

在 `src/package.json` 添加：

```json
{
  "scripts": {
    "wasm:build": "cd ../crates/wasm && wasm-pack build --target web --out-dir ../../src/src/wasm",
    "dev": "npm run wasm:build && vite",
    "build": "npm run wasm:build && vue-tsc -b && vite build"
  }
}
```

WASM 输出到 `src/src/wasm/`，`api/web.ts` 通过 `import('../wasm/trending_agent_wasm')` 加载。

---

## 7. 错误处理

| 场景 | WASM 行为 | Vue 行为 |
|------|-----------|----------|
| 选股宝 API 超时 | `Err(JsValue::from_str("timeout"))` | 显示 Toast "数据获取超时" |
| 选股宝 API 返回非200 | `Err(JsValue::from_str(...))` | 显示 Toast "数据源异常" |
| LLM key 未配置 | 跳过 analyses，其余数据正常 | AI 观点 Tab 显示 "未配置 AI" |
| WASM 加载失败 | JS import 报错 | 显示 "WASM 加载失败，请刷新" |
| JSON 解析失败 | N/A | catch 块显示错误信息 |

`crates/wasm` 中不 panic，全部用 `anyhow::Result` → `JsValue` 转换。

### 7.1 API Key 注入

WASM 在浏览器中运行，无 `.env` 文件。LLM API Key 通过以下方式传入：

```typescript
// vite.config.ts
export default defineConfig({
  define: {
    __OPENROUTER_KEY__: JSON.stringify(process.env.OPENROUTER_API_KEY || ''),
  },
})
```

```rust
// crates/wasm/src/lib.rs
#[wasm_bindgen]
pub fn set_openrouter_key(key: &str) {
    // 存储到 thread_local 或全局 OnceCell
}
```

Vue 启动时从 `import.meta.env` 读取 key 并调用 `wasm.set_openrouter_key(key)`。

---

## 8. 文件变更清单

```
修改：
  crates/wasm/Cargo.toml          # 添加 serde_json, serde-wasm-bindgen
  crates/wasm/src/lib.rs          # 实现 run_report()
  Cargo.toml                      # reqwest 加 rustls-tls
  src/vite.config.ts              # 加 wasm plugin
  src/package.json                # 加 wasm:build 脚本
  src/api/web.ts                  # 完善 WASM 加载逻辑
  src/stores/report.ts            # 删除 mock，调 WASM，对齐类型
  src/src/components/KpiCards.vue # 字段名修正
  src/src/components/tabs/*.vue   # 字段名修正

新增：
  （无新文件，全在现有文件中改动）
```

---

## 9. 测试验证

1. `cd crates/wasm && wasm-pack build --target web` — WASM 编译成功
2. `cd src && npm run dev` — Vite 启动无报错
3. 浏览器打开 → KPI 卡片显示真实选股宝数据（非 mock 的 120/30）
4. 涨停榜 Tab → 显示当天涨停股票列表（非贵州茅台）
5. 板块异动 Tab → 显示真实板块异动事件
6. 控制台无 CORS 报错（选股宝公开 API）
7. 刷新按钮 → 重新拉取最新数据

---

## 10. 风险与缓解

| 风险 | 影响 | 缓解 |
|------|------|------|
| 选股宝 API 有 CORS 限制 | 选股宝数据无法获取 | Vite dev 时加 proxy 转发 `/api/xuangubao` → `flash-api.xuangubao.cn`；生产环境 WASM 直接请求 |
| reqwest WASM 行为差异 | cookie/session 不可用 | 韭研公社等 auth 数据源走 Phase B cron 静态文件方案 |
| WASM 体积过大 | 首次加载慢 | `wasm-opt` 压缩，release 构建 |
| tokio 在 WASM 中的兼容性 | runtime 报错 | `wasm-bindgen-futures` 单线程 runtime 即可 |
