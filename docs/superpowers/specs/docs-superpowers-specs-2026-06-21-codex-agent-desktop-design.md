# Codex Desktop 风格 Agent 交易工具 — 设计文档

> 目标：将当前 OpenATrading Web 应用（6 页面 + AI Copilot）重构为三栏 Agent 布局，桌面端（Tauri）和 Web 端统一使用同一套布局。区别仅在于：
> - **桌面端**：直接进入三栏布局（登录后到聊天会话）
> - **Web 端**：多一个 LandingPage（产品介绍页），用户点击"开始使用"后进入三栏布局

---

## 1. 整体布局

### 两种布局模式

```                                          ```

**模式一：聊天模式（三栏）— 默认状态**

```
┌───────┬──────────────────────────┬──────────┐
│ 左侧   │  中央                     │  右侧     │
│ ~240px │  (自适应)                 │  ~420px  │
│ 可折叠  │                          │  可折叠   │
│ 可调宽  │  AI 对话                  │  可调宽   │
│        │  ├ 流式 Markdown          │          │
│ 会话    │  ├ slash command 快捷指令 │  手动打开 │
│ 列表    │  ├ 文字分析、代码块        │  的固定    │
│        │  └ 不渲染表格/SVG/组件     │  视图     │
│        │                           │          │
│        │   输入框下方：              │  龙虎榜   │
│        │  [龙虎榜] [晋级] [回放] [角色]│  晋级分析  │
│        │                           │  行情回放  │
│        │                           │  角色策略  │
└───────┴──────────────────────────┴──────────┘
    ←─── 拖拽调宽 ───→      ←── 拖拽调宽 ──→
```

**模式二：编辑器模式（两栏）— 点击"技能"或"设置"**

```
┌───────┬────────────────────────────────────────┐
│ 左侧   │  Skill 编辑器 / 设置页面               │
│ 会话    │  (占满右侧区域)                       │
│ 列表    │                                       │
│        │  Skill 编辑：                           │
│        │  ┌───编辑器(MD)───┬───预览───┐         │
│        │  │ name: [...]    │ 渲染后的  │         │
│        │  │ description    │ 指令内容  │         │
│        │  │ 关联工具: ☑... │           │         │
│        │  └────────────────┴──────────┘         │
└───────┴────────────────────────────────────────┘
```

**切换规则：**
- 点击左侧某个会话 → 进入聊天模式（当前会话）
- 点击左侧"技能"→ 切换到编辑器模式（两栏），右侧区域变为 Skill 编辑器
- 点击左下角"设置"→ 切换到设置模式（两栏），右侧区域变为设置页面
- 点击左侧某个会话标题或关闭编辑器 → 回到聊天模式

### 1.1 顶栏

- 左侧：应用名称 "OpenATrading"
- 高度 40px，暗色背景下半透明，border-bottom hairline
- **移除**现有的 6 个 NavLink 导航（用户不再手动切换页面，全由 Agent 调度）

### 1.2 左侧面板 (~240px，可折叠，可拖拽调整宽度)

```
┌────────────────────┐
│  + 新建会话         │ ← 按钮，点击展开 skill 下拉列表
├────────────────────┤
│  技能               │ ← 文字入口，点击切换到编辑器模式
├────────────────────┤
│ 🔍 搜索会话...      │
├────────────────────┤
│  今日                │
│  ├ 市场综述          │
│  昨日                │
│  ├ 涨停晋级复盘      │
├────────────────────┤
│                     │
├────────────────────┤
│ ⚙ 设置             │ ← 固定在左下角
└────────────────────┘
```

- 会话按天分组，支持搜索、选中高亮、右键删除
- 每个会话绑定一个 Skill，显示在会话标题下方（小字标签）
- 点击某个会话，中央恢复该会话的消息历史
- 支持拖拽调整宽度（最小 180px，最大 360px）
- 支持折叠为仅图标栏（~48px），点击图标展开/悬停展开

### 1.3 中央面板 (自适应) — AI 对话

- 登录后**默认自动创建一个新会话**并聚焦到输入框，用户可立即开始对话
- 直接复用现有 `AiCopilot.tsx` 组件及 `useAssistantChat` hook
- 去除 `min-h-screen pt-11` 等全屏样式，自适应填满中央区域
- 中央**仅做 AI 对话**——流式 Markdown、文字分析、代码块。不渲染表格、图表、组件
- 输入框下方有一排快捷按钮，点击直接打开右侧对应页面：

