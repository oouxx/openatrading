# ADR-0005: 代码只做机制，领域行为全由 skill 驱动，零人为干预

**状态：** 已接受
**日期：** 2026-06-19
**关联：** ADR-0001, docs/ai-native-desktop-plan.md, Front 7-8

## 背景

grilling 中确认复盘逻辑已放在 skill 机制里（`~/.atrading/skills/<name>/SKILL.md`，经 `SkillRegistry::from_disk()` 加载，`chat.rs` 的 `CommandType::Skill` 注入为 system prompt）。用户明确："skill 怎么写不是我们关心的，我们只把代码写好" + "不用加 prompt，要怎么做都在 skill 里面，不要人为干预"。

即：复盘的路径编排、向导约束、产物形态，全部由 skill 内容决定，代码不注入任何领域性默认 prompt。

## 决策

**代码只负责机制，不负责领域行为。所有复盘行为约束由 skill 内容承载，代码零人为干预。**

- 代码职责（机制层）：加载 skill、注入 skill 内容为 system prompt、装配工具、流式返回结果。仅此。
- 代码**不**做的事：不注入"复盘向导"默认 prompt、不强制"给路径不给结论"、不约束产物形态、不替 skill 兜底。
- ADR-0001 的"AI 是向导、给路径不给结论"**降级为 skill 作者的自律约束**，代码层无落实点。

## 影响与取舍

- **取舍：** 用"零人为干预"换取代码的纯粹机制定位。代价是 ADR-0001 在代码层无强制——若某个 skill 未写约束，AI 可能直接吐结论，代码不拦截。
- **M1 复盘走通的定义因此极简：** 现有 `/api/ai/chat` + skill 注入链路即闭环，前端不动布局，复盘零代码改动（前提是链路无 bug）。
- **后续若发现多 skill 需共享约束**，再考虑代码层兜底；当前不为未来需求预留接口（遵守 CLAUDE.md：不引入投机性配置）。
