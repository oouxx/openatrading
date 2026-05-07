# Apple Design System Redesign — Spec

**Date:** 2026-05-07
**Source:** Apple DESIGN.md (VoltAgent/awesome-design-md)
**Scope:** Vue 3 SPA frontend only (`src/`)

## Design Direction

Light shell + dark data surfaces. Single blue accent for interactions, red/green reserved for data semantics (涨跌). Full-width edge-to-edge tiles separated by background color change. Inter font (SF Pro substitute) + JetBrains Mono for data.

---

## Design Tokens

### Colors

| Token | Value | Usage |
|-------|-------|-------|
| `canvas` | `#ffffff` | Page background, KPI cards |
| `canvas-parchment` | `#f5f5f7` | Alternating sections, Tab bar, footer |
| `surface-dark` | `#272729` | Dark data tiles (tables, chart containers) |
| `surface-dark-2` | `#2a2a2c` | Adjacent dark tile micro-variation |
| `ink` | `#1d1d1f` | Headlines, body text |
| `muted` | `#86868b` | Secondary text, captions |
| `hairline` | `rgba(0,0,0,0.08)` | Card 1px borders on light surfaces |
| `hairline-dark` | `rgba(255,255,255,0.06)` | Borders on dark surfaces |
| `primary` | `#0066cc` | All interactive elements (buttons, links, active states) |
| `primary-focus` | `#0071e3` | Focus ring |
| `green-data` | `#34c759` | Price up (涨) |
| `red-data` | `#ff3b30` | Price down (跌) |

### Typography (Inter + JetBrains Mono)

| Token | Size/Weight/Line/Spacing | Usage |
|-------|--------------------------|-------|
| `display-lg` | 40px / 600 / 1.1 / -0.5px | Section headlines |
| `display-md` | 28px / 600 / 1.14 / -0.3px | Sub-section heads |
| `body` | 17px / 400 / 1.47 / -0.3px | Paragraph text |
| `body-strong` | 17px / 600 / 1.24 / -0.3px | Emphasized paragraphs |
| `caption` | 14px / 400 / 1.43 / -0.2px | Secondary text, button labels |
| `caption-strong` | 14px / 600 / 1.29 / -0.2px | Labels, emphasized captions |
| `nav-link` | 12px / 400 / 1.0 / -0.1px | Navigation items |
| `data` | 13px / 400 / 1.6 / 0 | Data tables (JetBrains Mono) |

### Border Radius

| Token | Value | Usage |
|-------|-------|-------|
| `rounded-card` | 18px | Content cards, data cards |
| `rounded-button` | 9999px | Buttons (pill shape) |
| `rounded-input` | 18px | Inputs |
| `rounded-badge` | 5px | Badges, tags |
| `rounded-tile` | 0px | Full-width tiles (no rounding) |

### Spacing

| Token | Value | Usage |
|-------|-------|-------|
| `section-y` | 80px | Vertical padding inside tiles |
| `card-padding` | 24px | Internal card padding |
| `card-gap` | 24px | Gap between cards in grids |

---

## Layout Structure

### Full-Width Tile System

Tiles stack edge-to-edge with zero gap. Background color change is the section divider.

```
┌────────────────────────────────────────┐
│ Nav: 000000, 44px, fixed, z-50        │
├────────────────────────────────────────┤
│ KPI Tile: #ffffff, 80px py            │
│   content ~980px centered             │
├────────────────────────────────────────┤
│ Tab Bar: #f5f5f7, sticky top 44px     │
├────────────────────────────────────────┤
│ Data Tile: #272729, 80px py           │
│   content ~1200px centered            │
├────────────────────────────────────────┤
│ Footer: #f5f5f7, 64px py             │
└────────────────────────────────────────┘
```

### Rules

- Zero gap between tiles — color change alone provides separation
- Full-width tiles have `0px` border-radius
- Cards INSIDE tiles use 18px radius + 1px hairline border
- No box-shadows on cards (exception: none in this project)
- Content max-width: 980px text-heavy, 1200px data grids

---

## Components

### Global Nav `.global-nav`

- Background `#000000`, height 44px, fixed top, z-50
- Left: brand "TRENDING.AGENT" (Inter 600, 14px, `-0.2px` letter-spacing)
- Right: data-source badge, date string (JetBrains Mono), refresh button
- Refresh button: `button-dark-utility` (ink bg, white text, 8px radius, 14px/400)