```
输入框: [ 输入你的问题...                  ] [发送]
────────────────────────────────────────────
[📈 龙虎榜] [🔥 晋级分析] [🎯 行情回放] [🤖 角色策略]
```

这些快捷按钮等价于手动打开右侧面板的入口。

**Slash Command（复用现有机制）：**

现有 slash command 实现保持不动：
- 通过 `unstable_useSlashCommandAdapter` 实现，Command 列表从 `/api/ai/commands` 获取
- 支持 `command_type`: `"tool"` | `"skill"` | `"pipeline"`
- 每个 Command 有 `name` + `description`，支持 Tab 自动补全
- 选择 Command 后 `cmdRef` 记录命令名和类型，随消息一起发给后端
- 新增：绑定当前 Skill 后，Command 过滤仅展示当前 Skill 支持的工具

### 1.4 布局切换逻辑

```
type AppMode = 'chat' | 'skill-editor' | 'settings';
```

- `chat` 模式：三栏（左侧会话 + 中央聊天 + 右侧参考面板），左右可折叠/调宽
- `skill-editor` 模式：两栏（左侧会话 + 中央 Skill 编辑器），点击左侧"技能"触发
- `settings` 模式：两栏（左侧会话 + 中央设置页面），点击左下角设置触发
- 点击左侧某个会话标题 → 回到 `chat` 模式

### 1.5 右侧面板 (~420px，可折叠，可拖拽调整宽度)

右侧面板**手动打开**，不随对话自动切换。右侧是一个简单的 ViewRouter：

```
type RightPanelView = 'dragon-tiger' | 'breakout' | 'replay' | 'roles' | null;
```

**打开方式：**
1. 点击输入框下方的快捷按钮（"龙虎榜""晋级分析"等）
2. 通过 slash command（如 `/dragon` 自动打开龙虎榜）

**可用的页面（直接嵌入现有完整页面）：**
- `🐉 龙虎榜` — 嵌入 `DragonTigerView.tsx`（去除外部 padding 和 min-height）
- `🔥 晋级分析` — 嵌入 `BreakoutView.tsx`（适配窄列布局）
- `🎯 行情回放` — 嵌入 `ReplayView.tsx`（Canvas 自适应宽度）
- `🤖 角色策略` — 嵌入 `RoleView.tsx`（侧边栏改为面板内 Tab）

技术实现：每个 View 包裹在 `<RightPanelView>` 容器中，容器提供标题栏 + 关闭按钮 + 滚动，内部渲染原始 View 组件。

- 支持拖拽调整宽度（最小 320px，最大 600px）
- 支持折叠关闭，关闭后中央占满全部宽度
- 点击快捷按钮切换右侧视图（如果已打开则替换，如果已打开同一视图则关闭）
- 右侧视图切换不影响中央对话

---

## 2. 左侧面板详情

### 2.1 新建会话 & Skill 选择

点击"+ 新建会话"按钮，展开下拉列表：

```
┌─────────────────┐
│  📈 市场分析     │
│  🐉 龙虎榜追踪    │
│  🔥 涨停晋级      │
│  🎯 行情回放      │
│  🤖 策略管理      │
│  ⚡ 自由对话      │
└─────────────────┘
```

点击某个 Skill：
1. 使用该 Skill 的 `name` 和 `description` 创建新会话
2. Skill 的 `SKILL.md` 指令内容作为 system prompt 注入 agent 上下文
3. Skill 的 `allowed-tools` 决定了 agent 可调用的工具
4. 自动进入中央聊天界面

### 2.2 会话管理

- 会话列表按日期分组（今天/昨天/更早）
- 选中状态高亮
- 每个会话显示标题 + 绑定 Skill 名称（小字标签）
- 搜索过滤
- 右键菜单：重命名、删除、导出会话

### 2.3 设置入口

- 固定在左下角齿轮图标 + "设置"
- 点击后切换到**两栏模式**，中央区域加载设置页面
- 点击左侧会话回到聊天模式
- 设置内容：
  - **Agent 配置**：API Key（LLM）、默认模型选择、系统提示词
  - **MCP 服务器管理**：添加/编辑/启停 MCP 连接（名称、URL、密钥）
  - **交易参数**：品种过滤器、风险阈值
  - **外观**：主题切换（暗色/亮色）
  - **数据源**：API 地址配置

