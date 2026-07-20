# Role Strategy Page Redesign — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Redesign `/roles` page to split-pane layout (role list + detail), add role CRUD, account reset, and P&L profit curve computed from existing trade logs.

**Architecture:** Left panel (320px) shows role mini-cards with sparklines. Right panel shows selected role's profit curve (lightweight-charts), positions table, and collapsible/editable markdown strategy. Role strategies persisted to `.atrading/strategies/<name>/base.md` filesystem. P&L curve computed real-time from trade_logs Open/Close records — no new DB tables.

**Tech Stack:** Rust (axum, rusqlite, tokio), Vue 3 + TypeScript, lightweight-charts, TailwindCSS 3.4

---

### Task 1: Add `role_name` column to trade_logs + new TradeManager methods

**Files:**

- Modify: `crates/core/src/trade/mod.rs`
- Modify: `crates/core/src/trade/sqlite.rs`

- [ ] **Step 1: Add trait methods to TradeManager**

In `crates/core/src/trade/mod.rs`, add to the `TradeManager` trait after `format_positions`:

```rust
fn delete_positions_by_role(&self, role_name: &str) -> Result<usize>;
fn delete_trade_logs_by_role(&self, role_name: &str) -> Result<usize>;
fn get_all_trade_logs_by_role(&self, role_name: &str) -> Result<Vec<TradeLogEntry>>;
```

- [ ] **Step 2: Add migration + implementations in sqlite.rs**

In `crates/core/src/trade/sqlite.rs`, in `SqliteTradeManager::open`, add after the CREATE TABLE statements:

```rust
// Migration: add role_name to trade_logs for closed-position traceability
conn.execute(
    "ALTER TABLE trade_logs ADD COLUMN role_name TEXT NOT NULL DEFAULT ''",
    [],
).ok();
```

Update the `add_position` INSERT into trade_logs to include role_name:

```rust
conn.execute(
    "INSERT INTO trade_logs (position_id,action,price,shares,reason,role_name) VALUES (?1,'Open',?2,?3,?4,?5)",
    rusqlite::params![id, pos.cost_price, pos.shares, pos.entry_reason, pos.role_name],
)?;
```

Update `close_position` INSERT:

```rust
// First get the role_name for this position
let role_name: String = conn.query_row(
    "SELECT role_name FROM positions WHERE id=?1",
    [id],
    |row| row.get(0),
)?;
conn.execute(
    "INSERT INTO trade_logs (position_id,action,price,shares,reason,role_name) VALUES (?1,'Close',?2,0,?3,?4)",
    rusqlite::params![id, price, reason, role_name],
)?;
```

Add the three new trait implementations after `format_positions`:

```rust
fn delete_positions_by_role(&self, role_name: &str) -> Result<usize> {
    let conn = self.conn.lock().unwrap();
    let n = conn.execute(
        "DELETE FROM positions WHERE role_name=?1",
        [role_name],
    )?;
    Ok(n)
}

fn delete_trade_logs_by_role(&self, role_name: &str) -> Result<usize> {
    let conn = self.conn.lock().unwrap();
    let n = conn.execute(
        "DELETE FROM trade_logs WHERE role_name=?1",
        [role_name],
    )?;
    Ok(n)
}

fn get_all_trade_logs_by_role(&self, role_name: &str) -> Result<Vec<TradeLogEntry>> {
    let conn = self.conn.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT tl.id, tl.position_id, tl.action, tl.price, tl.shares, \
         tl.reason, tl.created_at, COALESCE(p.symbol, '') as symbol \
         FROM trade_logs tl \
         LEFT JOIN positions p ON tl.position_id = p.id \
         WHERE tl.role_name = ?1 \
         ORDER BY tl.created_at ASC",
    )?;
    let rows = stmt.query_map([role_name], |row| {
        Ok(TradeLogEntry {
            id: row.get(0)?,
            position_id: row.get(1)?,
            action: row.get(2)?,
            price: row.get(3)?,
            shares: row.get(4)?,
            reason: row.get(5)?,
            created_at: row.get(6)?,
            symbol: row.get(7)?,
        })
    })?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}
```

- [ ] **Step 3: Build and test**

Run: `cargo build -p openatrading-core`
Expected: compiles cleanly

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/trade/mod.rs crates/core/src/trade/sqlite.rs
git commit -m "feat(trade): add role_name to trade_logs, add delete/get_all methods"
```

---

### Task 2: Role filesystem persistence + management functions

**Files:**

- Modify: `crates/core/src/roles/mod.rs`

- [ ] **Step 1: Add role management functions**

In `crates/core/src/roles/mod.rs`, add after `get_builtin_strategy`:

```rust
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

pub fn default_strategies_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".atrading").join("strategies")
}

/// Returns (name, display_name, strategy) for each user role on disk
pub fn list_user_roles(strategies_dir: &Path) -> Vec<(String, String, String)> {
    let mut roles = Vec::new();
    let dir = match fs::read_dir(strategies_dir) {
        Ok(d) => d,
        Err(_) => return roles,
    };
    for entry in dir.flatten() {
        let path = entry.path();
        if !path.is_dir() { continue; }
        let base_md = path.join("base.md");
        if !base_md.exists() { continue; }
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let strategy = fs::read_to_string(&base_md).unwrap_or_default();
        roles.push((name.clone(), name, strategy));
    }
    roles
}

pub fn save_role_strategy(strategies_dir: &Path, name: &str, strategy: &str) -> Result<()> {
    let role_dir = strategies_dir.join(name);
    fs::create_dir_all(&role_dir)
        .with_context(|| format!("创建目录失败: {}", role_dir.display()))?;
    fs::write(role_dir.join("base.md"), strategy)
        .with_context(|| format!("写入策略失败: {}/base.md", name))?;
    Ok(())
}

