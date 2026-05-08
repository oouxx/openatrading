# A-Share Intraday Chart Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the 1-min candlestick period with a professional A-share intraday line chart (分时图) in KlineDrawer using Lightweight Charts.

**Architecture:** Extend KlineDrawer.vue — split `initChart()` and `renderChart()` into intraday/kline branches. Intraday uses LineSeries (price + VWAP + pctAnchor) + HistogramSeries (volume) on index-based x-axis with custom time labels and dual Y-axis. Period switching destroys and recreates chart.

**Tech Stack:** Vue 3, TypeScript, Lightweight Charts 5.2, existing XuanguBao kline API

---

### Task 1: Replace period definitions and defaults

**Files:**
- Modify: `src/src/components/KlineDrawer.vue`

- [ ] **Step 1: Change periods array and default period**

```typescript
// Line 76-81: Replace
const periods = [
  { label: '分时', value: 'intraday' },
  { label: '日K', value: 'day' },
  { label: '周K', value: 'week' },
  { label: '月K', value: 'month' },
]

// Line 83: Change default
const period = ref('intraday')
```

- [ ] **Step 2: Update formatTime to handle 'intraday'**

```typescript
// Line 137-143: Replace formatTime
function formatTime(ts: number, p: string): string {
  const d = new Date(ts * 1000)
  const pad = (n: number) => String(n).padStart(2, '0')
  const ymd = `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`
  if (p === 'intraday' || p === 'minute') return `${ymd} ${pad(d.getHours())}:${pad(d.getMinutes())}`
  return ymd
}
```

- [ ] **Step 3: Update fetchKline filter condition**

```typescript
// Line 165: Replace 'minute' with 'intraday'
if (period.value === 'intraday') {
  klineData = klineData.filter((c: any) => isTradingTime(c.time))
}
```

- [ ] **Step 4: Run type check**

```bash
cd src && npx vue-tsc --noEmit
```

Expected: No errors.

- [ ] **Step 5: Commit**

```bash
git add src/src/components/KlineDrawer.vue
git commit -m "feat: replace 1-min period with intraday stub"
```

---

### Task 2: Add intraday-specific state and helpers

**Files:**
- Modify: `src/src/components/KlineDrawer.vue`

- [ ] **Step 1: Add intraday series state variables**

```typescript
// After line 93 (priceLines: PriceLine[] = []):
let priceSeries: ISeriesApi<'Line'> | null = null
let avgPriceSeries: ISeriesApi<'Line'> | null = null
let pctAnchorSeries: ISeriesApi<'Line'> | null = null
let timeLabels: string[] = []
```

- [ ] **Step 2: Add buildTimeLabels function**

```typescript
// After isTradingTime():
function buildTimeLabels(candles: any[]): string[] {
  return candles.map((c) => {
    const d = new Date(c.time * 1000)
    const pad = (n: number) => String(n).padStart(2, '0')
    return `${pad(d.getHours())}:${pad(d.getMinutes())}`
  })
}
```

- [ ] **Step 3: Add buildVwapData function**

```typescript
// After buildVolumeData():
function buildVwapData(): { time: number; value: number }[] {
  let cumValue = 0
  let cumVolume = 0
  return klineData.map((c: any, i: number) => {
    cumValue += c.close * c.volume
    cumVolume += c.volume
    return {
      time: i,
      value: cumVolume > 0 ? cumValue / cumVolume : c.close,
    }
  })
}
```

- [ ] **Step 4: Add buildPctAnchorData function**

```typescript
// After buildVwapData():
function buildPctAnchorData(preclose: number): { time: number; value: number }[] {
  return klineData.map((c: any, i: number) => ({
    time: i,
    value: (c.close - preclose) / preclose * 100,
  }))
}
```

- [ ] **Step 5: Add buildIndexedData helpers for intraday**

```typescript
// Replace buildCandleData with index-aware version:
function buildPriceData(): { time: number; value: number }[] {
  return klineData.map((c: any, i: number) => ({
    time: i,
    value: c.close,
  }))
}

function buildIntradayVolumeData(): HistogramData[] {
  return klineData.map((c: any, i: number) => ({
    time: i as Time,
    value: c.volume || 0,
    color: c.close >= c.open ? 'rgba(255,59,48,0.3)' : 'rgba(52,199,89,0.3)',
  }))
}
```

- [ ] **Step 6: Run type check**

