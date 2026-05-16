# Vue 3 Frontend

## 开发

```bash
cd src && bun install
cd src && bun run dev        # Vite dev server（或 make dev-frontend）
```

## 检查

```bash
cd src && bun run build      # 生产构建，含类型检查
cd src && npx vue-tsc --noEmit  # 仅类型检查
```

## 测试

```bash
# 暂无 vitest（如有请补充）
```

## 约定

- 包管理器：**bun**（`bun.lock`，不是 `package-lock.json`）
- 构建输出：`src/dist/`（Vite build）
- CSS：**TailwindCSS 3.4**，深色终端风格
- 状态管理：**Pinia 3**，stores 在 `src/stores/`
- 组件：`src/components/`，按功能分 tab 子目录
- 图表：`lightweight-charts 5`
- 颜色：`--color-up: #FF5C5C`（红涨）、`--color-down: #1D9E75`（绿跌）
- 金融数字：`font-variant-numeric: tabular-nums`
