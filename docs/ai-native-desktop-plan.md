# AI-Native 桌面复盘工作台改造计划

> **本文档状态：** 全景路线图（已被 ADR-0001~0005 收敛）。
> **当前执行切片：** 见 `docs/m1-review-walkthrough-framework.md`（M1：复盘走通，零代码改动）。
> **关联 ADR：**
> - [ADR-0001](adr/0001-ai-role-is-guide-not-analyst.md)：AI 是向导，给路径不给结论
> - [ADR-0002](adr/0002-operation-plan-is-core-artifact.md)：操作计划是核心物件，复盘是其生产过程
> - [ADR-0003](adr/0003-paper-trading-only.md)：产品是模拟盘系统，永不接真实实盘
> - [ADR-0004](adr/0004-l1-match-and-persist.md)：模拟撮合用 L1 最新价，账户跨日持久化
> - [ADR-0005](adr/0005-skill-driven-no-code-intervention.md)：代码只做机制，领域行为全由 skill 驱动

## 1. 背景与动机

### 1.1 现状

OpenATrading 目前是一个 **传统 SPA + 外挂 AI** 的形态：

- 顶部 nav 把功能切成 6 个平级页面（龙虎榜 / 晋级分析 / 行情回放 / 角色策略 / AI 问答 / 首页行情）
- 用户需要 **先知道要去哪个页面**，再手动触发数据查询
- AI 被塞在 `/copilot` 一个孤立路由里（`src/src/AiCopilot.tsx` 仅 18 行，本质是个聊天框）
- `App.tsx:118-127` 的路由表体现了这种"功能驱动"而非"对话驱动"的架构

### 1.2 问题

短线交易者每天的核心问题不是"打开龙虎榜页"，而是：

> 今天谁最强？主线是什么？龙头是谁？情绪如何？明天可能如何演化？

这些问题是 **叙述性的、跨模块的**，传统页面导航无法承载。当前 AI 只能回答文字、看不到桌面状态，用户在 AI 和功能页之间反复横跳。

### 1.3 目标

把 OpenATrading 改造成 **AI-Native 复盘工作台**，形成"复盘 → 计划 → 执行"闭环：

- AI 对话是 **默认入口**，不是某个子页面
- AI 是**复盘向导**，给路径不给结论（ADR-0001）；复盘的终点是产出一个**操作计划**（ADR-0002）
- AI 能 **直接驱动桌面 UI**（"帮我看下今天晋级情况" → 右侧自动渲染晋级分析组件）
- AI 输出从纯文字升级为 **Artifact**（图表、表格、KPI 卡片内联渲染）
- 执行端是**模拟盘**：模拟撮合引擎按操作计划撮合虚拟成交，**永不接真实实盘**（ADR-0003）
- 盘中 **主动推送**语义 = "操作计划的触发条件命中"（ADR-0002），而非"AI 分析结论"

参考标杆：Codex Desktop、Claude Desktop、Cursor Chat —— 对话即工作台。

> **重要收敛：** 本产品的"操盘/下单/执行"一律指模拟盘内的模拟撮合，不指真实券商交易。产品边界明确**不追求通往实盘**（ADR-0003）。

---

## 2. 现有能力盘点

改造前先确认地基，避免重复造轮子：

| 能力 | 现状 | 位置 |
|------|------|------|
| MCP 工具层 | ✅ 20+ 市场分析工具（akshare 数据源 + stdio/http 双传输） | `crates/core/src/llm/mcp_tools.rs` |
| AI Chat UI | ✅ 基于 `@assistant-ui/react`，支持流式 + reasoning | `src/src/components/thread.tsx` |
| Slash 命令 | ✅ 工具/技能/pipeline 三类 | `useAssistantChat.ts:35-50` |
| Role/Agent 系统 | ✅ 多角色策略、cron 调度 | `crates/core/src/roles/` |
| Tauri 2 桌面壳 | ✅ macOS/Linux/Windows | `src-tauri/src/main.rs` |
| Axum 本地网关 | ✅ `/api/ai/chat/stream` SSE | `crates/core/src/` |
| 指数 ticker | ✅ 顶部 IndexTicker | `App.tsx:14-47` |

### 2.1 缺口

1. **布局**：AI 是子页面，不是主舞台；没有 split-view
2. **AI ↔ 桌面联动**：AI 无法触发前端导航/渲染组件
3. **Artifact 渲染**：AI 输出仅 markdown 文本，无图表/表格组件内联
4. **会话持久化**：`useAssistantChat` 用 `useState` 存消息，刷新即丢
5. **MCP 管理 UI**：stdio/http server 配置散落在 `.mcp.json` / `config.toml`，无前端面板
6. **主动推送**：cron 任务产生结果只发 Telegram，不进对话流
7. **操作计划物件**：无操作计划的数据模型/视图/状态机（ADR-0002 待定义）
8. **模拟撮合引擎**：无模拟盘/撮合/虚拟账户（ADR-0003/0004）
9. **实时行情管道**：`fetch_quotes` 是请求-响应式，非盘中实时流（ADR-0004 前置工程）

