# K-Line Chart Design

**Date:** 2026-05-05
**Status:** Approved

## 1. Overview

为个股添加 K 线图展示能力。用户点击任意表格中的股票代码，右侧滑出抽屉面板显示交互式 K 线图。

**技术选型:** TradingView lightweight-charts（~40KB），Vue 组件无路由依赖。

## 2. Core Crate — 新增 K-line API

### 2.1 `XuanGuBaoClient::fetch_kline`

```rust
pub struct KlineParams {
    pub symbol: String,       // "000609.SZ"
    pub period: KlinePeriod,  // Day/Week/Month/Minute
    pub count: u32,           // 默认 256
}

pub enum KlinePeriod {
    Day,    // 86400
    Week,   // 604800
    Month,  // 2592000
    Minute, // 60
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KlineCandle {
    pub time: i64,         // unix timestamp
    pub open: f64,
    pub close: f64,
    pub high: f64,
    pub low: f64,
    pub volume: f64,
    pub change_rate: f64,  // %
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KlineData {
    pub symbol: String,
    pub period: String,
    pub candles: Vec<KlineCandle>,
}
```

API 端点: `https://api-ddc-wscn.xuangubao.com.cn/market/kline`
参数: `prod_code={symbol}&period_type={seconds}&tick_count={count}&adjust_price_type=forward&fields=tick_at,open_px,close_px,high_px,low_px,turnover_volume,px_change_rate`

### 2.2 字段映射

API 返回 `lines` 数组，每元素按 `fields` 参数顺序：
- `[0]` = tick_at → KlineCandle.time
- `[1]` = open_px → KlineCandle.open
- `[2]` = close_px → KlineCandle.close
- `[3]` = high_px → KlineCandle.high
- `[4]` = low_px → KlineCandle.low
- `[5]` = turnover_volume → KlineCandle.volume
- `[6]` = px_change_rate → KlineCandle.change_rate

## 3. Web Crate — 新增端点

### 3.1 `GET /api/kline/:symbol`

| Param | 默认 | 说明 |
|-------|------|------|
| `period` | `day` | day/week/month/minute |
| `count` | `256` | 数据条数 |

示例: `GET /api/kline/000609.SZ?period=day&count=128`

## 4. Frontend

### 4.1 点击入口

`TabLimitUp.vue` 和 `TabPlate.vue` 中股票代码/名称添加 `@click` 事件：

```vue
<span @click="openKline(stock.symbol, stock.stock_chi_name)"
      class="cursor-pointer hover:underline"
      style="color: var(--blue)">
  {{ stock.symbol }}
</span>
```

### 4.2 KlineDrawer.vue

- 从右侧滑入，宽度 ~480px，覆盖在主内容上
- 头部: 股票代码 + 名称 + 关闭按钮(×)
- 周期切换: [1分] [日K] [周K] [月K] 四个按钮
- 图表区: lightweight-charts `createChart` + `addCandlestickSeries`
- 数据获取: 目前前端按数据源策略（api 调后端，wasm 直调，json 模式下不可用显示提示）

### 4.3 App.vue 集成

```vue
<KlineDrawer v-if="klineSymbol" :symbol="klineSymbol" :name="klineName" @close="klineSymbol=''" />
```

provide/inject `openKline(symbol, name)` 到所有子组件。

### 4.4 静态/JSON 模式

JSON 模式下 K 线数据不可实时获取，抽屉内显示"静态模式不支持实时 K 线，请使用 API 模式"。

## 5. 依赖

- `src/package.json`: 新增 `lightweight-charts`
- `crates/core/Cargo.toml`: 不变（XuanGuBaoClient 未依赖额外 crate）
- `crates/web/Cargo.toml`: 不变

## 6. 文件清单

| 文件 | 变更 |
|------|------|
| `crates/core/src/api/xuangubao.rs` | 新增 `KlineParams`, `KlinePeriod`, `KlineCandle`, `KlineData`, `fetch_kline()` |
| `crates/web/src/main.rs` | 新增 `GET /api/kline/:symbol` 路由 |
| `src/package.json` | 新增 `lightweight-charts` |
| `src/src/components/KlineDrawer.vue` | 新建 — K 线抽屉组件 |
| `src/src/App.vue` | 挂载 KlineDrawer，provide kline open/close |
| `src/src/components/tabs/TabLimitUp.vue` | 股票点击 → kline emit |
| `src/src/components/tabs/TabPlate.vue` | 股票点击 → kline emit |
| `src/vite.config.ts` | proxy 新增 `/api/kline` → K 线 API |