```bash
cd src && npx vue-tsc --noEmit
```

Expected: No errors.

- [ ] **Step 7: Commit**

```bash
git add src/src/components/KlineDrawer.vue
git commit -m "feat: add intraday data builders and state"
```

---

### Task 3: Create initIntradayChart function

**Files:**
- Modify: `src/src/components/KlineDrawer.vue`

- [ ] **Step 1: Add initIntradayChart function**

Replace the single `initChart()` with two functions. First, rename existing `initChart` to `initKlineChart`:

```typescript
// Line 210: Rename initChart → initKlineChart, keep body identical
function initKlineChart() {
  if (!chartContainer.value) return

  chart = createChart(chartContainer.value, {
    width: chartContainer.value.clientWidth,
    height: 420,
    layout: {
      background: { color: '#272729' },
      textColor: '#86868b',
      fontSize: 10,
    },
    grid: {
      vertLines: { color: 'rgba(255,255,255,0.06)' },
      horzLines: { color: 'rgba(255,255,255,0.06)' },
    },
    crosshair: {
      mode: CrosshairMode.Normal,
    },
    timeScale: {
      timeVisible: period.value === 'intraday',
      secondsVisible: false,
      borderColor: 'rgba(255,255,255,0.06)',
      tickMarkFormatter: (time: Time) => formatTime(time as number, period.value),
    },
    rightPriceScale: {
      borderColor: 'rgba(255,255,255,0.06)',
      autoScale: true,
    },
    leftPriceScale: {
      visible: false,
    },
  })

  candleSeries = chart.addSeries(CandlestickSeries, {
    upColor: '#ff3b30',
    downColor: '#34c759',
    borderUpColor: '#ff3b30',
    borderDownColor: '#34c759',
    wickUpColor: '#ff3b30',
    wickDownColor: '#34c759',
    priceScaleId: 'right',
  })

  volumeSeries = chart.addSeries(HistogramSeries, {
    priceScaleId: 'volume',
    priceFormat: { type: 'volume' },
  })

  chart.priceScale('volume').applyOptions({
    scaleMargins: { top: 0.85, bottom: 0 },
    borderColor: 'rgba(255,255,255,0.06)',
  })

  chart.subscribeCrosshairMove((param: any) => {
    if (!param.point || !param.time || param.seriesData.size === 0) return
    const data = param.seriesData.get(candleSeries!)
    if (!data) return
    const preclose = quote.value?.preclose_px
    const changeRate = preclose ? ((data.close - preclose) / preclose * 100) : 0
    crosshair.value = {
      time: formatTime(data.time as number, period.value),
      open: data.open?.toFixed(2) || '-',
      high: data.high?.toFixed(2) || '-',
      low: data.low?.toFixed(2) || '-',
      close: data.close?.toFixed(2) || '-',
      volume: formatVolume((param.seriesData.get(volumeSeries!) as any)?.value || 0),
      change: changeRate >= 0 ? `+${changeRate.toFixed(2)}%` : `${changeRate.toFixed(2)}%`,
      changeNum: changeRate,
    }
  })
}
```

- [ ] **Step 2: Add new initIntradayChart function**

