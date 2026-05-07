# K-line Chart: lightweight-charts → ECharts

## Goal

Replace `lightweight-charts` with `echarts` in `KlineDrawer.vue`, keeping all existing features identical. The only behavioral fix: 1-minute K-line time axis respects A-share trading hours (9:30–11:30, 13:00–15:00).

## Scope

Single file replacement: `src/src/components/KlineDrawer.vue` (~364 lines → ~300 lines expected).

## Feature checklist (feature-parity)

| # | Feature | ECharts equivalent |
|---|---------|-------------------|
| 1 | Candlestick (OHLC) | `series: { type: 'candlestick' }` |
| 2 | Volume histogram below chart | `series: { type: 'bar' }` + `grid[1]` |
| 3 | 1-min / daily / weekly / monthly periods | `series.setOption()` on period change |
| 4 | Crosshair info bar (OHLCV + change%) | `tooltip` + `axisPointer` |
| 5 | Floating price tooltip on crosshair | `axisPointer.label` |
| 6 | Limit-up / limit-down price lines (1-min only) | `markLine` |
| 7 | Previous-close zero line (1-min only) | `markLine` |
| 8 | 1-min center-bounding around preclose | `yAxis.min`/`max` computed from data |
| 9 | Time formatting (YYYY-MM-DD / YYYY-MM-DD HH:mm) | `xAxis.axisLabel.formatter` |
| 10 | Real-time quote header + metrics grid | unchanged (outside chart) |
| 11 | Side drawer (Teleport + overlay) | unchanged |
| 12 | **1-min fixes trading hours** | `xAxis.min`/`max` set to 9:30/15:00, no non-trading gaps |

## Data format

Backend API unchanged. Internal conversion:

```
Backend:  { time: unix_seconds, open, high, low, close, volume }
ECharts:  [open, close, low, high]   // candlestick
          [time, volume]              // volume bar
```

## Dependencies

- Remove: `lightweight-charts`
- Add: `echarts`

No `vue-echarts` wrapper — use native `echarts.init()` / `dispose()` in `onMounted` / `onUnmounted`.

## Verification

- `vue-tsc --noEmit` passes
- `vite build` succeeds
- Click stock symbol in 涨停板 tab → drawer opens with daily K-line
- Switch to 1-min → chart shows, time axis fixed to 9:30–15:00
- Crosshair shows OHLCV + change%
- Limit-up/down lines visible on 1-min
- Period switch (日K/周K/月K) works
