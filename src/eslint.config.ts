import js from "@eslint/js";
import globals from "globals";
import tseslint from "typescript-eslint";
import reactHooks from "eslint-plugin-react-hooks";
import reactRefresh from "eslint-plugin-react-refresh";
import tailwind from "eslint-plugin-tailwindcss";
import prettier from "eslint-config-prettier";

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
      "tailwindcss/no-custom-classname": "warn",
    },
  },

  // 4. prettier 最后，覆盖所有格式规则
  prettier,
);
