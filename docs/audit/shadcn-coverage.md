# Shadcn 组件覆盖审计

> 盘点项目中手写的 UI 元素，哪些可以用已有 shadcn 组件替换。
>
> 生成时间：2026-07-02
> 可用组件列表：`avatar`, `button`, `card`, `checkbox`, `collapsible`, `command`, `dialog`, `input-group`, `input`, `popover`, `reasoning`, `scroll-area`, `select`, `separator`, `sheet`, `sidebar`, `skeleton`, `textarea`, `tooltip`

---

## 🔴 优先（单个文件 1-4 处，改动小收益明显）

| 文件 | 行号 | 手写内容 | 应替换为 |
|------|------|----------|----------|
| `AiCopilot.tsx` | 443, 464 | 股票搜索 `<input>` + 手写下拉列表 | `Command` + `CommandInput` + `CommandList` + `CommandItem` |
| `NewsFeed.tsx` | 113, 180 | `<select>` 源筛选器 | `Select` `SelectTrigger` `SelectContent` `SelectItem` |
| `BreakoutView.tsx` | 458 | `<input type="checkbox">` | `Checkbox` |
| `PromotionTab.tsx` | 218 | `<input type="checkbox">` | `Checkbox` |
| `ChatQuickButtons.tsx` | 8, 14, 20, 26 | 4 个裸 `<button>` | `Button variant="ghost" size="sm"` |
| `ChromeTabs.tsx` | 33 | 关闭 tab 的 × 按钮 | `Button variant="ghost" size="icon"` |
| `TradingView.tsx` | 44, 117 | 重试 `<button>` | `Button` |
| `RightPanel.tsx` | 110, 128 | Tab 按钮 | `Button variant="ghost"` |
| `WatchlistView.tsx` | 266-284, 379 | 排序列头 + 删除按钮（已导入 `Button` 但没用） | `Button variant="ghost" size="sm"` |

---

## 🟡 中等（每个文件 4-18 处，含弹窗重构）

### `RoleView.tsx` — 重灾区

| 分类 | 数量 | 手写内容 | 替换方案 |
|------|------|----------|----------|
| 裸 button | ~18 处 | 创建/删除/取消/Tab 切换/侧边栏 | `Button`（variant / size） |
| 手写弹窗 | 3 个 | Create、DeleteConfirm、ResetConfirm (`fixed inset-0 z-50`) | `Dialog` ＋ `Button` |
| 裸 input | 2 处 | 搜索框 (73)、名称输入 (725) | `Input` |
| 裸 textarea | 1 处 | Markdown 编辑器 (516) | `Textarea` |

### `BreakoutView.tsx`

| 分类 | 数量 | 手写内容 | 替换方案 |
|------|------|----------|----------|
| 裸 button | 3 处 | 筛选、关闭 | `Button` |
| 手写 Drawer | 含 backdrop + motion.div (605-659) | `Sheet side="right"` |

### `DragonTigerView.tsx`

| 分类 | 数量 | 手写内容 | 替换方案 |
|------|------|----------|----------|
| 裸 button | 5 处 | 分页、筛选、弹窗关闭 | `Button` |
| 手写弹窗 | 1 个 (337-356) | `fixed inset-0 z-50` | `Dialog` |
| 裸 input | 1 处 (171) | 搜索输入 | `Input` |

### `ReplayView.tsx`

| 分类 | 数量 | 手写内容 | 替换方案 |
|------|------|----------|----------|
| 裸 button | 9 处 | play/pause/prev/next/speed 控制 | `Button variant="ghost"` |

### `KlineDrawer.tsx`

| 分类 | 数量 | 手写内容 | 替换方案 |
|------|------|----------|----------|
| 裸 button | 3 处 | 周期选择、关闭 | `Button` |
| 手写弹窗 | 1 个 (280-306) | 详情弹窗 | `Dialog` |

### `MiniChartPreview.tsx`

| 分类 | 数量 | 手写内容 | 替换方案 |
|------|------|----------|----------|
| hover 弹窗 | 1 个 (84-106) | 绝对定位 popover | `Popover` |
| 放大弹窗 | 1 个 (108-132) | `fixed inset-0 z-[70]` | `Dialog` |

### `SettingsView.tsx`

| 分类 | 数量 | 手写内容 | 替换方案 |
|------|------|----------|----------|
| 裸 button | 3 处 (62, 220, 233) | 主题切换 + MCP 启用/删除 | `Button variant="ghost"` |

---

## ⚪ 低优（改动小，但影响不大）

| 文件 | 行号 | 内容 | 建议 |
|------|------|------|------|
| `LandingPage.tsx` | 313 | Copy 按钮 | 1 处，可顺手用 `Button` |
| `MarketIntelTab.tsx` | 53 | 筛选 tab | 1 处 |
| `TimelineTab.tsx` | 55 | 视图切换 | 1 处 |
| `RoleView.tsx` | — | 零散 `<label>` | 17 处，但无对应 shadcn 组件 |
| `RoleView.tsx` | 240-274 | 手写 SVG PnL 图 | 业务可视化，不在 UI 层替换 |

---

## ❌ 不推荐替换

| 类型 | 原因 |
|------|------|
| `data-table` 手写表格（style.css 203-228） | 金融数据表格有排序/点击行/迷你图等定制需求；项目中无 `Table` 组件，引入新组件不划算 |
| `<label>` 元素 | 项目中无 shadcn `Label` 组件 |
| SVG 图表（RoleView PnL） | 纯业务可视化，不是 UI 组件问题 |
| 已使用 `InputGroup` 的文件（SkillEditor） | 本身就是 shadcn 对齐的做法 |

---

## 按文件维度汇总

| 文件 | 严重度 | 主要问题 | 建议优先级 |
|------|--------|----------|-----------|
| `RoleView.tsx` | 🔴 | 18 button + 3 弹窗 + 2 input + 1 textarea | **最高** |
| `BreakoutView.tsx` | 🟡 | Drawer + button + checkbox | 高 |
| `DragonTigerView.tsx` | 🟡 | 弹窗 + button + input | 中 |
| `AiCopilot.tsx` | 🟡 | 搜索下拉 + stock search button | 中 |
| `ReplayView.tsx` | 🟡 | 9 个控制按钮 | 中 |
| `KlineDrawer.tsx` | 🟡 | button + 弹窗 | 中 |
| `MiniChartPreview.tsx` | 🟡 | popover + dialog | 中 |
| `NewsFeed.tsx` | 🟢 | select 替换 | 低 |
| `WatchlistView.tsx` | 🟢 | 排序列头 + 删除（已导入 Button） | 低 |
| `SettingsView.tsx` | 🟢 | 3 处 button | 低 |
| `ChatQuickButtons.tsx` | 🟢 | 4 处 button | 低 |
| `ChromeTabs.tsx` | 🟢 | × 关闭按钮 | 低 |
| `TradingView.tsx` | 🟢 | 2 处 retry button | 低 |
| `RightPanel.tsx` | 🟢 | tab 按钮 | 低 |
| `PromotionTab.tsx` | 🟢 | checkbox | 低 |