> **M1 已通（机制层）：** skill 注入 + 全工具 + SSE 流式返回链路已闭环，零代码改动即可跑通复盘对话（见 `m1-review-walkthrough-framework.md` §2）。上述缺口 1-6 属 Phase 1-3 工程，缺口 7-9 属 ADR-0002/0003/0004 待定义项。

---

## 3. 三阶段路线图

> **执行优先级已重排：** 当前只走通 M1（AI-Native 复盘这一步，见 `m1-review-walkthrough-framework.md`）。下列三阶段是 M1 通过后的全景规划，每步单独 grilling 后再启动。

### Phase 1 — 布局改造（1-2 周）

**目标**：AI 对话成为默认首页，右侧面板由 AI 输出驱动；右侧工作区需含"操作计划视图"和"模拟账户视图"作为一等公民（ADR-0002/0003）。

#### 3.1.1 新布局结构

```
┌─────────────────────────────────────────────────────┐
│ Header（指数 ticker + 角色切换 + 主题）              │
├──────────────────┬──────────────────────────────────┤
│                  │                                  │
│  AI 对话         │  动态工作区                       │
│  (左 40%)        │  (右 60%)                        │
│                  │                                  │
│  - 流式消息      │  根据 AI 输出 artifact 渲染：      │
│  - reasoning     │  - 行情图表                       │
│  - slash 命令    │  - 涨停结构表                     │
│  - 快捷指令      │  - KPI 卡片                       │
│  - skill 注入    │  - 龙虎榜                         │
│                  │  - 【操作计划视图】（ADR-0002）    │
│                  │  - 【模拟账户视图】（ADR-0003）    │
│                  │                                  │
└──────────────────┴──────────────────────────────────┘
```

#### 3.1.2 文件改动清单

**新增**：

- `src/src/components/SplitWorkspace.tsx` — 左右分栏容器，可拖拽调整比例
- `src/src/components/ArtifactRenderer.tsx` — 根据 AI 输出的 artifact type 分发到具体组件
- `src/src/components/artifacts/` — 各类 artifact 组件（ChartArtifact / TableArtifact / KpiArtifact）
- `src/src/stores/workspace.ts` — Zustand store，存当前右侧应渲染的 artifact

**修改**：

- `src/src/App.tsx:118-127` —— 路由表重构：
  - `/` → `Workspace`（AI + 动态右侧，桌面端默认）
  - `/dragon-tiger`、`/breakout`、`/replay` 保留但降级为"独立全屏视图"（供 AI 跳转或用户手动打开）
  - 移除 `/copilot` 独立路由，AI 并入主工作区
- `src/src/App.tsx:70-110` —— 顶部 nav 精简：保留行情/龙虎榜/晋级/回放作为"快捷指令"，点击后向 AI 发送预设 prompt
- `src/src/AiCopilot.tsx` —— 内容迁移到 `Workspace` 组件，原文件删除
- `src/src/hooks/useAssistantChat.ts` —— 解析 AI 流式输出中的 artifact 标记，写入 `workspace` store

#### 3.1.3 Artifact 协议

AI 输出在 markdown 中嵌入约定格式的 artifact 块：

````
```artifact:chart
{
  "type": "kline",
  "symbol": "000001.SS",
  "range": "1M"
}
```
````