### KPI Cards `.kpi-card`

- Background `canvas`, 18px radius, 1px `hairline` border, 24px padding
- Left 3px colored stripe: green (rise), red (fall), blue (ratio), none (bomb rate)
- Label: `caption` 14px/400, `muted` color
- Value: 28px/600, JetBrains Mono, `ink` color
- Grid: 4 columns desktop, 2 tablet, 1 mobile

### Risk Bar `.risk-bar`

- Background `canvas`, 18px radius, 1px hairline, 16px×24px padding
- Left: risk pill badge
  - 高: `rgba(255,59,48,0.1)` bg, `red-data` text
  - 中: `rgba(255,59,48,0.06)` bg, `red-data` text
  - 低: `rgba(52,199,89,0.1)` bg, `green-data` text
- Right: decision summary in `body` 17px/400, `muted` color

### Tab Bar `.tab-bar`

- Background `canvas-parchment`, sticky at top 44px
- Tab items: `caption` 14px/400, `muted` color
- Active: `ink` color + bottom 2px `primary` blue line
- No emoji prefixes — clean text labels
- Labels: 涨停数据 / 板块异动 / 晋级 / 时间线 / 舆情分析 / AI 观点 / 淘股吧

### Data Table `.data-table`

- Contained inside dark tile (`surface-dark` #272729)
- Header: 11px/600, uppercase, `muted` color
- Rows: 13px/400 JetBrains Mono
- Row divider: `hairline-dark`
- Hover: `rgba(255,255,255,0.04)` bg
- 18px radius, 24px padding, 1px `hairline-dark` border

### K-line Drawer `.kline-drawer`

- Right slide-out, 520px wide, `surface-dark` background
- Sticky header: stock name + code + realtime price
- Period toggle: pill-shaped button group (transparent → blue fill when active)
- Chart: ECharts dark theme
- Metrics grid: 3-column mini-cards below chart

### Buttons

| Variant | Background | Text | Border | Radius | Padding |
|---------|-----------|------|--------|--------|---------|
| `button-primary` | `primary` #0066cc | white | none | 9999px | 11px×22px |
| `button-secondary` | transparent | `primary` | 1px `primary` | 9999px | 11px×22px |
| `button-dark-utility` | `ink` #1d1d1f | white | none | 8px | 8px×15px |
| `text-link` | transparent | `primary` | none | — | — |

Active state: `transform: scale(0.95)` on all buttons.

### Sentiment Cards (Articles/Stocks)

- Background `canvas`, 18px radius, 24px padding, 1px hairline
- Article rows: `rgba(0,0,0,0.02)` bg, 1px hairline, expandable
- Stock ranking: rank number + code + name + heat count, hover pale blue bg

### Promotion Ladder

- Background `canvas`, 18px radius, 24px padding
- Row: yesterday label → promoted today label → progress bar → rate
- Progress bar: `rgba(0,0,0,0.06)` track, green (≥50%) or muted (<50%) fill
- Click to expand detail table (dark mini-table)

### Empty / Loading / Error States

- Loading: centered spinner + "加载数据..." in `muted` caption
- Error: light red bg card with error message + retry link
- Empty: centered muted text "暂无数据"

---

## Implementation Approach

### Strategy A: Tailwind-first

- Define all design tokens in `tailwind.config.cjs` under `theme.extend`
- Replace inline `style` bindings with Tailwind utility classes
- Remove CSS variable usage from `style.css` (keep only Tailwind directives + font import + base resets)
- Components rewritten to use semantic Tailwind classes

### Implementation Order

1. `tailwind.config.cjs` — design token definitions
2. `style.css` — base reset, font import, remove old variables
3. `App.vue` — layout shell (nav + tile structure)
4. `KpiCards.vue` — KPI cards + risk bar
5. `TabBar.vue` — tab navigation
6. `TabLimitUp.vue` — first data tab (establishes table patterns)
7. Remaining tabs: `TabPlate`, `TabPromotion`, `TabSentiment`, `TabTimeline`, `TabAi`, `TabTgb`
8. `KlineDrawer.vue` — chart drawer
9. Polish: loading, error, empty states

---

## What Does NOT Change

- `crates/core/templates/html/` — server-side Askama templates
- Data fetching logic (`stores/report.ts`)
- ECharts integration (only theme colors updated)
- Tab component logic (only styling)
- Vue component structure (same files, same props, same emits)