```typescript
function initIntradayChart() {
  if (!chartContainer.value) return

  chart = createChart(chartContainer.value, {
    width: chartContainer.value.clientWidth,
    height: 420,
    layout: {
      background: { color: '#272729' },
      textColor: '#86868b',
      fontSize: 10,
    },
    grid: {
      vertLines: { color: 'rgba(255,255,255,0.06)' },
      horzLines: { color: 'rgba(255,255,255,0.06)' },
    },
    crosshair: {
      mode: CrosshairMode.Normal,
    },
    timeScale: {
      timeVisible: false,
      secondsVisible: false,
      borderColor: 'rgba(255,255,255,0.06)',
      tickMarkFormatter: (time: Time) => {
        const idx = time as number
        return (idx >= 0 && idx < timeLabels.length) ? timeLabels[idx] : ''
      },
    },
    rightPriceScale: {
      borderColor: 'rgba(255,255,255,0.06)',
      autoScale: true,
      scaleMargins: { top: 0.05, bottom: 0.05 },
    },
    leftPriceScale: {
      visible: true,
      borderColor: 'rgba(255,255,255,0.06)',
      autoScale: true,
      scaleMargins: { top: 0.05, bottom: 0.05 },
    },
  })

  // Price line (left scale)
  priceSeries = chart.addSeries(LineSeries, {
    color: '#ffffff',
    lineWidth: 1,
    priceScaleId: 'left',
    crosshairMarkerVisible: true,
    crosshairMarkerRadius: 4,
    lastValueVisible: false,
    priceLineVisible: false,
  })

  // VWAP average price line (left scale)
  avgPriceSeries = chart.addSeries(LineSeries, {
    color: '#f5a623',
    lineWidth: 1,
    lineStyle: 2, // dashed
    priceScaleId: 'left',
    crosshairMarkerVisible: false,
    lastValueVisible: false,
    priceLineVisible: false,
  })

  // Hidden pct anchor (right scale)
  pctAnchorSeries = chart.addSeries(LineSeries, {
    lineWidth: 0,
    priceScaleId: 'right',
    lastValueVisible: false,
    priceLineVisible: false,
    crosshairMarkerVisible: false,
  })

  // Volume (separate scale at bottom)
  volumeSeries = chart.addSeries(HistogramSeries, {
    priceScaleId: 'volume',
    priceFormat: { type: 'volume' },
  })

  chart.priceScale('volume').applyOptions({
    scaleMargins: { top: 0.80, bottom: 0 },
    borderColor: 'rgba(255,255,255,0.06)',
  })

  chart.subscribeCrosshairMove((param: any) => {
    if (!param.point || param.time === undefined || param.seriesData.size === 0) return
    const idx = param.time as number
    if (idx < 0 || idx >= klineData.length) return
    const c = klineData[idx]
    const preclose = quote.value?.preclose_px
    const changeRate = preclose ? ((c.close - preclose) / preclose * 100) : 0

    // Get VWAP at this index
    let vwapVal = 0, cv = 0, cvol = 0
    for (let j = 0; j <= idx; j++) {
      cv += klineData[j].close * klineData[j].volume
      cvol += klineData[j].volume
    }
    vwapVal = cvol > 0 ? cv / cvol : c.close

    crosshair.value = {
      time: timeLabels[idx] || '',
      open: c.open?.toFixed(2) || '-',
      high: c.high?.toFixed(2) || '-',
      low: c.low?.toFixed(2) || '-',
      close: vwapVal?.toFixed(2) || '-',
      volume: formatVolume(c.volume || 0),
      change: changeRate >= 0 ? `+${changeRate.toFixed(2)}%` : `${changeRate.toFixed(2)}%`,
      changeNum: changeRate,
    }
  })
}
```

Crosshair bar for intraday shows: 时间 | 开 | 高 | 低 | 均 | 量 | 涨跌幅
Where "均" = VWAP, shown in the "close" field of crosshair since we reuse the same bar template.

- [ ] **Step 3: Run type check**

```bash
cd src && npx vue-tsc --noEmit
```

Expected: No errors.

- [ ] **Step 4: Commit**

```bash
git add src/src/components/KlineDrawer.vue
git commit -m "feat: add initIntradayChart with LineSeries + dual Y-axis"
```

---

### Task 4: Update renderChart for intraday branch

**Files:**
- Modify: `src/src/components/KlineDrawer.vue`

- [ ] **Step 1: Replace renderChart with branching logic**

```typescript
function renderChart() {
  if (!chartContainer.value) return

  if (period.value === 'intraday') {
    renderIntradayChart()
  } else {
    renderKlineChart()
  }
}

function renderIntradayChart() {
  if (klineData.length === 0) return

  // Build time labels
  timeLabels = buildTimeLabels(klineData)

  if (!chart) {
    initIntradayChart()
  }

  if (!priceSeries || !avgPriceSeries || !volumeSeries || !pctAnchorSeries) return

  priceSeries.setData(buildPriceData())
  avgPriceSeries.setData(buildVwapData())
  pctAnchorSeries.setData(buildPctAnchorData(quote.value?.preclose_px || 0))
  volumeSeries.setData(buildIntradayVolumeData())

  // Preclose price line on price series
  clearPriceLines()
  if (quote.value?.preclose_px && priceSeries) {
    priceLines.push(
      priceSeries.createPriceLine({
        price: quote.value.preclose_px,
        color: 'rgba(255,255,255,0.20)',
        lineStyle: 2,
        lineWidth: 1,
        axisLabelVisible: true,
        title: `昨收 ${quote.value.preclose_px.toFixed(2)}`,
      })
    )
  }

  chart!.timeScale().fitContent()
}

function renderKlineChart() {
  if (!chart) {
    initKlineChart()
  }

  if (!candleSeries || !volumeSeries) return

  candleSeries.setData(buildCandleData())
  volumeSeries.setData(buildVolumeData())
  updatePriceLines()
  updateAnchorSeries()
  chart!.timeScale().fitContent()
}
```