前端 `ArtifactRenderer` 识别 ` ```artifact:xxx ` 围栏，调用对应组件渲染。后端 prompt 工程配合：在 system prompt 中告诉 AI 可以输出 artifact 块。

#### 3.1.4 验证

- 启动 `bun run dev`（web）或 `cargo tauri dev`（桌面）
- 默认进入 `/` 看到 AI 对话 + 空白工作区
- 输入"看下今天大盘" → 右侧渲染 KPI + 指数图
- 输入"晋级分析" → 右侧渲染 BreakoutView 组件
- 旧的 `/breakout` 直链仍可访问

---

### Phase 2 — 深化 AI 能力 + 模拟撮合引擎（2-3 周）

**目标**：AI 从"能说话"升级为"能操作桌面"；新增模拟撮合引擎跑用户已定的操作计划。

#### 3.2.1 AI 调用桌面 API

前端暴露一组 **桌面能力 RPC**，AI 通过工具调用触发：

- `navigate(view: "breakout" | "dragon-tiger" | "replay" | "plan" | "account")` —— 切换右侧视图
- `render_artifact(type, payload)` —— 直接渲染 artifact，绕过 markdown
- `select_stock(symbol)` —— 在工作区高亮某只股票
- `query_market_snapshot()` —— 读取当前桌面已加载的数据（避免 AI 重复查库）

实现方式：在 `useAssistantChat` 的流式回调里监听特殊的 `tool_call` 事件，前端执行后回传结果。后端 Axum 侧在 `chat/stream` SSE 中增加 `tool_event` 帧。

#### 3.2.2 图表 Artifact

基于现有 `lightweight-charts`（见 `front-design.md` 技术栈）封装：

- `ChartArtifact` —— K 线 / 分时 / 成交量
- `TableArtifact` —— 涨停列表、龙虎榜（复用 `BreakoutView`、`DragonTigerView` 的表格逻辑）
- `KpiArtifact` —— 情绪温度、炸板率等卡片
- `ComparisonArtifact` —— 多股对比
- 【新增】`PlanArtifact` —— 操作计划视图（标的 / 触发条件 / 动作 / 止损 / 仓位），ADR-0002
- 【新增】`AccountArtifact` —— 模拟账户视图（持仓 / 资金 / 盈亏曲线），ADR-0003

AI 输出 artifact 时，前端优先复用已有视图组件，而不是重新画。

#### 3.2.3 多 Agent 编排复盘路径（不下结论）

> **ADR-0002 收敛：** 原"多 Agent 并发分析"重定义为"多 Agent 负责复盘路径编排，给路径不给结论"。每个角色的产出是"下一步看什么"的向导提示，不是分析结论。

当前 Role 系统是单 agent 串行。Phase 2 支持：

- 同时跑多个角色（情绪温度 / 龙头识别 / 风险监控）并行编排复盘路径
- 每个角色的中间结果作为 artifact 流式推到工作区（路径提示，非结论）
- 用户可在对话中指定"用 A 角色 + B 角色分析同一只票"，AI 汇总路径由用户拍板

涉及 `crates/core/src/roles/` 调度逻辑改造，需先定义 trait 接口（遵守 CLAUDE.md 模块边界约束）。

#### 3.2.4 AI 帮结论转计划（ADR-0002 延伸）

复盘末尾，AI 把用户拍板的复盘结论结构化成**操作计划草稿**（标的、触发条件、动作、止损、仓位），用户确认后落库。这是 AI 向导角色的延伸——铺路径的终点是生成计划草稿，用户确认。

#### 3.2.5 模拟撮合引擎（ADR-0003/0004）

**新增核心工程：** 模拟撮合引擎按操作计划在盘中自动撮合虚拟成交，维护虚拟账户的持仓/资金/盈亏。

- 撮合规则：**L1 最新价**成交（ADR-0004，已知风险：L1 系统性高估计划质量，尤其对涨停打板策略——逃生通道为未来接入五档盘口后升级 L2/L3）
- 账户持久化：**跨交易日累积**，可看长期模拟净值曲线（ADR-0004）
- 全程不接券商、不碰真钱

#### 3.2.6 实时行情管道（前置工程）

> **ADR-0004 待解决项：** `crates/api/src/sina.rs` 的 `fetch_quotes` 是请求-响应式，**非盘中实时行情流**。模拟撮合要盘中实时跑，需建立持续行情订阅机制（高频轮询新浪 vs WebSocket）。这是撮合引擎的前置工程，Phase 2 必须解决。

#### 3.2.7 验证

- AI 说"打开龙虎榜" → 右侧自动切到 DragonTigerView
- AI 输出 K 线 artifact → 右侧渲染可交互图表
- 多角色并发编排 → 工作区出现多个路径提示 artifact tab
- 复盘末尾 AI 产出操作计划草稿 → 用户确认落库
- 操作计划触发条件命中 → 模拟撮合按 L1 最新价记虚拟成交
- 模拟账户跨日累积持仓/盈亏

---

### Phase 3 — 计划触发与推送（1-2 周）

**目标**：AI 从"被动问答"升级为"操作计划触发器"。

> **ADR-0002 收敛：** 盘中推送语义从原"AI 分析结论推送"重定义为"操作计划的触发条件命中"（如"XX 高开，计划关注"）。AI 不做实时决策，只跑用户已定的计划。

#### 3.3.1 后台监控线程

复用现有 cron 调度（`crates/core/src/` 的 `tokio-cron-scheduler`），新增 **计划触发监控**：

- 盘中监控操作计划的触发条件，命中时推送"计划触发"提示到对话流
- 模拟撮合引擎按 L1 最新价自动撮合虚拟成交（ADR-0004），结果推到模拟账户视图
- 开盘前（9:15）推送"今日关注计划"摘要，收盘后（15:05）推送计划执行汇总

#### 3.3.2 桌面通知

Tauri 侧集成系统通知：

- 计划触发 → 系统通知 + 应用图标角标
- 用户点击通知 → 聚焦应用并滚动到对应计划/成交记录

#### 3.3.3 会话持久化

解决 `useAssistantChat` 刷新即丢的问题：

- 后端 Axum 增加 `/api/ai/sessions` 接口（基于现有 sqlite）
- 前端启动时拉取最近会话，支持多会话切换
- artifact 状态、操作计划、模拟账户随会话一起持久化

#### 3.3.4 验证

- 配置操作计划触发条件 → 盘中命中 → 对话流看到"计划触发"提示 + 模拟成交记录
- 模拟异常 → 收到系统通知
- 刷新页面 → 会话恢复，操作计划/模拟账户/artifact 重新渲染

---

## 4. 架构约束（遵守 CLAUDE.md）

### 4.1 函数签名

- 工作区渲染分发逻辑参数多，必须用 struct 封装：
  ```ts
  // ❌ 禁止
  function renderArtifact(type: string, payload: any, symbol: string, range: string)

  // ✅ 必须
  interface ArtifactSpec { type: ArtifactType; payload: ArtifactPayload }
  function renderArtifact(spec: ArtifactSpec): JSX.Element
  ```

### 4.2 模块边界

- AI ↔ 前端联动走 **定义好的事件协议**，不直接耦合组件内部状态
- `workspace` store 是唯一跨组件通信通道
- 后端 Axum 新增的 `tool_event` 帧必须先在 `crates/core` 定义 trait，前端再适配

### 4.3 不引入投机性配置

- 不为"未来可能支持"的 artifact 类型预留 if-else 分支
- 每种 artifact 类型必须有真实使用场景才加入 Renderer

---

## 5. 风险与取舍

| 风险 | 应对 |
|------|------|
| Artifact 协议复杂化 prompt | Phase 1 只支持 3 种（chart/table/kpi），验证 AI 输出稳定性后再扩 |
| AI 误触发桌面导航打扰用户 | 加"确认"机制：navigate 类工具需用户在对话中确认 |
| 多 Agent 并发资源占用 | Phase 2 限制最大并发数（默认 3），超出排队 |
| 会话持久化 schema 变更 | 用 sqlite migration，参考现有 `data.db` 模式 |
| **L1 撮合失真**（ADR-0004） | L1 最新价会系统性高估计划质量，尤其对涨停打板策略（涨停板只有买单无卖单，根本买不进，L1 却按涨停价成交）。逃生通道：未来接入五档盘口后升级 L2/L3。 |
| **ADR-0001 无代码兜底**（ADR-0005） | "给路径不给结论"靠 skill 作者自律，代码不拦。若 skill 未写约束，AI 会直接吐结论——这是零干预的既定代价，非 bug。 |
| **实时行情管道缺失**（ADR-0004） | `fetch_quotes` 是请求-响应式非盘中流，模拟撮合盘中跑的前置工程，Phase 2 必须解决。 |

---

## 6. 实施顺序建议

> **当前只走 M1。** M1 通过后再启动 Phase 1-3。

**M1（当前）：** AI-Native 复盘走通，零代码改动——见 `docs/m1-review-walkthrough-framework.md`。先验 AI-Native 复盘方向是否成立，再投工程。

**M1 通过后，按优先级推进（每步单独 grilling）：**

1. **Phase 1.1** SplitWorkspace + 路由重构（半天）
2. **Phase 1.2** ArtifactRenderer + 基础 artifact + 操作计划视图 + 模拟账户视图（2-3 天）
3. **Phase 1.3** workspace store + AI 输出解析（1 天）
4. **Phase 1.4** 验证 + 旧路由兼容（半天）
5. **Phase 2** AI 调桌面 API + 多 Agent 复盘路径编排 + AI 帮结论转计划 + 模拟撮合引擎 + 实时行情管道
6. **Phase 3** 计划触发监控 + 桌面通知 + 会话持久化

每个子任务完成后按 CLAUDE.md 工作流约定：询问是否提交代码，重大变更后同步文档 diff。

---

## 7. 待确认问题

> ADR-0001~0005 已收敛了产品定位、AI 角色、核心物件、撮合规则、skill 驱动原则。以下问题尚未定义，留给后续 grilling 关卡：

1. **操作计划的具体结构**：标的、触发条件、动作、止损、仓位的具体字段与状态机（待执行/已触发/已跳过/已超时）—— ADR-0002 待定义项。
2. **计划与复盘会话的关系**：一次复盘产出一个计划还是多个？
3. **实时行情管道选型**：高频轮询新浪 vs WebSocket？—— ADR-0004 待解决。
4. **M1 复盘效果验证**：M1 复盘好不好 100% 取决于 skill 内容质量（ADR-0005），代码不背锅。若复盘效果差，先改 skill 内容，不要动代码。

> **已确认（原 §7 已答）：** AI 对话默认左 40% 可拖拽；旧功能页保留为快捷入口；Artifact 协议用 markdown 围栏；会话持久化支持多会话切换。
