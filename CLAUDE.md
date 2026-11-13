# CLAUDE.md
## 测试规范

- 不写不必要的测试
- 避免微小改动全量测试
---
## 复用优先

- 必须复用pi-rs提供的能力，不要重复实现，基于pi-rs提供的能力做开发。
  > **依赖取舍的核心标准**：能力本身是否存在大量边界情况、已被广泛验证（应复用成熟库），还是仅为局部小封装（应自实现，避免引入依赖）。
---
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

---
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