- [ ] **Step 2: Update updateCrosshairFromData for intraday**

```typescript
function updateCrosshairFromData() {
  if (klineData.length > 0) {
    const last = klineData[klineData.length - 1]
    const preclose = quote.value?.preclose_px
    const changeRate = preclose ? ((last.close - preclose) / preclose * 100) : 0

    if (period.value === 'intraday') {
      // Compute VWAP for last point
      let cv = 0, cvol = 0
      for (let j = 0; j < klineData.length; j++) {
        cv += klineData[j].close * klineData[j].volume
        cvol += klineData[j].volume
      }
      const vwap = cvol > 0 ? cv / cvol : last.close
      crosshair.value = {
        time: timeLabels.length > 0 ? timeLabels[timeLabels.length - 1] : '',
        open: last.open?.toFixed(2) || '-',
        high: last.high?.toFixed(2) || '-',
        low: last.low?.toFixed(2) || '-',
        close: vwap?.toFixed(2) || '-',
        volume: formatVolume(last.volume || 0),
        change: changeRate >= 0 ? `+${changeRate.toFixed(2)}%` : `${changeRate.toFixed(2)}%`,
        changeNum: changeRate,
      }
    } else {
      crosshair.value = {
        time: formatTime(last.time, period.value),
        open: last.open?.toFixed(2) || '-',
        high: last.high?.toFixed(2) || '-',
        low: last.low?.toFixed(2) || '-',
        close: last.close?.toFixed(2) || '-',
        volume: formatVolume(last.volume || 0),
        change: changeRate >= 0 ? `+${changeRate.toFixed(2)}%` : `${changeRate.toFixed(2)}%`,
        changeNum: changeRate,
      }
    }
  }
}
```

- [ ] **Step 3: Update the crosshair template label for intraday**

```vue
<!-- Line 37: Change "收" to "均" when intraday -->
<span>收 <span class="text-white">{{ crosshair.close }}</span></span>
```
→
```vue
<span>{{ period === 'intraday' ? '均' : '收' }} <span class="text-white">{{ crosshair.close }}</span></span>
```

- [ ] **Step 4: Update updatePriceLines for kline-only and add period guard**

```typescript
// Replace 'minute' check — updatePriceLines and updateAnchorSeries now guard on period
function updatePriceLines() {
  if (!candleSeries) return
  clearPriceLines()

  // Only for K-line mode (not intraday)
  if (period.value === 'intraday' || !quote.value?.preclose_px) return

  const preclose = quote.value.preclose_px
  const rate = getLimitRate()
  const limitUp = Math.round(preclose * (1 + rate) * 100) / 100
  const limitDown = Math.round(preclose * (1 - rate) * 100) / 100

  priceLines.push(
    candleSeries.createPriceLine({
      price: preclose,
      color: 'rgba(255,255,255,0.20)',
      lineStyle: 2,
      lineWidth: 1,
      axisLabelVisible: true,
      title: `昨收 ${preclose.toFixed(2)}`,
    }),
    candleSeries.createPriceLine({
      price: limitUp,
      color: '#ff3b30',
      lineStyle: 2,
      lineWidth: 1,
      axisLabelVisible: true,
      title: `涨停 ${limitUp.toFixed(2)}`,
    }),
    candleSeries.createPriceLine({
      price: limitDown,
      color: '#34c759',
      lineStyle: 2,
      lineWidth: 1,
      axisLabelVisible: true,
      title: `跌停 ${limitDown.toFixed(2)}`,
    }),
  )
}

function updateAnchorSeries() {
  if (!chart || !candleSeries) return

  if (anchorSeries) {
    chart.removeSeries(anchorSeries)
    anchorSeries = null
  }

  // Only for K-line mode
  if (period.value === 'intraday' || !quote.value?.preclose_px || klineData.length === 0) return

  const preclose = quote.value.preclose_px
  let maxUpper = 0, maxLower = 0
  for (const c of klineData) {
    if (c.high > preclose) maxUpper = Math.max(maxUpper, c.high - preclose)
    if (c.low < preclose) maxLower = Math.max(maxLower, preclose - c.low)
  }
  const range = Math.max(maxUpper, maxLower) * 1.15 || preclose * 0.02
  const yMin = preclose - range
  const yMax = preclose + range

  anchorSeries = chart.addSeries(LineSeries, {
    lineWidth: 0,
    priceScaleId: candleSeries.priceScaleId(),
    lastValueVisible: false,
    priceLineVisible: false,
    crosshairMarkerVisible: false,
  })

  const firstTime = klineData[0].time as Time
  const lastTime = klineData[klineData.length - 1].time as Time
  anchorSeries.setData([
    { time: firstTime, value: yMin },
    { time: lastTime, value: yMax },
  ])
}
```

