# Frontend Design Document — OpenATrading

## 1. Overview

OpenATrading is an **AI-native A-share short-term trading decision workstation**. The frontend is a single-page React application that can run as a web app (Vercel/Cloudflare Pages), a Tauri v2 desktop app (macOS/Linux/Windows), or a local development server (Vite).

**Design direction**: Bloomberg Terminal × A-share swing trader recap × AI Dashboard. High information density, low visual noise, terminal-like professional feel.

---

## 2. Technology Stack

| Layer | Choice | Version |
|-------|--------|---------|
| Framework | React | ^19.1 |
| Language | TypeScript (strict) | ~5.7 |
| Bundler | Vite | ^6.0 |
| Routing | react-router-dom | ^7.6 |
| State | Zustand | ^5.0 |
| Animation | framer-motion | ^12.4 |
| Charts | lightweight-charts | ^5.2 |
| Markdown | react-markdown + remark-gfm | ^10.1 / ^4.0 |
| CSS | Tailwind CSS | ^3.4 |
| Package manager | Bun | latest |
| Desktop | Tauri v2 | ^2.11 |

---

## 3. Directory Structure

```
src/
├── index.html                  # SPA entry point
├── package.json
├── vite.config.ts              # Vite config (path aliases, proxies, defines)
├── tailwind.config.cjs          # Custom design tokens (colors, fonts, radii, shadows)
├── postcss.config.cjs
├── tsconfig.json                # Project references pattern
├── tsconfig.app.json
├── tsconfig.node.json
├── public/
│   ├── _headers                 # Cloudflare Pages security headers
│   ├── _redirects               # SPA fallback: /* → /index.html
│   └── vercel.json              # Vercel rewrites + API proxy
└── src/
    ├── main.tsx                 # React entry: createRoot → BrowserRouter → App
    ├── App.tsx                  # Root layout: header nav + Routes
    ├── style.css                # Tailwind layers + custom scrollbar/skeleton/table styles
    ├── env.d.ts                 # Build-time constant type declarations
    ├── LandingPage.tsx          # Marketing/splash page (/)
    ├── TradingView.tsx          # Main trading terminal (/trading)
    ├── RoleView.tsx             # AI role strategy management (/roles)
    ├── AiCopilot.tsx            # Full-page AI chat (/copilot)
    ├── ReplayView.tsx           # Market K-line replay (/replay)
    ├── stores/
    │   └── data.ts              # Zustand store: all global state + API fetch layer
    ├── components/
    │   ├── AiSidebar.tsx        # Embedded AI chat sidebar in TradingView
    │   ├── KpiCards.tsx         # Market sentiment KPI card grid (5 cards)
    │   ├── TabBar.tsx           # 7-tab navigation bar
    │   ├── NewsFeed.tsx         # Real-time financial news (CaiLianShe / JiuYanGongShe)
    │   ├── MiniChartPreview.tsx # Hover-to-preview stock GIF chart popup
    │   ├── KlineDrawer.tsx      # Slide-in drawer: K-line chart + stock details
    │   ├── Sparkline.tsx        # Canvas-based PnL sparkline (quadratic bezier, HiDPI)
    │   ├── SentimentHistory.tsx # Lightweight-charts sentiment history line chart
    │   └── tabs/
    │       ├── TimelineTab.tsx       # Market timeline: today anomalies + monthly events
    │       ├── SentimentChipTab.tsx  # Sentiment cycle + chip structure + history table
    │       ├── MarketIntelTab.tsx    # Hot stocks, hot articles, forum aggregation
    │       └── PromotionTab.tsx      # Consecutive limit-up ladder analysis
    ├── composables/
    │   └── useSpeech.ts         # TTS custom hook
    └── assets/
        ├── app-icon.png
        └── openatrading-logo.png
```

---

## 4. Routing

| Route | Component | Description |
|-------|-----------|-------------|
| `/` | `LandingPage` (web) / `TradingView` (Tauri) | Marketing page or direct trading terminal |
| `/trading` | `TradingView` | Main trading terminal with AI sidebar |
| `/roles` | `RoleView` | Multi-role AI strategy management |
| `/copilot` | `AiCopilot` | Full-page AI copilot chat |
| `/replay` | `ReplayView` | Market data K-line replay |

