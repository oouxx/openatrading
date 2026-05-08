# Stock Hover GIF Preview — Design

**Date:** 2026-05-08
**Scope:** Vue 3 SPA frontend

## Feature

Hover over a stock code or name shows a side mini-chart GIF from Sina Finance.

## URL Format

```
https://image.sinajs.cn/newchart/min/n/{prefix}{symbol}.gif
```

Prefix: `sh` for Shanghai (60xxxx, 68xxxx), `sz` for Shenzhen (00xxxx, 30xxxx).

## Interaction

- Hover code/name → floating card appears on the right side of the hovered row
- Card shows GIF (auto-play on load), stock code + name below
- Mouse leaves → 200ms delay close (avoid flicker on fast mouse moves)
- Click behavior unchanged (opens K-line drawer)

## Component: MiniChartPreview.vue

A shared component taking `symbol: string` and `name: string` props.

```vue
<template>
  <div class="relative inline-block" @mouseenter="show" @mouseleave="hide">
    <slot /><!-- text link -->
    <Teleport to="body">
      <div v-if="visible" class="fixed z-[60] bg-canvas rounded-card border border-hairline p-3 shadow-sm"
        :style="{ top: y + 'px', left: x + 'px' }">
        <img :src="gifUrl" width="240" height="180" class="rounded-[8px]" />
        <p class="text-caption font-mono text-center text-ink-muted mt-2">{{ symbol }} {{ name }}</p>
      </div>
    </Teleport>
  </div>
</template>
```

Position: calculate from the hovered element's bounding rect, place to the right.

## Components to Update

| Component | Where |
|-----------|-------|
| TabLimitUp | 代码列 `<td>`, 名称列 `<td>`, field items 代码/名称列 |
| TabPlate | 个股事件表代码列, 板块事件 stock chips |
| TabPromotion | 梯队详情表代码列, 首板表代码列 |

Each code/name cell wraps text in `<MiniChartPreview symbol="..." name="...">`.

## Verification

1. `npx vite build` — compiles without errors
2. Hover any stock code → GIF appears on right side
3. Click → K-line drawer still opens
4. Fast hover across multiple stocks → no flicker (debounce)
5. GIF URL format correct: `sh` for 60/68xxxx, `sz` for others
