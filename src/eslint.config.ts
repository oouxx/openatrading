import { defineConfig } from "eslint/config";
import js from "@eslint/js";
import globals from "globals";
import tseslint from "typescript-eslint";
import reactHooks from "eslint-plugin-react-hooks";
import reactRefresh from "eslint-plugin-react-refresh";
import prettier from "eslint-config-prettier";

export default defineConfig([
  // ----------------------------
  // 忽略
  // ----------------------------
  {
    ignores: [
      "dist",
      "node_modules",
      "coverage",
      ".vite",
      "src-tauri/target",
    ],
  },

  // ----------------------------
  // 基础 JS 推荐规则
  // ----------------------------
  js.configs.recommended,

  // ----------------------------
  // TypeScript 推荐
  // ----------------------------
  ...tseslint.configs.recommended,

  // ----------------------------
  // React / TSX 规则
  // ----------------------------
  {
    files: ["**/*.{ts,tsx}"],

    languageOptions: {
      globals: {
        ...globals.browser,
      },
    },

    plugins: {
      "react-hooks": reactHooks as any,
      "react-refresh": reactRefresh as any,
    },

    rules: {
      // React Hooks 推荐规则
      ...reactHooks.configs.recommended.rules,

      // React Fast Refresh
      "react-refresh/only-export-components": [
        "warn",
        { allowConstantExport: true },
      ],

      // TypeScript 常用约束
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
    },
  },

  // ----------------------------
  // Node.js scripts (.mjs)
  // ----------------------------
  {
    files: ["**/*.mjs"],
    languageOptions: {
      globals: {
        ...globals.node,
      },
    },
  },

  // ----------------------------
  // Prettier（必须最后）
  // ----------------------------
  prettier,
]);