- [ ] **Step 5: Update clearPriceLines to handle both series types**

```typescript
function clearPriceLines() {
  const series = period.value === 'intraday' ? priceSeries : candleSeries
  if (!series) return
  for (const line of priceLines) {
    series.removePriceLine(line)
  }
  priceLines = []
}
```

- [ ] **Step 6: Run type check**

```bash
cd src && npx vue-tsc --noEmit
```

Expected: No errors.

- [ ] **Step 7: Commit**

```bash
git add src/src/components/KlineDrawer.vue
git commit -m "feat: wire up intraday renderChart with price/VWAP/volume"
```

---

### Task 5: Period switching — destroy and recreate chart

**Files:**
- Modify: `src/src/components/KlineDrawer.vue`

- [ ] **Step 1: Add resetChart function and update period watcher**

```typescript
function resetChart() {
  clearPriceLines()
  if (chart) {
    chart.remove()
    chart = null
    candleSeries = null
    priceSeries = null
    avgPriceSeries = null
    pctAnchorSeries = null
    volumeSeries = null
    anchorSeries = null
  }
}

// Replace: watch(period, () => fetchKline())
watch(period, () => {
  resetChart()
  fetchKline()
})
```

- [ ] **Step 2: Update onUnmounted to clean up new series**

```typescript
onUnmounted(() => {
  clearPriceLines()
  chart?.remove()
  chart = null
  candleSeries = null
  priceSeries = null
  avgPriceSeries = null
  pctAnchorSeries = null
  volumeSeries = null
  anchorSeries = null
})
```

- [ ] **Step 3: Run type check**

```bash
cd src && npx vue-tsc --noEmit
```

Expected: No errors.

- [ ] **Step 4: Commit**

```bash
git add src/src/components/KlineDrawer.vue
git commit -m "feat: destroy and recreate chart on period switch"
```

---

### Task 6: Edge cases and empty state

**Files:**
- Modify: `src/src/components/KlineDrawer.vue`

- [ ] **Step 1: Add empty state in renderIntradayChart**

```typescript
function renderIntradayChart() {
  if (klineData.length === 0) {
    if (chart) {
      resetChart()
    }
    crosshair.value = { time: '暂无分时数据', open: '-', high: '-', low: '-', close: '-', volume: '-', change: '', changeNum: 0 }
    return
  }
  // ... rest of renderIntradayChart
}
```

- [ ] **Step 2: Guard renderKlineChart similarly**

```typescript
function renderKlineChart() {
  if (klineData.length === 0) {
    if (chart) resetChart()
    return
  }
  // ... rest
}
```

- [ ] **Step 3: Build and type check**

```bash
cd src && npx vue-tsc --noEmit && npx vite build
```

Expected: Both pass.

- [ ] **Step 4: Commit**

```bash
git add src/src/components/KlineDrawer.vue
git commit -m "fix: handle empty data and edge cases in intraday chart"
```

---

### Task 7: Final verification build

**Files:**
- Verify: `src/src/components/KlineDrawer.vue`

- [ ] **Step 1: Full type check + production build**

```bash
cd src && npx vue-tsc --noEmit && npx vite build
```

Expected: Both pass with no errors.

- [ ] **Step 2: Check the file is self-consistent**

```bash
wc -l src/src/components/KlineDrawer.vue
```

Expected: ~550-600 lines.

- [ ] **Step 3: Commit if any lint fixes needed, otherwise done**

No commit needed unless cleanup occurred.