Navigation: hardcoded `NavLink` header in `App.tsx` with links to `/trading`, `/replay`, `/roles`, `/copilot`. Brand "OpenATrading" links to `/`.

**Tauri detection**: `'__TAURI_INTERNALS__' in window` — when true, the root route renders `TradingView` directly instead of `LandingPage`, and the header brand becomes a non-navigable `<span>`.

---

## 5. Component Architecture

### Component Tree

```
<BrowserRouter>
  <App>
    <header>                    # Fixed top nav: brand + NavLinks
    <Routes>
      / → <LandingPage />       # (or TradingView in Tauri)
      /trading → <TradingView>
                    ├── <KpiCards />           # 5 animated market KPI cards
                    ├── <TabBar />             # 7-tab navigation
                    ├── <div.main-content>     # Conditional: active tab component
                    │     ├── strong-stocks    → inline table
                    │     ├── promotion        → <PromotionTab />
                    │     ├── plate            → <PlateTab /> (local)
                    │     ├── timeline         → <TimelineTab />
                    │     ├── sentiment-chip   → <SentimentChipTab />
                    │     ├── market-intel     → <MarketIntelTab />
                    │     └── news             → <NewsFeed variant="tab" />
                    ├── <AiSidebar />          # AI chat (340px right sidebar)
                    └── <KlineDrawer />        # Slide-in stock detail drawer
      /roles → <RoleView>
                 ├── Sidebar (320px): role list + PnL sparklines
                 ├── Detail panel: PnL chart, strategy editor, positions, logs
                 └── Modals: create / delete / reset role
      /copilot → <AiCopilot />
      /replay → <ReplayView />
```

### Page Components

**LandingPage** — Marketing/splash page. Feature cards (multi-role AI, auto data collection & decisions, Telegram interaction), desktop client downloads (macOS Intel/Apple Silicon, Linux, Windows), CLI install instructions, GitHub link.

**TradingView** — Core trading terminal. Index quote grid (3-column), KPI dashboard, 7-tab data analysis panel, AI sidebar (desktop: fixed 340px right; mobile: full-screen overlay), stock K-line slide-in drawer.

**RoleView** — AI role strategy management. Left sidebar (320px) with role list showing PnL sparklines and position counts. Right detail panel with: PnL curve chart (7d/30d/all), editable strategy textarea, equity stats, current positions table, trade log, closed-positions history. Modal dialogs for create/delete/reset role (framer-motion AnimatePresence).

**AiCopilot** — Full-page AI chat. Message list with streaming responses rendered via react-markdown + remark-gfm. Suggested questions grid on empty state. Input with Enter-to-send / Shift+Enter newline. API communication via SSE (web) or Tauri Channel (desktop).

**ReplayView** — Market K-line replay. Symbol input, play/pause controls with speed multiplier (1x/2x/4x/8x), progress slider, candlestick chart + volume histogram using lightweight-charts. Animated bar-by-bar playback from backend API. Color scheme: red-up, green-down (Chinese market convention).

### Shared Components

- **KpiCards** — 5 animated cards: 情绪温度, 涨停家数, 炸板率, 昨日涨停表现, 涨跌比. Plus risk-level badge and decision summary banner.
- **TabBar** — 7-tab navigation with framer-motion hover/tap animations. Tabs: 强势股, 晋级分析, 板块异动, 时间线, 情绪筹码, 市场情报, 资讯.
- **AiSidebar** — Embedded chat panel. Role selector dropdown, message history with streaming, react-markdown rendering. Uses `apiChatStream` dual-mode (SSE / Tauri IPC).
- **MiniChartPreview** — Hover-triggered stock chart popup loaded from Sina Finance GIF (`image.sinajs.cn`). Click-to-enlarge with scroll-zoom.
- **KlineDrawer** — Animated slide-in panel (520px). Multi-period chart selector (分时/日K/周K/月K) from Sina Finance, real-time quote data, related plates list, 12-metric grid (open/close/high/low/amplitude/turnover/PE/PB/market cap/etc.).
- **Sparkline** — Canvas-based PnL mini chart. Quadratic bezier curves, HiDPI-aware.
- **NewsFeed** — Real-time news from CaiLianShe / JiuYanGongShe. Source filtering (all/cls/jiuyan), severity indicators.
- **SentimentHistory** — Lightweight-charts line chart with auto-interpretation.

