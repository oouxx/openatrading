# Vue Frontend Implementation Plan

**Feature branch:** `feature/workspace-restructure`
**Worktree path:** `/Users/wxx/Desktop/github/openatrading-workspace-restructure`
**Design doc:** `docs/superpowers/specs/2026-05-04-vue-frontend-design.md`

---

## Overview

Implement Vue3 + TypeScript frontend in `src/` to clearly display all data from the openatrading core:
- Market overview (KPI cards)
- Limit-up stocks (leaderboard)
- Plate abnormal movements
- Sentiment analysis (JiuYan community)
- AI three-role analysis
- Tgb methodology posts

Reference style: xuangutong.com.cn (clean info-flow layout)

---

## Task List

### 1. Install Dependencies
- [ ] **Pinia** (state management): `cd src && npm install pinia`
- [ ] **Tailwind CSS**: `npm install -D tailwindcss @tailwindcss/vite`
- [ ] Configure `tailwind.config.js` + `postcss.config.js`
- [ ] Update `src/src/style.css` to include Tailwind directives

### 2. Create Pinia Store (`src/stores/report.ts`)
- [ ] Define `Report` interface matching `crates/core/src/report/report.rs` struct
- [ ] State: `report: Ref<Report | null>`, `loading: Ref<boolean>`
- [ ] Actions: `fetchReport(format?: string)`, `clearReport()`
- [ ] For Tauri: import from `../api/tauri`
- [ ] For Web: import from `../api/web`

### 3. Update `src/src/main.ts`
- [ ] Import `createPinia` and mount it
- [ ] Import `App.vue`
- [ ] Create app instance with Pinia plugin

### 4. Implement `src/src/App.vue`
- [ ] **Template structure:**
  - Top Nav (dark, fixed)
  - Hero Section (4 KPI cards)
  - Tab Bar (5 tabs: 涨停榜/板块异动/舆情分析/AI观点/淘股吧)
  - Tab Content (dynamic component via `<component :is="currentTab">`)
  - Footer
- [ ] **Script setup:** Pinia store consumption, tab switching logic

### 5. Implement KPI Cards (`src/src/components/KpiCards.vue`)
- [ ] Props: `overview: MarketOverview`
- [ ] 4 cards: 涨停家数, 跌停家数, 炸板率, 市场情绪
- [ ] Color indicators: red/green for rise/fall, colored dot for sentiment

### 6. Implement Tab Components (`src/src/components/tabs/`)
- [ ] `TabLimitUp.vue` - Limit-up table (stock code, name, change%, turnover, days)
- [ ] `TabPlate.vue` - Plate abnormal events list + sector heat bar (CSS)
- [ ] `TabSentiment.vue` - Hot stocks TOP10 + articles list + trend tags
- [ ] `TabAi.vue` - 3 role cards (游资操盘手/量化分析师/风控官)
- [ ] `TabTgb.vue` - User methodology list (expandable)

### 7. Global Styling (`src/src/style.css`)
- [ ] Tailwind base + custom dark nav
- [ ] Card shadows, hover effects
- [ ] Table stripes, tag colors (red/green/blue)
- [ ] Responsive adjustments for Tauri window

### 8. Update `src/index.html`
- [ ] Meta viewport, title "A股市场异动追踪"
- [ ] Link to Vite dev server in dev mode

### 9. Test & Verify
- [ ] `cd src && npm run dev` - check Vite dev server
- [ ] Verify KPI cards render with mock data
- [ ] Verify Tab switching works
- [ ] Verify each tab renders correctly with mock data

---

## File Structure (Final)

```
src/
├── index.html
├── package.json
├── vite.config.ts
├── tailwind.config.js
├── postcss.config.js
├── src/
│   ├── main.ts
│   ├── App.vue
│   ├── style.css
│   ├── stores/
│   │   └── report.ts
│   ├── components/
│   │   ├── KpiCards.vue
│   │   ├── TabBar.vue
│   │   ├── TabContent.vue
│   │   └── tabs/
│   │       ├── TabLimitUp.vue
│   │       ├── TabPlate.vue
│   │       ├── TabSentiment.vue
│   │       ├── TabAi.vue
│   │       └── TabTgb.vue
│   └── api/
│       ├── index.ts
│       ├── tauri.ts
│       └── web.ts
└── ...
```

---

## Mock Data Strategy

Since Tauri/Tauri commands aren't fully set up yet, use **mock data** matching the Rust `Report` struct to develop frontend independently:

```typescript
// src/stores/report.ts
const mockReport = {
  date: "2026-05-04",
  overview: { rise_count: 120, fall_count: 30, bomb_rate: 0.25, sentiment: "中" },
  limit_up: [/* ... */],
  // ... other fields
}
```

---

## Next Steps After Implementation

1. Commit all Vue frontend changes to `feature/workspace-restructure`
2. Set up Tauri commands to invoke core functions
3. Connect real data flow: Tauri → API layer → Pinia store
4. Test in Tauri desktop app
5. Merge to main when complete
