# 涨停晋级面板 Design Spec

## Goal

新增一个「晋级」Tab，按连板梯队展示昨日涨停股→今日晋级/断板情况，包含晋级率概览和明细列表。

## Data Source

- **今日数据**: 当前 `store.report.limit_up`（已有）
- **昨日数据**: `GET /api/report/live?date=<prev_trading_date>`
- **交易日推算**: 前端遍历 date-1 至 date-5，找到第一个有数据的日期作为前一交易日

## Data Merging

按 `symbol` 匹配今日和昨日的 `limit_up` 列表，每条记录包含：

| 字段 | 来源 |
|------|------|
| symbol, name | limit_up |
| 今板数 (today_days) | today.limit_up_days |
| 昨板数 (yesterday_days) | yesterday.limit_up_days |
| 今炸板 (today_bomb) | today.break_limit_up_times |
| 昨炸板 (yesterday_bomb) | yesterday.break_limit_up_times |
| change_percent, turnover_ratio, total_capital | today |

分类：
- `yesterday_days > 0 && today_days > 0` → **晋级成功**
- `yesterday_days == 0` → **首板新进**
- `today_days == 0`（昨天列表有、今天没有）→ **断板**

## UI Layout

新增 `TabPromotion.vue`，分两部分：

### 上半部分：连板梯队晋级率

每条表示一个晋级路径（昨X板→今Y板），格式：

```
昨2板(15只) → 今3板(8只)  晋级率 53%  ████████░░░░░░░░
```

- 按昨板数分组、降序排列
- 晋级率 bar 使用进度条样式
- 行可点击展开，显示该板数下的明细
- 汇总行：「合计 昨N只 → 今M只 综合晋级率 X%」

### 下半部分：明细列表（展开后）

三个子表，Tab 切换：

1. **晋级成功**：股票列表，列：symbol、名称、昨板→今板、换手率、炸板次数
2. **首板新进**：股票列表，列：symbol、名称、今板数、换手率、成交额
3. **断板**：股票列表，列：symbol、名称、昨板数、今状态（未涨停）、昨换手率

点击股票 symbol 复用现有的 K 线抽屉。

## Files

| 文件 | 操作 |
|------|------|
| `src/src/components/tabs/TabPromotion.vue` | 新建 |
| `src/src/App.vue` | 修改 — 注册新 Tab |
| `src/src/components/TabBar.vue` | 修改 — 添加「晋级」按钮 |

## Edge Cases

- 前一交易日无数据（节假日/接口故障）→ 显示「昨日数据不可用」
- 今日数据为空 → 显示「今日无涨停数据」
- 某板数晋级率为 0%（全部断板）→ 正常显示 0%，红色高亮

## No New Dependencies

纯数据对比展示，不需要新库。
