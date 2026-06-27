import js from "@eslint/js";
import globals from "globals";
import tseslint from "typescript-eslint";
import reactHooks from "eslint-plugin-react-hooks";
import reactRefresh from "eslint-plugin-react-refresh";
import tailwind from "eslint-plugin-tailwindcss";
import prettier from "eslint-config-prettier";

// =============================================================================
// Tailwind 白名单
// =============================================================================

// shadcn 语义色 token（@layer base 非 @theme，v4 不生成 utility）
// TODO: 迁移到 @theme --color-* 后删除此白名单，见 issue "shadcn 语义色 token 缺失"
const SHADCN_COLOR_TOKENS = "bg|text|border|ring|fill|stroke";
const SHADCN_TOKEN_NAMES =
  "background|foreground|muted|muted-foreground|border|ring|ring-ring|" +
  "popover|popover-foreground|accent|accent-foreground|card|card-foreground|" +
  "secondary|secondary-foreground|destructive|destructive-foreground|" +
  "input|primary-foreground|" +
  "sidebar|sidebar-foreground|sidebar-accent|sidebar-accent-foreground|" +
  "sidebar-border|sidebar-ring|sidebar-primary|sidebar-primary-foreground";

// 变体前缀：覆盖 hover: / dark: / aria-*: / data-[slot=x]: / [&>svg]: 等 v4 写法
const VARIANT = "(?:[^:\\s]+:)*";
const OPACITY = "(?:\\/[\\d.]+)?";

const TAILWIND_WHITELIST = [
  // A 类：@theme --font-size-* 生成的 text-* utility
  `^${VARIANT}text-(?:caption|caption-strong|body|body-strong|display-md)${OPACITY}$`,
  // A 类：项目自定义 class + clsx 形参误判
  `^${VARIANT}(?:data-table|markdown-preview|p-card-inner|prose|prose-sm|prose-invert|reasoning-content|inputs)$`,
  // B 类 TODO：shadcn 语义色（@layer base，非 @theme）
  `^${VARIANT}(?:${SHADCN_COLOR_TOKENS})-(?:${SHADCN_TOKEN_NAMES})${OPACITY}$`,
  // B 类 TODO：--font-heading（@layer base，非 @theme）
  `^${VARIANT}font-heading$`,
];

// =============================================================================
// ESLint 配置
// =============================================================================

export default tseslint.config(
  // 1. ignores 必须第一个独立块
  {
    ignores: ["dist", "node_modules", "coverage", ".vite", "src-tauri/target"],
  },

  // 2. 基础规则
  js.configs.recommended,
  ...tseslint.configs.recommended,

  // 3. TS/TSX 主配置
  {
    files: ["**/*.{ts,tsx}"],
    languageOptions: {
      globals: globals.browser,
    },
    plugins: {
      "react-hooks": reactHooks,
      "react-refresh": reactRefresh,
      tailwindcss: tailwind,
    },
    rules: {
      // --- React ---
      ...reactHooks.configs.recommended.rules,
      "react-refresh/only-export-components": [
        "warn",
        { allowConstantExport: true },
      ],

      // --- TypeScript ---
      "@typescript-eslint/no-explicit-any": "warn",
      "@typescript-eslint/no-unused-vars": [
        "warn",
        {
          argsIgnorePattern: "^_",
          varsIgnorePattern: "^_",
          caughtErrorsIgnorePattern: "^_",
        },
      ],
      "@typescript-eslint/no-empty-object-type": "warn",
      "@typescript-eslint/no-require-imports": "error",

      // --- Tailwind ---
      ...tailwind.configs.recommended.rules,
      "tailwindcss/classnames-order": "off", // 交给 prettier-plugin-tailwindcss
      "tailwindcss/no-custom-classname": [
        "warn",
        { whitelist: TAILWIND_WHITELIST },
      ],
    },
  },

  // 4. prettier 最后，覆盖所有格式规则
  prettier,
);