---

## 6. State Management

**Single Zustand v5 store** (`src/stores/data.ts`): `useDataStore`.

### Data Groups (each with `{ data, loading, error }`):

| Group | Data |
|-------|------|
| `overview` | Market overview KPI, risk level, decision summary |
| `limitUp` | Today's limit-up stocks |
| `fieldItems` | Sector/plate analysis |
| `plateAbnormal` | Abnormal plate movements |
| `timeline` | Market timeline events |
| `sentiment` | Sentiment data (JiuYan / Taoguba) |
| `indexQuotes` | Major index quotes |
| `sentimentHistory` | Historical sentiment data |
| `yesterdayLimitUp` | Yesterday's limit-up stocks |
| `roles` | List of AI trading roles |
| `rolePositions` | Per-role current positions (keyed by role name) |
| `roleLogs` | Per-role trade logs |
| `rolePnlHistory` | Per-role PnL history points |
| `roleHistory` | Per-role closed positions |

### Action Pattern

All async actions follow: `set loading → try fetchJson → set data, clear error → catch set error`. Mutations (createRole, updateRoleStrategy, deleteRole, resetRole) follow the same pattern with POST/PUT/DELETE requests.

---

## 7. API Layer

Dual-mode architecture co-located in `stores/data.ts`:

### Core Functions

- **`getApiBaseUrl()`** — Returns `''` in Tauri, or `localStorage` / `__API_BASE_URL__` in browser.
- **`fetchJson<T>(path, init?)`** — Typed fetch. In Tauri: delegates to `invoke('api_fetch'/'api_post')`. In browser: uses `fetch()` with base URL.
- **`apiFetch(path, init?)`** — Raw Response version for components that need direct stream/response access.
- **`apiChatStream(body, callbacks)`** — SSE streaming. Browser: `fetch()` with `ReadableStream`, parses `data:` SSE lines. Tauri: `Channel<string>` IPC with `invoke('api_chat_stream')`.

### Backend Endpoints

All proxied through Vite dev server (dev) or Tauri IPC (desktop):

| Endpoint | Method | Consumer |
|----------|--------|----------|
| `/api/overview` | GET | TradingView (KPI cards) |
| `/api/limit-up` | GET | TradingView (strong stocks tab) |
| `/api/field-items?date=` | GET | TradingView (sector tab) |
| `/api/plate-abnormal` | GET | TimelineTab |
| `/api/timeline` | GET | TimelineTab |
| `/api/sentiment` | GET | MarketIntelTab |
| `/api/index-quotes` | GET | TradingView (index grid) |
| `/api/sentiment-history?days=` | GET | SentimentChipTab |
| `/api/yesterday-limit-up` | GET | PromotionTab |
| `/api/roles` | GET/POST | RoleView |
| `/api/roles/{name}` | DELETE | RoleView |
| `/api/roles/{name}/positions` | GET | RoleView |
| `/api/roles/{name}/logs` | GET | RoleView |
| `/api/roles/{name}/pnl-history?days=` | GET | RoleView |
| `/api/roles/{name}/history?limit=` | GET | RoleView |
| `/api/roles/{name}/strategy` | PUT | RoleView |
| `/api/roles/{name}/reset` | POST | RoleView |
| `/api/ai/chat` | POST (SSE) | AiCopilot, AiSidebar |
| `/api/ai/roles` | GET | AiSidebar |
| `/api/telegraph` | GET | NewsFeed |
| `/api/kline/{symbol}?period=&count=` | GET | ReplayView, KlineDrawer |
| `/api/realtime/{symbol}` | GET | KlineDrawer |
| `/api/plates-by-any-stock/{symbol}` | GET | KlineDrawer |

### Vite Proxy Configuration

- `/api/xgb/*` → `https://flash-api.xuangubao.cn/api/*`
- `/api/*` → `http://localhost:8080`

### Build-Time Defines

