# A-Share Intraday Chart Design (分时图)

**Date:** 2026-05-08
**Status:** Approved

## 1. Overview

在现有 KlineDrawer 中新增专业 A 股分时图模式，使用 Lightweight Charts LineSeries 渲染。目标对标同花顺/东方财富/雪球的专业分时图体验。

## 2. Architecture

**方案：扩展 KlineDrawer.vue** — 将 "1分" 周期替换为 "分时"，在同一组件内按 period 分叉渲染逻辑。

```
Period Toggle: [分时] [日K] [周K] [月K]
               ↓
  'intraday' ────→ LineSeries ×3 + HistogramSeries + 双Y轴
  'day'/'week'/'month' ──→ CandlestickSeries + HistogramSeries（现有）
```

默认 `period = 'intraday'`，打开抽屉先看到分时图。

## 3. X-Axis: Index-Based with Custom Time Labels

**问题：** Lightweight Charts 基于时间轴，11:30-13:00 数据中断必然留白。

**方案：** 用数组索引 (0, 1, 2...) 作为 `time` 值，午休前后索引连续。

```
数据映射:
  09:30 → index 0    11:30 → index 120
  13:00 → index 121  15:00 → index 240

索引→时间标签映射表:
  timeLabels[0]="09:30", timeLabels[120]="11:30"
  timeLabels[121]="13:00", timeLabels[240]="15:00"
```

`tickMarkFormatter(index)` → `timeLabels[index]` 翻译为 HH:MM。

横轴稀疏显示关键时间点：`09:30 | 10:30 | 11:30/13:00 | 14:00 | 15:00`。

## 4. Chart Series Layout

分时模式下共 5 个 series：

```
┌───────────────────────────────────────────┐
│ LineSeries "price"      白色 1px          │ ← priceScaleId: 'left'
│ LineSeries "avgPrice"   橙色 1px dashed   │ ← priceScaleId: 'left'
│ PriceLine "preclose"    白色 20% 虚线     │ ← on price series
│ LineSeries "pctAnchor"  透明不可见         │ ← priceScaleId: 'right'
├───────────────────────────────────────────┤
│ HistogramSeries "vol"   红涨绿跌           │ ← priceScaleId: 'volume'
└───────────────────────────────────────────┘
        左Y轴(价格, 元)        右Y轴(涨跌幅, %)
```

- **price**: 每根1分钟 K线的 `close` 价格
- **avgPrice**: 累计 VWAP 均价（见 Section 5）
- **preclose**: `priceSeries.createPriceLine()` 绘制，昨收水平参考线
- **pctAnchor**: 数据值 = `(close - preclose) / preclose * 100`，用于驱动右轴 autoScale，使 `preclose ↔ 0%` 自然对齐
- **volume**: 红涨绿跌柱，涨 = `close >= open`

数据转换：

```typescript
// KlineCandle[] → intraday arrays
const priceData = candles.map((c, i) => ({ time: i, value: c.close }))

const volumeData = candles.map((c, i) => ({
  time: i,
  value: c.volume,
  color: c.close >= c.open ? 'rgba(255,59,48,0.3)' : 'rgba(52,199,89,0.3)',
}))
```

## 5. VWAP Average Price Line

前端从1分钟K线数据实时计算：

```typescript
let cumValue = 0   // Σ(close × volume) 近似成交额
let cumVolume = 0

const vwapData = candles.map((c, i) => {
  cumValue += c.close * c.volume
  cumVolume += c.volume
  return {
    time: i,
    value: cumVolume > 0 ? cumValue / cumVolume : c.close,
  }
})
```

**近似说明：** 用 `close * volume` 替代成交额（API 无 `turnover_value` 字段）。精度足够画参考线。后续若 API 增加成交额字段，改为 `cumValue += c.turnover_value`。

## 6. Dual Y-Axis

| 轴 | Scale ID | 显示 | 数据来源 |
|----|----------|------|----------|
| Left | `'left'` | 价格 (元) | price + avgPrice LineSeries |
| Right | `'right'` | 涨跌幅 (%) | hidden pctAnchor LineSeries |

**对齐原理：** 两个 scale 各自 autoScale。pctAnchor 数据 = `(price - preclose) / preclose * 100`，范围与价格系列线性对应，`preclose ↔ 0%` 中点对齐。

