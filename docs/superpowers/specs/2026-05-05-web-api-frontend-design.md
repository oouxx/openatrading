# Web Crate REST API + Frontend Multi-Source Design

**Date:** 2026-05-05
**Status:** Approved

## 1. Overview

改造 `crates/web` 提供 JSON REST API，前端通过环境变量 `VITE_DATA_SOURCE` 在三种数据源（API / JSON / WASM）间切换。

## 2. Web Crate REST API

### 2.1 端点

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` | `/api/report` | 返回缓存 Report JSON，若缓存为空则先拉取 |
| `GET` | `/api/report?section=limit-up,overview` | 返回指定截面 JSON |
| `GET` | `/api/report/live` | 强制重新拉取数据，返回最新 JSON，更新缓存 |
| `GET` | `/` | 保留旧 HTML SPA |
| `GET` | `/report?format=markdown\|html` | 保留旧渲染端点 |

### 2.2 启动行为

web crate `main.rs` 启动时自动调用 `trending_agent_core` 拉取全量数据到 `WebState`。不依赖 CLI 前置注入。

```rust
// main.rs 伪代码
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let state = WebState::new();
    // 启动时拉取初始数据
    refresh_report(&state).await;
    start_web_server(8080, state).await;
    Ok(())
}
```

### 2.3 Section 过滤

`?section=` 参数接受逗号分隔的字段名，对应 `Report` 结构体的顶层字段：

| section 值 | 返回内容 |
|-----------|---------|
| `overview` | 市场概览 |
| `limit-up` | 涨停数据 |
| `plate-abnormal` | 板块异动 |
| `field-items` | 韭研异动 |
| `timeline` | 时间线 |
| `sentiment` | 舆情分析 |
| `analyses` | AI 角色分析 |
| `summary` | risk_level + opportunity_signals + decision_summary |

不传 section 则返回完整 Report。

### 2.4 依赖变更

`crates/web/Cargo.toml` 新增：
- `serde_json` — JSON 序列化
- `serde` — 与 core 的 Serialize 一致

## 3. 前端多数据源

### 3.1 环境变量控制

`vite.config.ts` 读取 `VITE_DATA_SOURCE`：

```ts
define: {
  __DATA_SOURCE__: JSON.stringify(rootEnv.VITE_DATA_SOURCE || 'api'),
}
```

| `VITE_DATA_SOURCE` | 行为 |
|-------------------|------|
| `api` | 请求 `http://localhost:8080/api/report`（开发）/ 同源 `/api/report`（生产） |
| `json` | 加载 `public/report.json` 静态文件 |
| `wasm` | 浏览器 WASM 直接拉取（走 Vite proxy） |

### 3.2 Store 改造

`stores/report.ts` 改为单入口分发：

```ts
async function fetchReport() {
  store.loading = true
  store.error = ''
  try {
    switch (__DATA_SOURCE__) {
      case 'api':  store.report = await fetchFromApi(); break
      case 'json': store.report = await fetchFromJson(); break
      case 'wasm': store.report = await fetchFromWasm(); break
    }
  } catch (e: any) {
    store.error = e.message || String(e)
  } finally {
    store.loading = false
  }
}
```

### 3.3 刷新策略

- 页面加载时调一次 `fetchReport()`
- 用户点"刷新"按钮调一次
- API 模式下切换 tab 不重复请求（数据已在缓存）

### 3.4 开发 vs 生产

- 开发：`api` 模式连 `localhost:8080`，前端 `npm run dev` 用 Vite proxy 转发 `/api` 到 web 服务
- 生产：`api` 模式下前端和 web crate 同源部署，无需 proxy

## 4. 兼容性

- 旧 HTML 端点 `/` 和 `/report` 保持不变，不影响 Vercel 静态部署
- CLI `--web` 模式继续工作（注入 Report 进 WebState）
- WASM 和 static JSON 模式不受影响

## 5. 数据拉取函数

web crate 不依赖 CLI，从 `crates/core` 直接调用 API client 拉取数据：

```rust
// crates/web/src/main.rs
async fn refresh_report(state: &WebState) -> anyhow::Result<()> {
    let client = trending_agent_core::api::Client::new();
    // 并行拉取（复用 core 的 tokio::join! 模式）
    let (overview, limit_up, limit_down, plate_abnormal) = tokio::join!(
        client.xuangubao.fetch_market_overview(),
        client.xuangubao.fetch_limit_up(),
        client.xuangubao.fetch_limit_down(),
        client.xuangubao.fetch_plate_abnormal(30),
    );
    let report = Report::new(
        overview?, limit_up?, vec![], vec![], vec![], vec![], None, vec![],
        XuanGubaoExtra { limit_down: limit_down?, plate_abnormal: plate_abnormal? },
    );
    state.set_report(report).await;
    Ok(())
}
```

Jiuyan 数据（需要登录）和 AI 分析（需要 API key）为可选——有 env 配了就拉，没有就跳过。

## 6. 文件清单

| 文件 | 变更 |
|------|------|
| `crates/web/src/main.rs` | 新增 `/api/report`、`/api/report/live` 端点；启动时自动拉数据 |
| `crates/web/Cargo.toml` | 新增 `serde_json`, `serde` |
| `src/vite.config.ts` | 新增 `VITE_DATA_SOURCE` env 变量、`/api` proxy |
| `src/src/stores/report.ts` | 改为三源分发，移除优先级回退逻辑 |
| `src/src/env.d.ts` | 新增 `__DATA_SOURCE__` 类型声明 |
