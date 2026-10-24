# CLAUDE.md

## 后端架构约束

按照社区最佳实践来实现

---

## 错误处理

- 生产路径禁止 `unwrap()` / `expect()`；要么传播错误，要么在注释中说明该 panic 不可能发生的不变量
- 使用项目统一的 `Error` 类型，禁止随意 `.map_err(|e| anyhow!(e))`
- 错误信息面向调用者，不暴露内部实现细节

---

## 缺陷修复

Fix a bug at its cause, reasoning from first principles and established best practice. Do not patch over the symptom.

- 先拿出根因结论（哪个不变量被谁在什么前提下破坏），再动代码；没定位到就继续查，不先打补丁
- 从第一性原理推：正确数据的约束是什么、哪一层最先创造了非法状态；修复放在破坏点，不放在症状出现的调用点
- 修复方式是补齐通用正确路径（补全分支、显式错误、类型约束、挡住非法输入），不是加特判
- 不用重试、超时、默认值、降级或 `try/except` 静默吞错来掩盖问题
- 不为了"先让它不报错"加 if 特例、空实现或注释掉校验
- 修复后补一条能锁住该根因的回归测试；只在看了症状消失的自测不算完成

---

## 测试规范

### 优先级

1. Integration Test 和 E2E Test（验证真实业务流程）
2. 最容易失败的路径（边界、并发、外部依赖失败）
3. 不写 getter/setter 测试，不写简单纯函数测试

### 写测试前必须输出测试矩阵

列出：

- **Happy Path**：正常使用场景
- **Edge Case**：边界值、空数据、并发
- **Failure Case**：外部依赖失败、非法输入、超时

确认矩阵后再写代码。

---

## 文档维护

- `README.md`：功能概述、使用示例
- 内联注释：复杂逻辑必须注释，简单逻辑不注释
- 示例代码必须可运行，参数与最新实现一致
- 使用第二人称，标注前提条件，代码块带语言标签

---

## Anti-Patterns

### 依赖与配置

- 不为小便利引入重型依赖
- 不添加"以防万用"的投机性配置项或 feature flag
- 不静默削弱安全策略或访问约束

### 代码变更

- 不把大量格式化改动与功能改动混在同一次提交
- 不在"顺手"时修改无关模块
- 不在重构提交中隐藏行为变更的副作用
- 不绕过失败的检查，除非附上明确解释

### 代码质量

- 不用下划线前缀或 `#[allow(dead_code)]` 掩盖未使用的生产代码——删除、接入行为，或建 issue 跟踪
- 不为绕过失败测试而硬编码返回值或添加特判逻辑掩盖真实问题，必须实现通用正确路径
- 不在测试数据、示例、文档或提交中包含真实个人信息
- 不在生产路径留 `unwrap()` / `expect()`（见错误处理章节）

### 硬编码

- 不把应随环境变化的值（端口、超时时长、API endpoint、阈值参数）写死在代码里——提取为配置项或常量
- 不在业务逻辑中硬编码魔法数字，必须命名为有意义的常量并注明来源/依据
- 不硬编码与用户、环境强相关的路径（如 `/Users/xxx/...`、绝对路径假设）——用相对路径或运行时解析
- 不把密钥、token、内部地址等敏感信息直接写入源码，即使是临时调试也不行

### 复用优先

**代码库内复用**

- 写新逻辑前先搜索代码库是否已有等价实现，不重复造轮子
- 不为单一调用点提前设计通用抽象——抽象应在第二、三个真实使用场景出现时再提炼（Rule of Three）
- 不引入与现有架构风格不一致的抽象方式（如项目统一用 trait，不要突然引入泛型函数式组合）
- 公共逻辑提取为共享函数/模块时，抽取前后行为必须等价，不破坏原调用方的行为边界
- 不为了"看起来更通用"而增加不必要的泛型参数或 trait bound，复杂度需匹配实际使用场景数

**第三方库复用**

- 优先使用生态内成熟、维护活跃的库实现通用能力（序列化、HTTP 客户端、日期处理、加密、重试退避等），不手写已有可靠实现的功能
- 引入新依赖前检查下载量、最近更新时间、issue 响应情况，避免引入维护停滞或单人维护的脆弱依赖
- Rust 生态事实标准（`serde` / `tokio` / `reqwest`）、Go 标准库优先，不无理由用自造实现替代
- 自造实现仅限于：成熟库不存在、引入成本（依赖体积/复杂度）明显不划算、或业务逻辑高度定制化无法复用通用库

- 必须复用pi-rs提供的能力，不要重复实现，基于pi-rs提供的能力做开发。
  > **依赖取舍的核心标准**：能力本身是否存在大量边界情况、已被广泛验证（应复用成熟库），还是仅为局部小封装（应自实现，避免引入依赖）。

## 工作流

- 写完代码不要自动提交

## token节约

- 禁止使用playwright等浏览器自动化工具调试bug

## Frontend Architecture & Design System Guidelines

Prioritize mature, battle-tested libraries, keep component source code accessible and under control, maintain a consistent design language, and strictly limit duplicate dependencies.

With the right libraries and clear constraints in place, even a one-shot implementation can produce excellent frontend results.

Use the following libraries consistently across the project:

- **UI components and design system:** shadcn/ui (Nova style)
- **Icons:** Lucide
- **Animations:** Motion
- **Resizable layouts:** react-resizable-panels
- **Drag and drop:** dnd-kit
- **State management:** Zustand


## UI/UX Design Principles
- Avoid unnecessary secondary labels and descriptions. Let the layout do the explaining.
- Prefer clarity and simplicity over information density.
- Do not repeat information that is already clear from the page context, navigation, or surrounding components.
- Every label, icon, badge, card, and description must serve a clear user purpose.
- Prioritize essential information and use progressive disclosure for secondary details.
- Preserve visual consistency across existing screens, including typography, spacing, colors, and component styles.
- Avoid decorative UI elements, excessive borders, nested cards, and unnecessary status indicators.
- For industrial monitoring interfaces, make critical states immediately recognizable without adding visual noise.
- Show additional explanations only when they help users understand an abnormal state or take action.
- Before adding a new UI element, consider whether removing or simplifying an existing element would achieve the same goal.