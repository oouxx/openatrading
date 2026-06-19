# M1：AI-Native 复盘走通框架

**状态：** 框架已定，待执行
**日期：** 2026-06-19
**关联：** ADR-0001 ~ 0005
**取代：** 本文档是 `docs/ai-native-desktop-plan.md` 的**当前执行切片**——原计划是全景路线图，本文档是"只走通复盘这一步"的最小框架，其余推后。

---

## 1. 范围边界

**这一步只做：在现有 `/copilot` 聊天页，用 skill 跑通 AI-Native 复盘。**

| 在范围内 | 推后 |
|---|---|
| skill 注入 + 全工具 + 流式返回（已通） | 操作计划生成（ADR-0002 的计划阶段） |
| 复盘对话能跑 | 模拟撮合引擎（ADR-0003/0004 的执行阶段） |
| | 前端布局改造（split-view / artifact） |
| | 复盘记录落库（M2/M3） |
| | 实时行情管道 |

**决策依据：** ADR-0005（代码只做机制）+ M1（纯对话不落库）+ 不动前端布局。先验 AI-Native 复盘方向是否成立，再投工程。

---

## 2. 代码改动：零

核实结论——M1 链路**已通**，零代码改动即可跑通复盘：

| 链路环节 | 现状 | 位置 |
|---|---|---|
| skill 加载 | ✅ `SkillRegistry::from_disk()` 从 `~/.atrading/skills/<name>/SKILL.md` 读 | `crates/core/src/skills/mod.rs:81` |
| skill 列入 slash 命令 | ✅ `get_ai_commands` 遍历 skills 返回 `CommandType::Skill` | `crates/web/src/chat.rs:66-71` |
| skill 注入 system prompt | ✅ `CommandType::Skill` 分支用 skill content 构建 prompt + 配全部工具 | `crates/web/src/chat.rs:218-231` |
| 流式返回 | ✅ SSE 多事件（text/thinking/tool-delta/tool-end） | `crates/web/src/chat.rs:181-370` |
| 前端渲染 | ✅ `@assistant-ui/react` 流式 + reasoning | `src/src/hooks/useAssistantChat.ts` |

**唯一前提：** skill 内容由用户（你）负责编写。代码不干预 skill 行为（ADR-0005）。

---

## 3. 验证步骤

M1"走通"的判定标准——在 `/copilot` 页完成一次完整复盘对话：

1. 启动 gateway：`openatrading gateway run`（或 `cargo tauri dev` 桌面端）
2. 打开 `/copilot` 页
3. slash 命令选择已安装的 skill（如"首板"）
4. 输入复盘指令（如"复盘今天"）
5. 验证：
   - AI 流式输出（非一次性返回）
   - AI 调用了市场数据工具（tool-delta/tool-end 事件触发，非纯嘴炮）
   - 返回内容与 skill 约束一致（skill 写了什么约束就表现什么）
6. 全程无 500/超时/工具调用失败

**通过 = M1 复盘走通。** 不通过 = 链路有 bug，按失败点定位（见 §5）。

---

## 4. ADR 接续关系

| ADR | 在 M1 的落实状态 |
|---|---|
| ADR-0001（AI 是向导） | ⚠️ 代码层无落实点（ADR-0005 决定）。靠 skill 内容自律。M1 不强制。 |
| ADR-0002（操作计划是核心物件） | ⏸️ 推后。M1 不产计划，纯对话。 |
| ADR-0003（模拟盘） | ⏸️ 推后。M1 无撮合。 |
| ADR-0004（L1 撮合+持久化） | ⏸️ 推后。M1 无账户。 |
| ADR-0005（代码只做机制） | ✅ M1 完全遵守。零代码干预。 |

---

## 5. 已知风险与边界

- **ADR-0001 无代码兜底**：若 skill 未写"给路径不给结论"约束，AI 会直接吐结论，代码不拦。这是 ADR-0005 的既定代价，不是 bug。
- **skill 内容质量决定复盘质量**：M1 的"复盘好不好"100% 取决于 skill 写得怎么样，代码不背锅。若复盘效果差，先改 skill 内容，不要动代码。
- **"首板"skill 现状**（`~/.atrading/skills/首板/SKILL.md`）内容极简（"只做首板"），复盘路径未编排。M1 能跑通机制，但复盘效果取决于你是否补全 skill 内容。

---

## 6. 下一步（M1 通过后）

M1 验证方向成立后，按优先级推进（每步单独 grilling，不在本框架内）：

1. **M2**：复盘记录落库（对话 + 产出持久化），能看历史复盘
2. **布局改造**：split-view + artifact，AI 当默认首页
3. **操作计划生成**：ADR-0002 的计划阶段，复盘末尾产出结构化计划
4. **模拟撮合引擎**：ADR-0003/0004 的执行阶段 + 实时行情管道