pub fn delete_role_dir(strategies_dir: &Path, name: &str) -> Result<()> {
    let role_dir = strategies_dir.join(name);
    if role_dir.exists() {
        fs::remove_dir_all(&role_dir)
            .with_context(|| format!("删除角色目录失败: {}", role_dir.display()))?;
    }
    Ok(())
}
```

- [ ] **Step 2: Build and verify**

Run: `cargo build -p openatrading-core`
Expected: compiles cleanly

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/roles/mod.rs
git commit -m "feat(roles): add filesystem persistence for role strategies"
```

---

### Task 3: P&L history computation

**Files:**

- Create: `crates/core/src/roles/pnl.rs`
- Modify: `crates/core/src/roles/mod.rs`

- [ ] **Step 1: Create P&L computation module**

Create `crates/core/src/roles/pnl.rs`:

```rust
use serde::Serialize;
use crate::trade::TradeLogEntry;

#[derive(Debug, Clone, Serialize)]
pub struct PnlPoint {
    pub date: String,
    pub equity: f64,        // total account value
    pub cash: f64,
    pub market_value: f64,
    pub return_pct: f64,    // cumulative return %
}

/// Compute P&L history from trade logs. Starts with 1,000,000 initial capital.
pub fn compute_pnl_history(logs: &[TradeLogEntry], current_positions_value: f64) -> Vec<PnlPoint> {
    let initial_capital: f64 = 1_000_000.0;
    if logs.is_empty() {
        return vec![];
    }

    let mut points: Vec<PnlPoint> = vec![];
    let mut cash = initial_capital;
    // Track open positions: symbol -> (shares, avg_cost)
    let mut holdings: std::collections::HashMap<String, (i64, f64)> = std::collections::HashMap::new();

    for log in logs {
        match log.action.as_str() {
            "Open" => {
                let cost = log.price * log.shares as f64;
                cash -= cost;
                holdings.insert(log.symbol.clone(), (log.shares, log.price));
            }
            "Close" => {
                let proceeds = log.price * log.shares as f64;
                cash += proceeds;
                holdings.remove(&log.symbol);
            }
            _ => {}
        }

        // Mark-to-market: use last known price for open positions
        let market_value: f64 = holdings.iter().map(|(_, (shares, avg_cost))| {
            *avg_cost * *shares as f64  // use cost as proxy when no current price
        }).sum();

        let equity = cash + market_value;
        let return_pct = (equity - initial_capital) / initial_capital * 100.0;

        points.push(PnlPoint {
            date: log.created_at[..10].to_string(),
            equity: (equity * 100.0).round() / 100.0,
            cash: (cash * 100.0).round() / 100.0,
            market_value: (market_value * 100.0).round() / 100.0,
            return_pct: (return_pct * 100.0).round() / 100.0,
        });
    }

    // Add final point with current positions mark-to-market
    if current_positions_value > 0.0 {
        let last = points.last().unwrap();
        let equity_with_current = last.cash + current_positions_value;
        let return_pct = (equity_with_current - initial_capital) / initial_capital * 100.0;
        points.push(PnlPoint {
            date: chrono::Local::now().format("%Y-%m-%d").to_string(),
            equity: (equity_with_current * 100.0).round() / 100.0,
            cash: last.cash,
            market_value: (current_positions_value * 100.0).round() / 100.0,
            return_pct: (return_pct * 100.0).round() / 100.0,
        });
    }

    // Deduplicate by date: keep last entry per day
    let mut daily: Vec<PnlPoint> = vec![];
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for p in points.into_iter().rev() {
        if seen.insert(p.date.clone()) {
            daily.push(p);
        }
    }
    daily.reverse();
    daily
}
```

- [ ] **Step 2: Register module**

In `crates/core/src/roles/mod.rs`, add:

```rust
pub mod pnl;
```

- [ ] **Step 3: Add chrono dependency if needed**

Check if `chrono` is already in `Cargo.toml`. If not, add it.

Run: `grep chrono crates/core/Cargo.toml`
If not found: `cargo add chrono -p openatrading-core`

- [ ] **Step 4: Build**

Run: `cargo build -p openatrading-core`
Expected: compiles

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/roles/pnl.rs crates/core/src/roles/mod.rs crates/core/Cargo.toml
git commit -m "feat(roles): add P&L history computation from trade logs"
```

---

### Task 4: API handlers for role CRUD + reset + P&L history

**Files:**

- Modify: `crates/core/src/web/handlers.rs` (append new handlers)

- [ ] **Step 1: Add handler types and imports at top of handlers.rs**

Add to existing imports:

```rust
use crate::roles;
use crate::roles::pnl::compute_pnl_history;
use axum::response::Json;
use std::path::PathBuf;
```

Add request/response types before the handler functions:

```rust
#[derive(Deserialize)]
pub struct CreateRoleRequest {
    pub name: String,
    pub display_name: String,
    pub strategy: String,
}

#[derive(Deserialize)]
pub struct UpdateStrategyRequest {
    pub strategy: String,
}

