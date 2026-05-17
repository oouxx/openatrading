# 角色策略页面

## 目标

Vue 前端新增角色策略展示页面，以阅读为主。展示每个角色的完整策略、持仓盈亏、最近决策理由。

## 导航

全局 header 添加「角色策略」链接，`vue-router` 路由 `/roles`。

## 后端 API

在 `crates/core/src/web/handlers.rs` 新增 3 个 handler：

### `GET /api/roles`

返回所有角色及其完整策略文本（base.md + supplement.md 合并后）。

```json
[
  {
    "name": "zarili",
    "display_name": "zarili量价派",
    "strategy": "核心信仰：一切围绕量能。量价关系是一切判断的根基..."
  }
]
```

实现：`AppConfig::load(project_root)` → 遍历 `roles` → `AppConfig::load_strategy(entry)` → JSON

### `GET /api/roles/:name/positions`

返回该角色持仓列表，含盈亏计算所需原始数据。

```json
[
  {
    "id": 1,
    "symbol": "000001",
    "stock_name": "平安银行",
    "cost_price": 12.5,
    "current_price": 14.2,
    "shares": 1000,
    "entry_reason": "量能放大突破5日线",
    "entry_date": "2026-05-14"
  }
]
```

实现：`trade.list_positions(name)` → 序列化返回。盈亏由前端计算。

### `GET /api/roles/:name/logs`

返回该角色最近交易记录（从 trade_logs 表查询，含开仓和平仓）。

```json
[
  {
    "date": "2026-05-16",
    "action": "买入",
    "symbol": "000001",
    "price": 12.5,
    "shares": 1000,
    "reason": "量能放大突破5日线"
  }
]
```

实现：通过 `trade` 模块查询 trade_logs 表，按 role_name 过滤，按时间倒序，取最近 20 条。

## 注册路由

在 `crates/core/src/web/mod.rs` 的 `build_router()` 中新增 3 条路由：

```rust
.route("/api/roles", get(handlers::get_roles))
.route("/api/roles/:name/positions", get(handlers::get_role_positions))
.route("/api/roles/:name/logs", get(handlers::get_role_logs))
```

## 前端

### 路由

`src/src/router.ts` 新增：

```ts
import RoleView from './RoleView.vue'

{ path: '/roles', component: RoleView }
```

### 导航

`src/src/App.vue` header 加：

```html
<router-link to="/roles">角色策略</router-link>
```

### Store

`src/src/stores/data.ts` 新增：

```ts
export interface RoleEntry {
  name: string
  display_name: string
  strategy: string
}

export interface RolePosition {
  id: number
  symbol: string
  stock_name: string
  cost_price: number
  current_price: number
  shares: number
  entry_reason: string
  entry_date: string
}

export interface RoleLog {
  date: string
  action: string
  symbol: string
  price: number
  shares: number
  reason: string
}

export const roles = reactive({ data: [] as RoleEntry[], loading: false, error: '' })
export const rolePositions = reactive<Record<string, { data: RolePosition[], loading: boolean }>>({})
export const roleLogs = reactive<Record<string, { data: RoleLog[], loading: boolean }>>({})

export async function fetchRoles()
export async function fetchRolePositions(name: string)
export async function fetchRoleLogs(name: string)
```

### 页面: `RoleView.vue`

纵向全宽卡片列表。每张卡片：

- **头部**：角色名 + 总盈亏（红涨绿跌，前端 `current_price * shares - cost_price * shares`）
- **策略区**：默认折叠（显示前 3 行），点击展开/收起全文
- **持仓区**：表格（symbol、股票名、盈亏%、成本/现价、shares、建仓理由）
- **交易记录**：最近 5 条 trade_logs（日期 + 动作 + symbol + 价格 + 理由）

### 颜色

沿用现有设计系统：涨红 `#FF5C5C`、跌绿 `#1D9E75`、AI 紫 `#7F77DD`、深色背景。