Injected from parent directory `.env` into `vite.config.ts`: `__OPENROUTER_KEY__`, `__JIUYAN_PHONE__`, `__JIUYAN_PASSWORD__`, `__API_BASE_URL__`.

---

## 8. Design System

### Principles

1. High information density
2. Low visual noise
3. AI-native feel
4. Professional terminal aesthetic
5. Strong visual hierarchy
6. Module cross-linking

### Color Tokens (Tailwind config)

```
bg:            #0d0d1a    (background)
bg-surface:    #111124    (card surfaces)
bg-hover:      #1a1a35    (hover states)
ink:           #e8e8ed    (primary text)
ink-muted:     #888780    (muted/secondary text)
up:            #FF5C5C    (up/rise — Chinese market convention: red = up)
down:          #1D9E75    (down/fall — green = down)
ai:            #7F77DD    (AI accent)
ai-focus:      #9990f0    (AI accent focused)
hairline:      rgba(white, 6%)  (borders)
```

### Typography

- **Fonts**: Inter (sans-serif), JetBrains Mono (monospace)
- **Financial numbers**: Always `tabular-nums`, monospaced digits
- **Size scale**: `display-lg` (40px), `display-md` (28px), `body` (17px), `caption` (14px), `nav-link` (12px)

### Animation

- `framer-motion` for: KPI card entrance (staggered), tab hover/tap, modal dialogs (scale/fade), K-line drawer slide-in, AI message entrance, suggested question buttons
- **Constraint**: No entertainment animations. No cartoonish UI. Motion serves data perception, not decoration.

### Custom CSS Classes (style.css)

- `data-table` — Financial data table styling
- `skeleton` — Shimmer loading animation
- Custom scrollbar — Dark-themed, subtle
- `::selection` — AI accent purple

### Prohibited

- White backgrounds / white financial webpage style
- Entertainment animations / cartoonish UI
- Generic admin panel / SaaS dashboard style
- Low-density whitespace design
- Traditional F10 pages, full brokerage systems, news portals, traditional quant platforms

---

## 9. Build & Deployment

### Scripts (package.json)

```
dev               → vite
build             → tsc -b && vite build
build:vercel      → vite build
build:cloudflare  → vite build
build:tauri       → vite build
preview           → vite preview
```

### Deployment Targets

| Platform | Config | Notes |
|----------|--------|-------|
| **Cloudflare Pages** | `public/_headers`, `public/_redirects` | Cache/security headers, SPA fallback `/* → /index.html` |
| **Vercel** | `public/vercel.json` | API rewrites proxy to production backend, SPA fallback |
| **Tauri Desktop** | `src-tauri/tauri.conf.json` | Window 1400×900, CSP, macOS/Linux/Windows bundles |
| **Dev** | Vite dev server | Proxy: `/api/*` → `localhost:8080` |

### Path Alias

`@` → `./src` (configured in both `vite.config.ts` and `tsconfig.app.json`)

---

## 10. Key Design Patterns & Decisions

### Dual-Mode API (Browser ↔ Tauri)

The single biggest architectural decision: the entire API layer transparently switches between HTTP fetch and Tauri IPC via runtime detection. This allows a single codebase to serve both web and desktop with zero code forks in components.

### Co-located Store + API Layer

The Zustand store (`data.ts`) contains both state AND all API functions. There is no separate service layer, API client module, or repository pattern. This is a deliberate simplicity choice — API responses map 1:1 to store slices.

### Tab-Based Data Navigation

TradingView uses a flat tab bar rather than nested routes. Active tab determines which data-fetching component renders. Each tab fetches its own data independently (no pre-fetching).

### AI as First-Class UI Citizen

The AI copilot is not a floating chat bubble — it's a persistent sidebar (340px) in TradingView and a full-page view at `/copilot`. Responses stream in real-time via SSE, rendered as GitHub-flavored markdown. AI is positioned as "co-pilot", not chatbot — it should proactively explain, warn, and summarize.

### Data Everywhere, Errors Recoverable

Every data display component handles four states: loading (skeleton/spinner), data, empty ("暂无数据"), error (banner + retry button). This is applied consistently across KPI cards, tables, charts, and news feeds.
