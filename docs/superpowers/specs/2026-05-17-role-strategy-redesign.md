# Role Strategy Page Redesign

## Summary

Redesign `/roles` page from flat card list to split-pane layout: role list (left) + role detail (right). Add role CRUD, account reset, and profit curve computed from existing trade logs — no new database tables.

## Layout

```
┌──────────────────────────────────────────────────────┐
│  [App Navbar]                                        │
├────────────┬─────────────────────────────────────────┤
│  Role List │  Role Detail                            │
│  (320px)   │                                         │
│            │  [Header: name, P&L, edit/reset/del]    │
│  Mini Card │  ───────────────────────────────────    │
│  +sparkline│  [Profit Curve: lightweight-charts]     │
│            │  [Positions Table]                      │
│  [+ New]   │  [Strategy: collapsible Markdown]       │
└────────────┴─────────────────────────────────────────┘
```

### Left: Role List (320px)

- Mini cards stacked vertically
- Selected state: left purple border + `bg-ai/10`
- Each card: colored dot + display name + P&L% + position count + sparkline
- Sparkline: 30-day cumulative return trend (Canvas mini chart)
- Empty position shows "空仓"
- Bottom: "+ 新增角色" entry → inline form (name, display_name, initial strategy)

### Right: Role Detail

**Action bar:** display name, cumulative P&L%, position count, initial capital ¥1,000,000. Buttons: [编辑策略] [重置账户] [删除角色].

**Profit curve** (lightweight-charts, Area + Line series):
- 累计收益率% (left axis) — red/green
- 持仓市值 (right axis) — AI purple
- 现金 (right axis) — gray dashed
- Time ranges: 7d / 30d / all
- Data source: real-time compute from trade logs. Start from 100万 initial capital, replay Open/Close transactions, mark-to-market open positions at current price. No new DB table.

**Positions table:** 代码 | 名称 | 盈亏% | 成本/现价 | 股数 | 持仓天数 | 理由. Red-up green-down. tabular-nums. New "持仓天数" column from entry_date.

**Strategy section:**
- View mode: collapsed by default, expand to render Markdown (headings, lists, bold), max-height 400px scrollable
- Edit mode (click [编辑策略]): textarea for Markdown editing, [取消] [保存]

## New API Endpoints

| Method | Path | Purpose |
|--------|------|---------|
| POST | `/api/roles` | Create role (name, display_name, strategy) |
| PUT | `/api/roles/:name/strategy` | Update role strategy text |
| DELETE | `/api/roles/:name` | Delete role + positions + trade logs |
| POST | `/api/roles/:name/reset` | Reset: clear positions/logs, restore 100万 initial state |
| GET | `/api/roles/:name/pnl-history` | Compute P&L time-series from trade logs |

## P&L Computation (no new DB)

Given trade logs with `action` (Open/Close), `price`, `shares`, and `created_at`:
1. Start with 1,000,000 cash, 0 positions
2. Sort all logs by `created_at`
3. Open: subtract cost from cash, add to positions
4. Close: add proceeds to cash, remove from positions, record realized P&L
5. At each log timestamp: mark-to-market open positions → total equity = cash + market_value
6. Return daily equity snapshots for the requested time range

## Reset Account

- Confirm dialog: "确认重置 Zarili量价派 账户？将清空所有持仓和交易记录，恢复初始资金 ¥1,000,000。此操作不可撤销。"
- On confirm: delete all positions and trade_logs for that role. No schema changes needed.

## Delete Role

- Confirm dialog: "确认删除 Zarili量价派？将删除该角色及所有相关持仓和交易记录。此操作不可撤销。"
- On confirm: delete role config + delete positions + delete trade_logs.

## Frontend Changes

### New/Modified Files

| File | Change |
|------|--------|
| `src/src/RoleView.vue` | Complete rewrite: split-pane layout |
| `src/src/stores/data.ts` | Add new API functions + RolePnlPoint type |
| `crates/core/src/web/handlers.rs` | Add 5 handler functions |
| `crates/core/src/web/mod.rs` | Register 5 new routes |
| `crates/core/src/trade/mod.rs` | Add `delete_positions_by_role`, `delete_logs_by_role` |
| `crates/core/src/trade/sqlite.rs` | Implement new methods |
| `crates/core/src/roles/mod.rs` | Add `create_role`, `update_strategy`, `delete_role` |

### Design Tokens (existing)

```
--color-bg: #0d0d1a
--color-surface: #111124
--color-up: #FF5C5C (red = up, A-Share rule)
--color-down: #1D9E75 (green = down)
--color-ai: #7F77DD (purple = AI)
--color-hairline: rgba(255,255,255,0.06)
```

## Implementation Order

1. Backend: TradeManager methods + role management
2. Backend: API handlers + routes
3. Frontend: Data store API functions
4. Frontend: RoleView.vue rewrite
5. Frontend: P&L computation logic + sparkline
6. Integration test
