# Tailwind CSS v4 + shadcn/ui 最佳实践

## 核心原则

> **只维护一套 Design Token，以 `@theme` 为唯一主题来源。**

避免同时维护：

-   `@theme`
-   `:root` 中另一套 `--background`、`--border` 等变量

这样可以降低维护成本，并与 Tailwind CSS v4 的设计保持一致。

------------------------------------------------------------------------

## 推荐目录结构

``` text
src/
├── styles/
│   ├── index.css
│   ├── theme.css
│   ├── base.css
│   ├── components.css
│   ├── animations.css
│   └── utilities.css
└── components/
    └── ui/
```

------------------------------------------------------------------------

## index.css

唯一入口：

``` css
@import "tailwindcss";
@import "tw-animate-css";
@import "shadcn/tailwind.css";
@import "@fontsource-variable/geist";

@import "./theme.css";
@import "./base.css";
@import "./animations.css";
@import "./components.css";
@import "./utilities.css";
```

------------------------------------------------------------------------

## theme.css

仅放 Design Token：

-   Color
-   Radius
-   Font
-   Shadow
-   Spacing（可选）

例如：

``` css
@theme {
  --color-background: rgb(250 250 245);
  --color-foreground: rgb(45 45 42);

  --color-surface: white;

  --color-border: rgb(0 0 0 / .06);
  --color-input: rgb(0 0 0 / .08);
  --color-ring: rgb(191 101 52);

  --color-primary: rgb(191 101 52);
  --color-primary-foreground: white;

  --radius-sm: 8px;
  --radius-md: 12px;
  --radius-lg: 18px;

  --font-sans: "Geist Variable";
}
```

不要在这里写：

-   body
-   html
-   -   
-   scrollbar

------------------------------------------------------------------------

## base.css

仅放基础样式：

``` css
@layer base {
  *,
  *::before,
  *::after {
    box-sizing: border-box;
  }

  html {
    font-family: var(--font-sans);
  }

  body {
    @apply bg-background text-foreground;
  }
}
```

包含：

-   Reset
-   body/html
-   Scrollbar
-   Selection

------------------------------------------------------------------------

## components.css

仅放全局组件覆盖。

例如：

``` css
[data-slot="dialog-content"] {
  border-radius: 20px;
}

[data-slot="dialog-overlay"] {
  backdrop-filter: blur(6px);
}
```

尽量不要修改 shadcn 组件源码。

------------------------------------------------------------------------

## animations.css

集中管理：

-   Skeleton
-   Loading
-   Accordion
-   Reasoning
-   Fade
-   Slide

所有 `@keyframes` 放在这里。

------------------------------------------------------------------------

## utilities.css

存放自定义 Utility：

``` css
@utility glass {
  backdrop-filter: blur(10px);
}

@utility text-gradient {
  ...
}
```

------------------------------------------------------------------------

## Dark Mode

推荐：

``` css
:root {
  --color-background: ...;
}

.dark {
  --color-background: ...;
}
```

不要维护两套 Token（`--background` 与 `--color-background`）。

------------------------------------------------------------------------

## shadcn/ui 最佳实践

-   保持官方组件源码不改。
-   通过 Token 控制颜色、圆角、阴影。
-   使用 `className` 扩展，而不是修改组件内部。

例如：

修改：

``` css
--color-input
```

即可影响所有 Input、Textarea 等组件。

------------------------------------------------------------------------

## 不要硬编码颜色

❌

``` css
background: #fff;
border: #ddd;
```

✅

``` css
background: var(--color-background);
border: var(--color-border);
```

或：

``` css
@apply bg-background border-border;
```

------------------------------------------------------------------------

## 推荐拆分

  文件             职责
  ---------------- ------------------------
  theme.css        Design Token
  base.css         Reset、body、scrollbar
  components.css   shadcn 全局覆盖
  animations.css   所有动画
  utilities.css    自定义 Utility

------------------------------------------------------------------------

## 总结

1.  `@theme` 作为唯一 Design Token 来源。
2.  避免维护两套主题变量。
3.  样式按职责拆分，入口保持简单。
4.  优先修改 Token，而不是修改 shadcn 组件。
5.  不硬编码颜色，全部通过 Token 或 Tailwind Utility 引用。