```typescript
// Right scale setup
chart.priceScale('right').applyOptions({
  visible: true,
  scaleMargins: { top: 0, bottom: 0 },
})

// pctAnchor data
const anchorData = candles.map((c, i) => ({
  time: i,
  value: (c.close - preclose) / preclose * 100,
}))

chart.addSeries(LineSeries, {
  priceScaleId: 'right',
  lineWidth: 0,          // invisible
  lastValueVisible: false,
  priceLineVisible: false,
  crosshairMarkerVisible: false,
}).setData(anchorData)
```

## 7. Crosshair

`chart.subscribeCrosshairMove()` 分时模式下信息栏：

```
  09:45    开 10.02  高 10.05  低 9.98  均 9.99  量 1.2万  +0.50%
```

字段：时间 | 开盘 | 最高 | 最低 | 均价 | 成交量 | 涨跌幅

从 crosshair 命中的 `price: LineData` 和 `volume: HistogramData` 提取值，均价和涨跌幅通过 `dataIndex` 查找对应 VWAP 和 pctAnchor 数组。

## 8. Period Switching

```
分时 → 日K/周K/月K:
  chart.remove() → createChart() → CandlestickSeries + HistogramSeries → fetchKline()

日K/周K/月K → 分时:
  chart.remove() → createChart() → LineSeries×3 + HistogramSeries → fetchKline()
```

Lightweight Charts 不支持动态替换 series 类型，切换周期销毁重建 chart。

## 9. Data Flow

```
XuanguBao API
  │ GET /api/kline/{symbol}?period=minute&count=256
  │ → KlineData { candles: Vec<KlineCandle> }
  ▼
fetchKline()
  │ → klineData = data.candles
  │ → filter(isTradingTime)  // 仅保留 9:30-11:30, 13:00-15:00
  │ → 按时间排序后分配 index: 0, 1, 2...
  ▼
renderIntraday()
  │ → buildPriceData()
  │ → buildVwapData()
  │ → buildVolumeData()
  │ → buildPctAnchorData()
  │ → setData() × 4
  │ → createPriceLine(preclose)
  │ → timeScale().fitContent()
```

## 10. Edge Cases

| 场景 | 处理 |
|------|------|
| 非交易时段 | 空 chart + 提示文字 |
| 数据不足（新股/停牌） | 有数据显示多少画多少，0条显示"暂无分时数据" |
| 仅有上午数据 | 折线到 11:30 截止（index 120），无下午段 |
| 复权/除权日 | `preclose_px` 已修正，无需额外处理 |
| API 报错 | crosshair bar 显示 "-"，chart 空白 |
| 快速切换周期 | `chart?.remove()` 判空，`fetchKline()` 竞态由浏览器自动取消前一个 fetch |
| 数据实时更新 | 后续用 `series.update(point)` 增量更新，不频繁 `setData` |

## 11. Styling

| 元素 | 颜色 | 样式 |
|------|------|------|
| 价格折线 | `#ffffff` | width 1, solid |
| 均价线 | `#f5a623` | width 1, dashed |
| 昨收线 | `rgba(255,255,255,0.20)` | width 1, dashed |
| 涨柱 | `rgba(255,59,48,0.3)` | — |
| 跌柱 | `rgba(52,199,89,0.3)` | — |
| 背景 | `#272729` | — |
| 网格 | `rgba(255,255,255,0.06)` | — |
| 文字 | `#86868b` | font 10px |

## 12. File Manifest

| File | Change |
|------|--------|
| `src/src/components/KlineDrawer.vue` | 新增 intraday 渲染分支，替换 period='minute' 为 'intraday' |
| `src/package.json` | 不变（lightweight-charts 已安装） |
| `crates/core/src/api/xuangubao.rs` | 不变（复用现有 fetch_kline minute） |
| `crates/web/src/main.rs` | 不变 |

## 13. Test Notes

- TypeScript 类型检查 + Vite 生产构建必须通过
- 手动验证：打开任意 A 股代码 → 默认显示分时图 → 价格折线连续无午休空白 → 昨收虚线可见 → 左价格轴 + 右涨跌幅轴 → 切换日K/周K/月K 正常
- 手动验证极端数据：停牌股、新股（< 1 天数据）、非交易时段打开
