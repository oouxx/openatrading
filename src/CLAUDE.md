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

### 设计系统契约（`src/src/styles/theme.css` 为唯一来源）

**选定风格：暖灰极简（Claude 风）—— 低饱和中性底 + 单一暖杏强调 + 细描边分层 + 克制动效。**
不引入玻璃态大面积面板、霓虹渐变、拟物阴影。唯一例外：`SettingsView` 的主题预览图是固定明暗的设备 mock，允许写死字面色。

- **色彩**：1 个品牌强调色（`brand`，兼作 `ai` 强调）+ 涨跌数据色（`up`/`down`，彩度更高）+ 状态色（`success`/`warning`/`danger`/`info`）。
  - 禁止 Tailwind 原始调色板（`text-yellow-500`、`bg-slate-800`…）与内联 `#hex`/`rgba()`（图表除外，走 `lib/chartColors`）。
  - 禁止 `text-white`/`bg-black`：字面色只有 `pure-white`/`pure-black`，遮罩用 `scrim`/`scrim-soft`/`scrim-strong`。
  - 禁止用 `text-primary/40` 这类透明度叠色做层级——三级语义色 `primary`(15.8:1) / `secondary`(8.8:1) / `tertiary`(5.0:1) 已按 WCAG 标定，直接用。
- **排版**：正文 Geist（`--font-sans`）、数字/代码 Geist Mono（`--font-number`）；字号只用 `text-hero/h1/h2/body/caption` 五级，`caption` 只给元信息（时间、计数、代码），正文段落用 `text-body`。
  - 中文标签禁止 `uppercase` 与 `tracking-wider`（CJK 无大小写、字距会散架）。
- **间距**：面板容器 `p-6`，卡片内容 `p-card-inner`(16px)，列表行 `px-5 py-4`，列表间距 `space-y-4`、`gap-3` 起。不要回到 `p-2`/`gap-2` 的拥挤密度。
- **动效**：时长与缓动只有一套 token——`--default-transition-duration`(200ms) 与 `--default-transition-timing-function`(`cubic-bezier(.32,.72,0,1)`，与 `lib/motion.ts` 的 EASE 同源)。用 `transition-colors` 等显式属性，只有进度条类才用 `transition-[width]`；禁止 `transition-all` 与点击位移（`active:translate-y`），用 `active:opacity-90`。
- **分层**：面板 = `bg-surface` + `border-hairline`；浮层/弹窗 = `shadow-modal`；卡内子块 = `bg-canvas`；hover = `bg-hover`。不要再混入 `border-border` / `hairline-dark`。
- **圆角**：只用 `--radius-sm/md/lg`（4/6/8px），禁止 `rounded-[Npx]`。
- 系统开启「减少动效」时全站动画自动收敛（`styles/animations.css` 的 `prefers-reduced-motion` 块）。
