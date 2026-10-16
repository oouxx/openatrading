### 前端：shadcn 组件纪律（防止风格漂移）

**核心原则：UI 组件优先复用已注册的来源（shadcn registry），不允许为了"图省事"手搓一个功能重复但风格不一致的组件。**

禁止的模式：

- 项目里已有 `components/ui/` 下的 shadcn 组件（如 `Button`、`Command`、`Dialog`），却在新功能里直接写裸 `<button className="...">` 或重新定义一套样式
- 颜色/间距使用硬编码值（`#3b82f6`、`mt-[17px]`）而不是项目里已定义的 CSS 变量/Tailwind 设计 token（`bg-primary`、`mt-4`）
- 同一个语义的组件在不同文件里出现不同实现（比如三个地方各自写了一版"加载中"的 spinner，样式还不一样）
- 新增组件前没有检查 `components/ui/` 目录里是否已存在功能相近的实现

允许的例外（必须在代码注释中写明理由）：

1. shadcn registry 确实没有覆盖的功能（如业务专属的 K 线图、TUI 风格终端面板），此时才允许自定义，但仍要遵循项目已有的 Tailwind token 体系
2. 对 shadcn 生成文件的局部样式覆盖（如调整 `Command` 的圆角），优先用 `className` 覆盖或 CSS 变量调整，而不是 fork 整个文件重写

执行要求：

- 新增 UI 组件前，先确认 shadcn registry 是否已有对应条目：`npx shadcn@latest add <component>` 优先于手写
- 颜色、间距、圆角、阴影一律走项目 `tailwind.config` 里定义的 token，禁止裸写任意数值，除非该数值本身就是一次性、无复用意义的微调
- Review 时主动检查：新文件是否引入了与现有组件视觉/交互不一致的模式（圆角大小、按钮 hover 效果、间距密度），不一致需要说明原因或改用已有组件
- 涉及 AG-UI / 自定义 MessageBus 事件的 UI 交互（如 slash command 映射到 agent 行为），优先复用 `ComposerPrimitive` 的输入捕获能力，再接自定义事件分发，不要重新实现输入框本身

## Code Quality

### ESLint

Run lint check:

```bash
bun run lint
```

Auto fix:

```bash
bun run lint:fix
```

**Rules to follow:**

- Always run `bun run lint` after modifying TypeScript/TSX files
- Fix all ESLint errors before considering a task complete
- Warnings can be left but should be noted
- Tailwind classes must follow the order defined by `eslint-plugin-tailwindcss`

**Do NOT:**

- Disable ESLint rules with `eslint-disable` comments unless absolutely necessary
- Add `any` type to bypass TypeScript checks


## ESLint Policy

- `warn` level issues can be ignored during development
- Only `error` level must be fixed before commit
- Never add `eslint-disable` without a comment
- Format issues are handled by Prettier, not ESLint