---

## 3. Skill 系统（遵循 agentskills.io 规范）

### 3.1 数据格式

遵循 [agentskills.io](https://agentskills.io/specification) 标准：

```
skills/
├── market-analysis/
│   ├── SKILL.md       # 必选: YAML frontmatter + Markdown 指令
│   ├── scripts/       # 可选: 可执行代码
│   └── references/    # 可选: 附加文档
├── dragon-tiger-tracking/
│   ├── SKILL.md
│   └── scripts/
└── ...
```

**SKILL.md 格式：**

```yaml
---
name: dragon-tiger-tracking
description: 龙虎榜分析 — 查询上榜个股、营业部买卖明细、净买入排行。用于追踪主力资金动向。
license: MIT
allowed-tools: dragon_tiger_detail dragon_tiger_seat
---

你是一个龙虎榜分析专家，擅长追踪主力资金动向。

## 可查询的信息
- 上榜个股列表（含涨跌幅、净买入、上榜原因）
- 买方/卖方席位明细
- 上榜后 T+1/T+5/T+10 表现

## 工具
- `dragon_tiger_detail(page, pageSize)` — 获取龙虎榜列表
- `dragon_tiger_seat(code, date, direction)` — 获取某只股票的席位明细
```

### 3.2 Skill 编辑器（占据中央+右侧区域，两栏布局）

点击左侧"技能"文字，右侧两栏区域统一变为 Skill 编辑器：

```
┌──────┬──────────────────────────────────────┐
│ 左侧  │  Skill 编辑器                        │
│ 会话  │                                      │
│      │  已安装 Skill          [导入] [导出]    │
│      │  ┌───┐ ┌───┐ ┌───┐ ┌───┐            │
│      │  │市场│ │龙虎│ │涨停│ │策略│            │
│      │  │分析│ │榜  │ │晋级│ │管理│            │
│      │  └───┘ └───┘ └───┘ └───┘            │
│      │                                      │
│      │ ── 当前编辑: dragon-tiger-tracking ── │
│      │                                      │
│      │ name:     [dragon-tiger-tracking    ] │
│      │ description: [龙虎榜分析...         ] │
│      │                                      │
│      │ 关联工具（从后端 /api/tools 获取）:    │
│      │ ☑ dragon_tiger_detail  龙虎榜明细查询  │
│      │ ☑ dragon_tiger_seat    营业部席位查询  │
│      │ ☐ market_overview      大盘概况       │
│      │ ☐ limit_up_list        涨停列表       │
│      │ ☐ replay_events        行情事件回放   │
│      │ ☐ role_list            角色列表       │
│      │ ☐ role_positions       角色持仓       │
│      │                                      │
│      │ ┌─ 指令 (Markdown) ──────┐ ┌─ 预览 ─┐│
│      │ │ 你是一个龙虎榜分析专家.. │ │ 渲染后  ││
│      │ └────────────────────────┘ └────────┘│
│      │                                      │
│      │ [ 保存 ]    [ 重置 ]                 │
└──────┴──────────────────────────────────────┘
```

- 工具列表从后端接口 `/api/tools` 获取，每项包含: `name`, `description`, `schema`
- 前端直接打勾选择，不再手写工具名称
- 选择的工具自动写入 SKILL.md 的 `allowed-tools` 字段
- 布局类似现有 RoleView 编辑器（左编辑右预览 Markdown）

### 3.3 Skill 加载机制（Progressive Disclosure）

遵循 agentskills.io 的渐进式加载策略：

1. **发现阶段**：应用启动时扫描 `skills/` 目录，只加载每个 skill 的 `name` 和 `description`
2. **新建会话时**：将所选 skill 的完整 `SKILL.md` 注入为 system prompt
3. **工具绑定**：skill 的 `allowed-tools` 字段决定了该会话中 agent 可调用的 API 工具

### 3.4 预设 Skill

| Skill 名称 | 描述 | 关联 API |
|---|---|---|
| `market-analysis` | 市场大盘分析 | indexQuotes, overview |
| `dragon-tiger-tracking` | 龙虎榜追踪 | dragonTigerDetail |
| `breakout-analysis` | 涨停晋级分析 | limitUp, yesterdayLimitUp |
| `market-replay` | 行情回放复盘 | replayEvents |
| `strategy-manager` | 角色策略管理 | role CRUD APIs |
| `free-chat` | 通用 AI 助手 | 全部工具可调 |

---

## 4. 右侧页面改造

四个现有页面直接嵌入右侧面板，每个页面保留完整功能，仅做布局适配：

| 页面 | 适配修改 |
|---|---|
| `DragonTigerView.tsx` | 去除 `min-h-screen pt-11` 和 max-w 约束 |
| `BreakoutView.tsx` | 左侧筛选栏改为顶部收缩栏，表格列减少 |
| `ReplayView.tsx` | Canvas 宽度自适应，去除全屏高度约束 |
| `RoleView.tsx` | 260px 侧边栏改为面板内 Tab 切换 |

嵌入方式：
```tsx
<RightPanelView title="龙虎榜" onClose={() => setView(null)}>
  <DragonTigerView />
</RightPanelView>
```

每个页面保持独立状态，切换右侧视图时不丢失数据。

---

## 5. 会话与 Skill 绑定数据模型

```typescript
interface Conversation {
  id: string;
  title: string;
  skillName: string;          // 绑定的 skill 名称
  messages: Message[];
  createdAt: number;
  updatedAt: number;
}

interface Message {
  role: 'user' | 'assistant';
  content: string;
  timestamp: number;
  toolCalls?: ToolCall[];      // agent 调用的工具记录
}

interface SkillEntry {
  name: string;
  description: string;
  license?: string;
  allowedTools?: string[];
  path: string;                // skills/<name>/SKILL.md
  isBuiltin: boolean;
}
```

---

## 6. 文件系统结构

```
resources/                    # Tauri 打包资源
├── skills/                   # 预设 Skill
│   ├── market-analysis/SKILL.md
│   ├── dragon-tiger-tracking/SKILL.md
│   └── ...

<app-data-dir>/               # 用户数据目录 (Tauri app_data_dir)
├── sessions/                 # 会话数据
│   ├── <session-id>.json
│   └── ...
├── skills/                   # 用户安装的自定义 Skill
│   └── ...
└── settings.json             # 应用设置
```

---

## 6. 实施阶段

### Phase 1: 布局重构
1. 创建 `AppShell.tsx` — 管理 `chat` / `skill-editor` / `settings` 三种模式切换
2. 左侧 `ConversationSidebar.tsx` — 会话列表 + 技能入口 + 设置入口，可折叠可调宽
3. 右侧 `ReferencePanel.tsx` — 固定/钉住的参考视图，可折叠可调宽
4. 中央嵌入 AiCopilot.tsx，去除全屏样式
5. 三栏拖拽分隔条（左右均可调整宽度）
6. 折叠/展开切换（左右栏独立）
7. 布局模式切换逻辑
8. Skill 可视化编辑器（`SkillEditor.tsx`，两栏模式）
9. 设置页面（`SettingsView.tsx`，两栏模式）

### Phase 1b: Web 端 LandingPage
- 保留现有 `LandingPage.tsx` 作为 Web 端首页
- LandingPage 顶部 CTA 按钮"开始使用"跳转到 `/app` 路由
- Web 端路由：`/` → LandingPage，`/app` → `AppShell`（三栏布局）
- 桌面端（Tauri）：`isTauri` 检测后直接渲染 `AppShell`，无 LandingPage

### Phase 2: Skill 系统
1. Skill 数据结构和加载器（扫描本地 skills/ 目录）
2. Skill 选择器（新建会话下拉）
3. 对接后端 `/api/tools` 接口获取可用工具列表
4. Skill 编辑器中的打勾选工具功能
5. 预设 Skill (6 个)
6. 导入/导出标准 agentskills.io skill

### Phase 3: 右侧面板
1. `RightPanelView.tsx` — 通用容器（标题栏 + 关闭 + 滚动 + 拖拽调宽）
2. 输入框下方快捷按钮栏（龙虎榜/晋级/回放/角色）
3. `DragonTigerView.tsx` 适配窄列
4. `BreakoutView.tsx` 适配窄列
5. `ReplayView.tsx` 适配窄列
6. `RoleView.tsx` 适配窄列（侧边栏改为 Tab）
7. 右侧面板折叠/展开状态管理

### Phase 4: 会话持久化 + Tauri 集成
1. 会话数据存储和恢复
2. Skill 数据持久化到本地文件系统
3. Tauri 窗口管理
4. 设置持久化