#[derive(Serialize)]
pub struct PnlHistoryResponse {
    pub points: Vec<roles::pnl::PnlPoint>,
    pub initial_capital: f64,
}
```

- [ ] **Step 2: Add handler functions**

Append to `crates/core/src/web/handlers.rs`:

```rust
/// POST /api/roles — create a new role
pub async fn create_role(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateRoleRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    roles::save_role_strategy(&state.strategies_dir, &body.name, &body.strategy)
        .map_err(|e| {
            eprintln!("create_role error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(serde_json::json!({"ok": true})))
}

/// PUT /api/roles/:name/strategy — update role strategy
pub async fn update_role_strategy(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Json(body): Json<UpdateStrategyRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    // Check built-in
    if roles::get_builtin_strategy(&name).is_some() {
        // Allow editing built-in strategies too — they get saved as user override
    }
    roles::save_role_strategy(&state.strategies_dir, &name, &body.strategy)
        .map_err(|e| {
            eprintln!("update_role_strategy error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(serde_json::json!({"ok": true})))
}

/// DELETE /api/roles/:name — delete role and all its data
pub async fn delete_role(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    state.trade.delete_trade_logs_by_role(&name).ok();
    state.trade.delete_positions_by_role(&name).ok();
    roles::delete_role_dir(&state.strategies_dir, &name).ok();
    Ok(Json(serde_json::json!({"ok": true})))
}

/// POST /api/roles/:name/reset — reset account to 1,000,000
pub async fn reset_role(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    state.trade.delete_trade_logs_by_role(&name)
        .map_err(|e| {
            eprintln!("reset_role error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    state.trade.delete_positions_by_role(&name)
        .map_err(|e| {
            eprintln!("reset_role error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(serde_json::json!({"ok": true})))
}

/// GET /api/roles/:name/pnl-history?days=30 — P&L curve data
pub async fn get_role_pnl_history(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<impl IntoResponse, StatusCode> {
    let days: usize = params.get("days").and_then(|d| d.parse().ok()).unwrap_or(30);
    let all_logs = state.trade.get_all_trade_logs_by_role(&name)
        .map_err(|e| {
            eprintln!("get_role_pnl_history error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Calculate current positions value
    let positions = state.trade.list_positions(&name).unwrap_or_default();
    let current_value: f64 = positions.iter().map(|p| {
        p.current_price.unwrap_or(p.cost_price) * p.shares as f64
    }).sum();

    let mut points = compute_pnl_history(&all_logs, current_value);
    // Filter to last N days
    let cutoff = points.len().saturating_sub(days);
    points = points.into_iter().skip(cutoff).collect();

    Ok(Json(PnlHistoryResponse {
        points,
        initial_capital: 1_000_000.0,
    }))
}
```

- [ ] **Step 3: Build**

Run: `cargo build -p openatrading-core`
Expected: compiles. Fix any import errors.

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/web/handlers.rs
git commit -m "feat(web): add role CRUD, reset, and P&L history API handlers"
```

---

### Task 5: Update AppState, routes, and gateway

**Files:**

- Modify: `crates/core/src/gateway/mod.rs` (GatewayConfig, start)
- Modify: `crates/core/src/web/mod.rs` (AppState, routes)
- Modify: `crates/cli/src/cli/gateway_cmd.rs` (pass strategies_dir)

- [ ] **Step 1: Add strategies_dir to GatewayConfig (gateway/mod.rs)**

Add `strategies_dir` field to `GatewayConfig`:

```rust
pub struct GatewayConfig {
    pub cron: Option<(CronConfig, Arc<dyn NotificationSender>)>,
    pub web_port: Option<u16>,
    pub strategies_dir: PathBuf,
}
```

In `Gateway::start`, pass it to AppState. Change line ~67:

```rust
let strategies_dir = config.strategies_dir.clone();
// ...
let state = Arc::new(AppState {
    llm: LlmClient::from_env(),
    api: ApiClient::new(),
    memory: self.memory.clone(),
    trade: self.trade.clone(),
    roles,
    strategies_dir,
});
```

- [ ] **Step 2: Update AppState and routes in web/mod.rs**

Add `use serde::Deserialize;` and `use std::path::PathBuf;`. Add `strategies_dir` to AppState:

```rust
pub struct AppState {
    pub llm: LlmClient,
    pub api: ApiClient,
    pub memory: Arc<SqliteMemoryStore>,
    pub trade: Arc<SqliteTradeManager>,
    pub roles: Vec<RoleEntry>,
    pub strategies_dir: PathBuf,
}
```

In `build_router`, add new routes after the existing role routes:

```rust
.route("/api/roles", post(handlers::create_role))
.route("/api/roles/:name/strategy", axum::routing::put(handlers::update_role_strategy))
.route("/api/roles/:name", axum::routing::delete(handlers::delete_role))
.route("/api/roles/:name/reset", post(handlers::reset_role))
.route("/api/roles/:name/pnl-history", get(handlers::get_role_pnl_history))
```

Change the axum routing imports:

```rust
use axum::{
    routing::{get, post, put, delete},
    Router,
};
```

Keep the existing `RoleEntry` struct in `web/mod.rs` (it was there before). The role loading part moves to `gateway/mod.rs`.

- [ ] **Step 3: Update gateway/mod.rs — load roles and pass strategies_dir**

In `crates/core/src/gateway/mod.rs`, update the AppState construction inside `start()`:

```rust
// Load built-in + user roles from filesystem
let mut roles: Vec<RoleEntry> = crate::roles::BUILTIN_STRATEGIES
    .iter()
    .map(|(name, display_name, strategy)| RoleEntry {
        name: name.to_string(),
        display_name: display_name.to_string(),
        strategy: strategy.to_string(),
    })
    .collect();

for (name, display_name, strategy) in crate::roles::list_user_roles(&config.strategies_dir) {
    // Skip if already exists (user override of built-in)
    if !roles.iter().any(|r| r.name == name) {
        roles.push(RoleEntry { name, display_name, strategy });
    }
}

let state = Arc::new(AppState {
    llm: LlmClient::from_env(),
    api: ApiClient::new(),
    memory: self.memory.clone(),
    trade: self.trade.clone(),
    roles,
    strategies_dir: config.strategies_dir.clone(),
});
```

- [ ] **Step 4: Update CLI to pass strategies_dir (gateway_cmd.rs)**

In `crates/cli/src/cli/gateway_cmd.rs`, update the `GatewayConfig` construction:

```rust
let strategies_dir = project_root.join(".atrading").join("strategies");

gateway
    .start(GatewayConfig {
        cron: cron_entry,
        web_port,
        strategies_dir,
    })
    .await
```

- [ ] **Step 5: Build full workspace**

Run: `cargo build`
Expected: compiles

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/web/mod.rs crates/core/src/gateway/mod.rs crates/cli/src/cli/gateway_cmd.rs
git commit -m "feat(web): add role CRUD routes, strategies_dir to AppState"
```

---

### Task 6: Frontend — data store API functions + types

**Files:**

- Modify: `src/src/stores/data.ts`

- [ ] **Step 1: Add types and API functions**

Append to `src/src/stores/data.ts` after the existing role functions:

```typescript
// ── Role P&L History ───────────────────────────────────

export interface PnlPoint {
  date: string;
  equity: number;
  cash: number;
  market_value: number;
  return_pct: number;
}

export interface PnlHistoryResponse {
  points: PnlPoint[];
  initial_capital: number;
}

// Per-role P&L history cache
export const rolePnlHistory = reactive<
  Record<string, { data: PnlPoint[]; loading: boolean; error: string }>
>({});

export async function fetchRolePnlHistory(name: string, days = 30) {
  if (!rolePnlHistory[name]) {
    rolePnlHistory[name] = reactive({ data: [], loading: false, error: "" });
  }
  const entry = rolePnlHistory[name];
  entry.loading = true;
  entry.error = "";
  try {
    const res = await fetchJson<PnlHistoryResponse>(
      `/api/roles/${name}/pnl-history?days=${days}`,
    );
    entry.data = res.points;
  } catch (e: any) {
    entry.error = e.message || String(e);
  } finally {
    entry.loading = false;
  }
}

// ── Role CRUD ──────────────────────────────────────────

export async function createRole(
  name: string,
  displayName: string,
  strategy: string,
) {
  await fetchJson("/api/roles", {
    method: "POST",
    body: JSON.stringify({ name, display_name: displayName, strategy }),
  });
}

export async function updateRoleStrategy(name: string, strategy: string) {
  await fetchJson(`/api/roles/${name}/strategy`, {
    method: "PUT",
    body: JSON.stringify({ strategy }),
  });
}

export async function deleteRole(name: string) {
  await fetchJson(`/api/roles/${name}`, { method: "DELETE" });
}

export async function resetRole(name: string) {
  await fetchJson(`/api/roles/${name}/reset`, { method: "POST" });
}

// Refresh single role's positions
export async function refreshRole(name: string) {
  await fetchRolePositions(name);
  await fetchRoleLogs(name);
  await fetchRolePnlHistory(name);
}
```

- [ ] **Step 2: Type-check**

Run: `cd src && npx vue-tsc --noEmit`
Expected: no new errors (may have existing ones)

- [ ] **Step 3: Verify build**

Run: `cd src && bun run build`
Expected: builds

- [ ] **Step 4: Commit**

```bash
git add src/src/stores/data.ts
git commit -m "feat(frontend): add role CRUD and P&L history API functions"
```

---

### Task 7: Frontend — Sparkline component

**Files:**

- Create: `src/src/components/Sparkline.vue`

- [ ] **Step 1: Create Sparkline.vue**

```vue
<template>
  <canvas
    ref="canvasRef"
    :width="width"
    :height="height"
    class="inline-block"
  ></canvas>
</template>

<script setup lang="ts">
import { ref, onMounted, watch } from "vue";
import type { PnlPoint } from "@/stores/data";

const props = defineProps<{
  points: PnlPoint[];
  width?: number;
  height?: number;
}>();

const canvasRef = ref<HTMLCanvasElement>();
const w = props.width ?? 200;
const h = props.height ?? 32;

function draw() {
  const canvas = canvasRef.value;
  if (!canvas || !props.points.length) return;
  const ctx = canvas.getContext("2d");
  if (!ctx) return;

  canvas.width = w * devicePixelRatio;
  canvas.height = h * devicePixelRatio;
  canvas.style.width = w + "px";
  canvas.style.height = h + "px";
  ctx.scale(devicePixelRatio, devicePixelRatio);

  ctx.clearRect(0, 0, w, h);

  const values = props.points.map((p) => p.return_pct);
  const min = Math.min(...values, 0);
  const max = Math.max(...values, 0);
  const range = max - min || 1;

  const padX = 2;
  const padY = 2;
  const plotW = w - padX * 2;
  const plotH = h - padY * 2;

  const getX = (i: number) =>
    padX + (i / Math.max(values.length - 1, 1)) * plotW;
  const getY = (v: number) => padY + plotH - ((v - min) / range) * plotH;

  // Draw line
  ctx.beginPath();
  ctx.strokeStyle = values[values.length - 1] >= 0 ? "#FF5C5C" : "#1D9E75";
  ctx.lineWidth = 1.5;
  ctx.lineJoin = "round";

  const firstX = getX(0);
  const firstY = getY(values[0]);
  ctx.moveTo(firstX, firstY);

  for (let i = 1; i < values.length; i++) {
    const x = getX(i);
    const y = getY(values[i]);
    // Simple quadratic bezier for smoothness
    const prevX = getX(i - 1);
    const prevY = getY(values[i - 1]);
    const cx = (prevX + x) / 2;
    ctx.quadraticCurveTo(prevX, prevY, cx, (prevY + y) / 2);
    ctx.lineTo(x, y);
  }
  ctx.stroke();
}

onMounted(draw);
watch(() => props.points, draw, { deep: true });
</script>
```

- [ ] **Step 2: Type-check**

Run: `cd src && npx vue-tsc --noEmit`
Expected: no errors

- [ ] **Step 3: Commit**

```bash
git add src/src/components/Sparkline.vue
git commit -m "feat(frontend): add Sparkline canvas component"
```

---

### Task 8: Frontend — RoleView.vue rewrite

**Files:**

- Modify: `src/src/RoleView.vue`

- [ ] **Step 1: Rewrite RoleView.vue with split-pane layout**

```vue
<template>
  <div class="pt-11 min-h-screen bg-bg">
    <div class="flex h-[calc(100vh-44px)]">
      <!-- Left Panel: Role List (320px) -->
      <div
        class="w-[320px] shrink-0 border-r border-hairline overflow-y-auto px-4 py-6 flex flex-col gap-3"
      >
        <h2 class="text-lg font-bold text-white px-1 mb-1">角色策略</h2>

        <div v-if="roles.loading" class="flex items-center gap-2 px-1 py-4">
          <div class="w-2 h-2 rounded-full bg-ai animate-pulse"></div>
          <span class="text-caption text-ink-muted">加载中...</span>
        </div>

        <div v-else-if="roles.error" class="text-up text-caption px-1 py-4">
          {{ roles.error }}
        </div>

        <template v-else>
          <div
            v-for="role in roles.data"
            :key="role.name"
            @click="selectRole(role.name)"
            class="rounded-card p-3 cursor-pointer transition-colors border"
            :class="
              selectedRole === role.name
                ? 'bg-ai/10 border-ai/30'
                : 'bg-bg-surface border-hairline hover:bg-white/[0.03]'
            "
          >
            <div class="flex items-center justify-between mb-1">
              <div class="flex items-center gap-2">
                <span class="w-2 h-2 rounded-full bg-ai shrink-0"></span>
                <span class="text-white font-semibold text-sm truncate">{{
                  role.display_name
                }}</span>
              </div>
            </div>
            <div class="flex items-center justify-between text-caption">
              <span
                v-if="rolePnls[role.name] !== undefined"
                :class="rolePnls[role.name] >= 0 ? 'text-up' : 'text-down'"
                class="font-mono"
              >
                {{ rolePnls[role.name] >= 0 ? "+" : ""
                }}{{ rolePnls[role.name].toFixed(1) }}%
              </span>
              <span v-else class="text-ink-muted">空仓</span>
              <span class="text-ink-muted"
                >{{ rolePositionCounts[role.name] || 0 }}支</span
              >
            </div>
            <!-- Sparkline -->
            <div class="mt-2 flex justify-center">
              <Sparkline
                v-if="rolePnlHistories[role.name]?.length"
                :points="rolePnlHistories[role.name]"
                :width="220"
                :height="28"
              />
              <span v-else class="text-ink-muted text-xs">暂无数据</span>
            </div>
          </div>

          <!-- Add Role Button -->
          <button
            @click="showCreateForm = true"
            class="rounded-card p-3 border border-dashed border-hairline text-ink-muted hover:text-ai hover:border-ai/30 transition-colors text-sm text-center cursor-pointer"
          >
            + 新增角色
          </button>

          <!-- Inline Create Form -->
          <div
            v-if="showCreateForm"
            class="bg-bg-surface border border-hairline rounded-card p-4 space-y-3"
          >
            <input
              v-model="createForm.name"
              placeholder="角色名 (英文)"
              class="w-full bg-bg border border-hairline rounded px-3 py-2 text-white text-sm font-mono"
            />
            <input
              v-model="createForm.displayName"
              placeholder="显示名 (中文)"
              class="w-full bg-bg border border-hairline rounded px-3 py-2 text-white text-sm"
            />
            <textarea
              v-model="createForm.strategy"
              placeholder="初始策略 (Markdown)"
              rows="4"
              class="w-full bg-bg border border-hairline rounded px-3 py-2 text-white text-sm font-mono resize-none"
            ></textarea>
            <div class="flex gap-2">
              <button
                @click="doCreateRole"
                class="px-4 py-1.5 bg-ai/20 text-ai rounded text-sm hover:bg-ai/30 transition-colors"
              >
                创建
              </button>
              <button
                @click="showCreateForm = false"
                class="px-4 py-1.5 text-ink-muted text-sm hover:text-white transition-colors"
              >
                取消
              </button>
            </div>
          </div>
        </template>
      </div>

      <!-- Right Panel: Role Detail -->
      <div class="flex-1 overflow-y-auto px-8 py-6">
        <!-- Empty state -->
        <div
          v-if="!selectedRole"
          class="flex items-center justify-center h-full text-ink-muted text-sm"
        >
          请选择一个角色
        </div>

        <template v-else>
          <!-- Action Bar -->
          <div class="flex items-center justify-between mb-6">
            <div>
              <h2 class="text-xl font-bold text-white">
                {{ currentRole?.display_name }}
              </h2>
              <div
                class="flex items-center gap-4 mt-1 text-caption text-ink-muted"
              >
                <span
                  >累计
                  <span
                    :class="
                      rolePnls[selectedRole] >= 0 ? 'text-up' : 'text-down'
                    "
                    class="font-mono"
                  >
                    {{ rolePnls[selectedRole] >= 0 ? "+" : ""
                    }}{{ (rolePnls[selectedRole] || 0).toFixed(1) }}%
                  </span></span
                >
                <span>持仓 {{ rolePositionCounts[selectedRole] || 0 }} 支</span>
                <span>初始资金 ¥1,000,000</span>
              </div>
            </div>
            <div class="flex gap-2">
              <button
                @click="editingStrategy = !editingStrategy"
                class="px-3 py-1.5 border border-hairline rounded text-sm text-ink-muted hover:text-white hover:border-white/20 transition-colors"
              >
                {{ editingStrategy ? "取消编辑" : "编辑策略" }}
              </button>
              <button
                @click="doResetRole"
                class="px-3 py-1.5 border border-hairline rounded text-sm text-ink-muted hover:text-up hover:border-up/30 transition-colors"
              >
                重置账户
              </button>
              <button
                @click="doDeleteRole"
                class="px-3 py-1.5 border border-hairline rounded text-sm text-ink-muted hover:text-up hover:border-up/30 transition-colors"
              >
                删除角色
              </button>
            </div>
          </div>

          <!-- Profit Curve -->
          <div
            class="bg-bg-surface border border-hairline rounded-card p-5 mb-6"
          >
            <div class="flex items-center justify-between mb-3">
              <h3 class="text-white font-semibold text-sm">盈利曲线</h3>
              <div class="flex gap-1">
                <button
                  v-for="d in [7, 30, 0]"
                  :key="d"
                  @click="pnlDays = d"
                  class="px-2 py-0.5 rounded text-xs transition-colors"
                  :class="
                    pnlDays === d
                      ? 'bg-ai/20 text-ai'
                      : 'text-ink-muted hover:text-white'
                  "
                >
                  {{ d === 0 ? "全部" : d + "日" }}
                </button>
              </div>
            </div>
            <div
              v-if="rolePnlHistories[selectedRole]?.loading"
              class="text-center py-10 text-ink-muted text-sm"
            >
              加载中...
            </div>
            <div
              v-else-if="!rolePnlHistories[selectedRole]?.data?.length"
              class="text-center py-10 text-ink-muted text-sm"
            >
              暂无交易数据
            </div>
            <div v-else ref="chartContainerRef" class="w-full h-[300px]"></div>
          </div>

          <!-- Positions Table -->
          <div
            class="bg-bg-surface border border-hairline rounded-card p-5 mb-6"
          >
            <h3 class="text-white font-semibold text-sm mb-3">持仓明细</h3>
            <div
              v-if="
                !rolePositions[selectedRole]?.loading &&
                !rolePositions[selectedRole]?.data?.length
              "
              class="text-caption text-ink-muted py-4 text-center"
            >
              暂无持仓
            </div>
            <table
              v-else-if="rolePositions[selectedRole]?.data?.length"
              class="w-full text-caption"
            >
              <thead>
                <tr class="text-ink-muted text-left">
                  <th class="pb-2 font-normal">代码</th>
                  <th class="pb-2 font-normal">名称</th>
                  <th class="pb-2 font-normal text-right">盈亏%</th>
                  <th class="pb-2 font-normal text-right">成本/现价</th>
                  <th class="pb-2 font-normal text-right">股数</th>
                  <th class="pb-2 font-normal text-right">持仓天数</th>
                  <th class="pb-2 font-normal">理由</th>
                </tr>
              </thead>
              <tbody>
                <tr
                  v-for="pos in rolePositions[selectedRole].data"
                  :key="pos.id"
                  class="border-t border-hairline"
                >
                  <td class="py-2 font-mono text-white/70">{{ pos.symbol }}</td>
                  <td class="py-2 text-white/70">{{ pos.stock_name }}</td>
                  <td
                    class="py-2 text-right font-mono"
                    :class="pnl(pos) >= 0 ? 'text-up' : 'text-down'"
                  >
                    {{ pnl(pos) >= 0 ? "+" : "" }}{{ pnl(pos).toFixed(1) }}%
                  </td>
                  <td class="py-2 text-right font-mono text-white/50">
                    {{ pos.cost_price.toFixed(2) }} /
                    {{ (pos.current_price || 0).toFixed(2) }}
                  </td>
                  <td class="py-2 text-right font-mono text-white/70">
                    {{ pos.shares }}
                  </td>
                  <td class="py-2 text-right font-mono text-white/50">
                    {{ daysSince(pos.entry_date) }}天
                  </td>
                  <td class="py-2 text-white/50 max-w-[200px] truncate">
                    {{ pos.entry_reason }}
                  </td>
                </tr>
              </tbody>
            </table>
          </div>

          <!-- Strategy Section -->
          <div class="bg-bg-surface border border-hairline rounded-card p-5">
            <h3 class="text-white font-semibold text-sm mb-3">策略</h3>

            <!-- View Mode -->
            <div v-if="!editingStrategy">
              <button
                @click="toggleStrategy"
                class="text-caption text-ai hover:text-ai/80 transition-colors font-mono"
              >
                {{ strategyExpanded ? "收起 ▲" : "展开 ▼" }}
              </button>
              <div
                v-if="strategyExpanded"
                class="mt-2 bg-bg rounded-card p-4 max-h-[400px] overflow-y-auto text-caption text-white/70 leading-relaxed prose prose-invert prose-sm max-w-none"
              >
                <div v-html="renderMarkdown(currentRole?.strategy || '')"></div>
              </div>
              <p
                v-else
                class="mt-1 text-caption text-ink-muted truncate max-w-2xl"
              >
                {{
                  (currentRole?.strategy || "")
                    .split("\n")
                    .slice(0, 3)
                    .join(" ")
                }}...
              </p>
            </div>

            <!-- Edit Mode -->
            <div v-else>
              <textarea
                v-model="strategyDraft"
                rows="20"
                class="w-full bg-bg border border-hairline rounded-card p-4 text-caption text-white/70 font-mono resize-none focus:border-ai/30 focus:outline-none"
              ></textarea>
              <div class="flex gap-2 mt-3">
                <button
                  @click="doSaveStrategy"
                  class="px-4 py-1.5 bg-ai/20 text-ai rounded text-sm hover:bg-ai/30 transition-colors"
                >
                  保存
                </button>
                <button
                  @click="editingStrategy = false"
                  class="px-4 py-1.5 text-ink-muted text-sm hover:text-white transition-colors"
                >
                  取消
                </button>
              </div>
            </div>
          </div>
        </template>
      </div>
    </div>

    <!-- Confirm Dialog -->
    <div
      v-if="confirmAction"
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/60"
      @click.self="confirmAction = null"
    >
      <div
        class="bg-bg-surface border border-hairline rounded-card p-6 max-w-sm w-full"
      >
        <p class="text-white text-sm mb-4">{{ confirmMessage }}</p>
        <div class="flex gap-2 justify-end">
          <button
            @click="confirmAction = null"
            class="px-4 py-1.5 border border-hairline rounded text-sm text-ink-muted hover:text-white transition-colors"
          >
            取消
          </button>
          <button
            @click="executeConfirm"
            class="px-4 py-1.5 bg-up/20 text-up rounded text-sm hover:bg-up/30 transition-colors"
          >
            确认
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch, nextTick } from "vue";
import {
  roles,
  fetchRoles,
  rolePositions,
  fetchRolePositions,
  roleLogs,
  fetchRoleLogs,
  rolePnlHistory,
  fetchRolePnlHistory,
  createRole,
  updateRoleStrategy,
  deleteRole,
  resetRole,
  type RolePosition,
  type PnlPoint,
} from "@/stores/data";
import Sparkline from "@/components/Sparkline.vue";
import { createChart, ColorType, type IChartApi } from "lightweight-charts";

// ── State ──────────────────────────────────────────────

const selectedRole = ref<string | null>(null);
const strategyExpanded = ref(false);
const editingStrategy = ref(false);
const strategyDraft = ref("");
const showCreateForm = ref(false);
const pnlDays = ref(30);

const createForm = ref({
  name: "",
  displayName: "",
  strategy: "# 新角色\n\n## 策略\n\n",
});

const confirmAction = ref<"reset" | "delete" | null>(null);
const confirmMessage = computed(() =>
  confirmAction.value === "reset"
    ? `确认重置 ${currentRole.value?.display_name} 账户？将清空所有持仓和交易记录，恢复初始资金 ¥1,000,000。此操作不可撤销。`
    : `确认删除 ${currentRole.value?.display_name}？将删除该角色及所有相关持仓和交易记录。此操作不可撤销。`,
);

const chartContainerRef = ref<HTMLDivElement>();
let chart: IChartApi | null = null;

// ── Computed ────────────────────────────────────────────

const currentRole = computed(() =>
  roles.data.find((r) => r.name === selectedRole.value),
);

const rolePnls = computed(() => {
  const result: Record<string, number> = {};
  for (const role of roles.data) {
    const positions = rolePositions[role.name]?.data || [];
    if (!positions.length) continue;
    let totalCost = 0,
      totalValue = 0;
    for (const p of positions) {
      totalCost += p.cost_price * p.shares;
      totalValue += (p.current_price || 0) * p.shares;
    }
    result[role.name] =
      totalCost > 0 ? ((totalValue - totalCost) / totalCost) * 100 : 0;
  }
  return result;
});

const rolePositionCounts = computed(() => {
  const result: Record<string, number> = {};
  for (const role of roles.data) {
    result[role.name] = (rolePositions[role.name]?.data || []).length;
  }
  return result;
});

const rolePnlHistories = computed(() => {
  const result: Record<string, PnlPoint[]> = {};
  for (const role of roles.data) {
    result[role.name] = rolePnlHistory[role.name]?.data || [];
  }
  return result;
});

// ── Methods ─────────────────────────────────────────────

function pnl(pos: RolePosition): number {
  if (!pos.current_price || !pos.cost_price) return 0;
  return ((pos.current_price - pos.cost_price) / pos.cost_price) * 100;
}

function daysSince(dateStr: string): number {
  if (!dateStr) return 0;
  const d = new Date(dateStr);
  const now = new Date();
  return Math.floor((now.getTime() - d.getTime()) / 86400000);
}

function renderMarkdown(md: string): string {
  return md
    .replace(
      /^### (.+)$/gm,
      '<h4 class="text-white font-semibold mt-4 mb-1">$1</h4>',
    )
    .replace(
      /^## (.+)$/gm,
      '<h3 class="text-ai font-semibold mt-4 mb-2">$1</h3>',
    )
    .replace(
      /^# (.+)$/gm,
      '<h3 class="text-ai font-bold mt-3 mb-2 text-sm">$1</h3>',
    )
    .replace(/\*\*(.+?)\*\*/g, '<strong class="text-white/90">$1</strong>')
    .replace(/^\- (.+)$/gm, '<li class="ml-4 text-white/60">$1</li>')
    .replace(/\n\n/g, "<br/><br/>")
    .replace(/\n/g, "<br/>");
}

function toggleStrategy() {
  strategyExpanded.value = !strategyExpanded.value;
}

async function selectRole(name: string) {
  selectedRole.value = name;
  strategyExpanded.value = false;
  editingStrategy.value = false;
  await refreshRole(name);
  await nextTick();
  renderChart();
}

async function refreshRole(name: string) {
  await fetchRolePnlHistory(name, pnlDays.value === 0 ? 365 : pnlDays.value);
  if (!rolePositions[name]?.data?.length) {
    fetchRolePositions(name);
    fetchRoleLogs(name);
  }
}

// ── CRUD ────────────────────────────────────────────────

async function doCreateRole() {
  if (!createForm.value.name.trim()) return;
  try {
    await createRole(
      createForm.value.name.trim(),
      createForm.value.displayName.trim() || createForm.value.name.trim(),
      createForm.value.strategy,
    );
    showCreateForm.value = false;
    createForm.value = {
      name: "",
      displayName: "",
      strategy: "# 新角色\n\n## 策略\n\n",
    };
    await fetchRoles();
  } catch (e: any) {
    alert("创建失败: " + (e.message || e));
  }
}

async function doSaveStrategy() {
  if (!selectedRole.value) return;
  try {
    await updateRoleStrategy(selectedRole.value, strategyDraft.value);
    editingStrategy.value = false;
    await fetchRoles();
  } catch (e: any) {
    alert("保存失败: " + (e.message || e));
  }
}

function doResetRole() {
  confirmAction.value = "reset";
}

function doDeleteRole() {
  confirmAction.value = "delete";
}

async function executeConfirm() {
  if (!selectedRole.value) return;
  try {
    if (confirmAction.value === "reset") {
      await resetRole(selectedRole.value);
      await refreshRole(selectedRole.value);
      await renderChart();
    } else if (confirmAction.value === "delete") {
      await deleteRole(selectedRole.value);
      const deleted = selectedRole.value;
      selectedRole.value = null;
      await fetchRoles();
    }
  } catch (e: any) {
    alert("操作失败: " + (e.message || e));
  } finally {
    confirmAction.value = null;
  }
}

// Watch edit mode to populate draft
watch(editingStrategy, (editing) => {
  if (editing && currentRole.value) {
    strategyDraft.value = currentRole.value.strategy;
  }
});

// Watch P&L days
watch(pnlDays, async (days) => {
  if (selectedRole.value) {
    await fetchRolePnlHistory(selectedRole.value, days === 0 ? 365 : days);
    await nextTick();
    renderChart();
  }
});

// ── Chart ───────────────────────────────────────────────

function renderChart() {
  if (chart) {
    chart.remove();
    chart = null;
  }

  const container = chartContainerRef.value;
  if (!container || !selectedRole.value) return;

  const points = rolePnlHistory[selectedRole.value]?.data || [];
  if (!points.length) return;

  chart = createChart(container, {
    width: container.clientWidth,
    height: 300,
    layout: {
      background: { type: ColorType.Solid, color: "transparent" },
      textColor: "#888780",
    },
    grid: {
      vertLines: { color: "rgba(255,255,255,0.04)" },
      horzLines: { color: "rgba(255,255,255,0.04)" },
    },
    rightPriceScale: { borderColor: "rgba(255,255,255,0.08)" },
    timeScale: { borderColor: "rgba(255,255,255,0.08)", timeVisible: false },
  });

  // Cumulative return % (area series)
  const returnData = points.map((p) => ({
    time: p.date,
    value: p.return_pct,
  }));

  const returnSeries = chart.addAreaSeries({
    lineColor:
      points.length && points[points.length - 1].return_pct >= 0
        ? "#FF5C5C"
        : "#1D9E75",
    topColor:
      points.length && points[points.length - 1].return_pct >= 0
        ? "rgba(255,92,92,0.15)"
        : "rgba(29,158,117,0.15)",
    bottomColor: "rgba(0,0,0,0)",
    lineWidth: 2,
    priceFormat: {
      type: "custom",
      formatter: (v: number) => v.toFixed(1) + "%",
    },
  });
  returnSeries.setData(returnData);

  // Market value (line series, right axis)
  const mvData = points.map((p) => ({
    time: p.date,
    value: p.market_value,
  }));

  const mvSeries = chart.addLineSeries({
    color: "#7F77DD",
    lineWidth: 1,
    priceScaleId: "right",
    priceFormat: {
      type: "custom",
      formatter: (v: number) => (v / 10000).toFixed(1) + "万",
    },
  });
  mvSeries.setData(mvData);

  // Cash (line series, right axis)
  const cashData = points.map((p) => ({
    time: p.date,
    value: p.cash,
  }));

  const cashSeries = chart.addLineSeries({
    color: "#888780",
    lineWidth: 1,
    lineStyle: 2, // dashed
    priceScaleId: "right",
    priceFormat: {
      type: "custom",
      formatter: (v: number) => (v / 10000).toFixed(1) + "万",
    },
  });
  cashSeries.setData(cashData);

  chart.timeScale().fitContent();
}

// ── Init ────────────────────────────────────────────────

onMounted(async () => {
  await fetchRoles();
  // Load positions/pnl for all roles (for sparklines)
  for (const role of roles.data) {
    fetchRolePositions(role.name);
    fetchRolePnlHistory(role.name, 30);
  }
  // Auto-select first role
  if (roles.data.length && !selectedRole.value) {
    selectRole(roles.data[0].name);
  }
});
</script>
```

- [ ] **Step 2: Type-check and build**

Run: `cd src && npx vue-tsc --noEmit`
Expected: no new errors

Run: `cd src && bun run build`
Expected: builds

- [ ] **Step 3: Commit**

```bash
git add src/src/RoleView.vue
git commit -m "feat(frontend): redesign role strategy page with split-pane layout"
```

---

### Task 9: Integration test — run the full stack

**Files:** None (verification only)

- [ ] **Step 1: Build Rust**

Run: `cargo build`
Expected: compiles, no errors

- [ ] **Step 2: Build frontend**

Run: `cd src && bun run build`
Expected: builds, no errors

- [ ] **Step 3: Review checklist**

- [ ] Left panel shows role cards with sparklines
- [ ] Clicking a role shows its detail on the right
- [ ] Profit curve renders with 3 series (return%, market value, cash)
- [ ] Positions table shows all columns including 持仓天数
- [ ] Strategy section renders Markdown when expanded
- [ ] Edit strategy → textarea → save → refreshes
- [ ] Reset account → confirm dialog → clears positions/logs
- [ ] Delete role → confirm dialog → removes role from list
- [ ] Create role → inline form → adds to list

- [ ] **Step 4: Final commit (if needed)**

```bash
git status
# Commit any remaining changes
```
