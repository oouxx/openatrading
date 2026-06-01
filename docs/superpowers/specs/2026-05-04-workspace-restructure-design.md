# Workspace Restructure Design: openatrading → Tauri + Vue

**Date:** 2026-05-04
**Status:** Approved by user
**Approach:** Phased Migration (per user confirmation)

---

## 1. Overview

Restructure the existing `openatrading` Rust project from a single-crate layout into a Cargo workspace with clear separation between business logic (`crates/core`), delivery mechanisms (`crates/cli`, `crates/web`), WASM wrapper (`crates/wasm`), Tauri desktop app (`src-tauri/`), and Vue frontend (`src/`).

**Goal:** Enable multi-platform delivery (CLI, Web, Desktop) while keeping core business logic framework-agnostic.

---

## 2. Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                     Frontend (src/)                          │
│  Vue 3 + TypeScript                                        │
│  ├── api/index.ts (auto-switch mode)                        │
│  ├── api/tauri.ts (invoke Tauri commands)                   │
│  └── api/web.ts (call WASM exports)                         │
└──────────────┬──────────────────────────┬───────────────────┘
               │ Tauri IPC               │ WASM bindings
               ▼                         ▼
┌──────────────────────┐      ┌──────────────────────┐
│   src-tauri/         │      │   crates/wasm/       │
│   Tauri Desktop App  │      │   wasm-bindgen      │
│   (invokes core)     │      │   wrapper over core  │
└──────────┬───────────┘      └──────────┬───────────┘
           │                             │
           └──────────┬──────────────────┘
                      ▼
           ┌──────────────────┐
           │  crates/core/    │
           │  Pure business   │
           │  logic, zero     │
           │  framework deps  │
           └──────────┬───────┘
                      │
           ┌──────────┴───────┐
           ▼                  ▼
┌──────────────────┐  ┌──────────────────┐
│  crates/cli/     │  │  crates/web/     │
│  Clap CLI binary │  │  Axum web server │
└──────────────────┘  └──────────────────┘
```

**Key architectural decisions:**
- `core` is framework-agnostic: no Axum, no Clap, no Tauri dependencies
- `cli` and `web` are thin wrappers that parse input and call `core`
- `wasm` exposes `core` functions to web via wasm-bindgen
- `src-tauri` uses Tauri's `invoke` system to call Rust functions from Vue

---

## 3. Component Breakdown

### 3.1 `crates/core/` (Pure Business Logic)

**Dependencies:** `tokio`, `reqwest`, `scraper`, `serde`, `anyhow`, `chrono`, `futures`, `dotenvy`, `askama`, `regex`
**No framework deps:** No Clap, no Axum, no Tauri

**Modules (moved from current `src/`):**
- `api/`: `XuanguBaoClient`, `JiuYanSession`, `TgbClient` (all data fetching logic)
- `agents/`: `SentimentAgent`, `TechnicalAgent`, `MainAgent` (analysis logic)
- `llm/`: `openrouter.rs` (LLM integration)
- `report/`: `Report` struct, `Reporter` (data aggregation + rendering)
- `roles/`: `analyst.rs` (role-based prompts)
- `templates/`: Askama template structs (shared between CLI/Web/Tauri)
- `utils/`: `html.rs` (HTML-to-Markdown conversion)

### 3.2 `crates/cli/` (Clap CLI Binary)

**Dependencies:** `core`, `clap`, `rustyline`

**Content:**
- `main.rs`: Parse CLI args via Clap (all existing subcommands: `limit-up`, `limit-down`, `run`, `sentiment`, `agent`, etc.)
- Invokes `core` functions directly, passes results to `core::report::Reporter` for output

### 3.3 `crates/web/` (Axum Web Server)

**Dependencies:** `core`, `axum`

**Content:**
- `main.rs`: Axum routes (`GET /`, `GET /report?format=markdown|html`)
- `WebState`: `Arc<Mutex<Option<Report>>>` (shares `core::Report` instances)
- No business logic, only HTTP handling + calling `core` functions

### 3.4 `crates/wasm/` (WASM Wrapper)

**Dependencies:** `core`, `wasm-bindgen`, `wasm-bindgen-futures`

**Content:**
- `lib.rs`: Exports thin wrappers around `core` functions via `#[wasm_bindgen]` for web use
- No business logic, only type conversions (Rust ↔ JS) and async runtime setup

### 3.5 `src-tauri/` (Tauri Desktop App)

**Dependencies:** `core`, `tauri` (v2), `tauri-build`

**Content:**
- `Cargo.toml`: Tauri crate config, links to `core`
- `src/main.rs`: Tauri setup, registers Tauri commands that invoke `core` functions
- Communicates with Vue frontend via Tauri's IPC (`invoke` handler)

### 3.6 `src/` (Vue 3 Frontend)

**Dependencies:** Vue 3, TypeScript, Vite, Tauri API (if in Tauri mode)

**Structure:**
- `api/index.ts`: Auto-switches between Tauri mode (desktop) and WASM mode (web) based on environment
- `api/tauri.ts`: Calls Tauri commands via `window.__TAURI__.invoke`
- `api/web.ts`: Imports WASM module, calls exported functions
- Existing UI components (to be added later, out of scope for restructuring)

---

## 4. Data Flow

**Report Generation Flow (CLI/Web/Tauri all follow this):**

```
1. User triggers command (CLI arg / HTTP request / Tauri invoke)
        │
        ▼
2. CLI/Web/Tauri handler receives request
   - CLI: Clap parses args → calls core::run_command()
   - Web: Axum handler → calls core::run_command()
   - Tauri: invoke handler → calls core::run_command()
        │
        ▼
3. core::api::Client fetches data (parallel via tokio::join!)
   - XuanguBaoClient → limit_up, limit_down, overview, etc.
   - JiuYanSession → community articles, field data
   - TgbClient → user methodology posts
        │
        ▼
4. core::agents analyze data
   - SentimentAgent → processes community articles
   - TechnicalAgent → processes limit-up data
   - MainAgent → LLM integration via core::llm
        │
        ▼
5. core::report::Report aggregates results
        │
        ▼
6. Output via core::templates (Askama)
   - CLI: prints to terminal / writes to file
   - Web: returns HTTP response (Markdown/HTML)
   - Tauri: returns data to Vue frontend via IPC
```

**Key:** All paths converge at `core::` — the delivery mechanism (CLI/Web/Tauri) is just a thin shell.

---

## 5. Error Handling

Following existing pattern — **`anyhow::Result` everywhere in core**, no custom error types.

- `core`: All functions return `anyhow::Result<T>`
- `cli`: Propagates errors via `?` operator, prints to stderr
- `web`: Axum error handlers convert `anyhow::Error` to HTTP responses (500 for server errors, 400 for bad requests)
- `src-tauri`: Tauri commands return `Result<T, String>`, converting `anyhow::Error` to strings
- `wasm`: WASM exports return `JsValue` for errors, converting `anyhow::Error` to JS exceptions

**Retry logic** (existing in `JiuYanSession`) stays in `core::api::JiuYanSession` — no changes needed.

---

## 6. Testing Strategy

**Existing tests:**
- `tests/jiuyan.rs` (7 integration tests, `#[ignore]`, need `JIUYAN_PHONE`)
- `tests/xuangubao.rs` (unit test for JSON parsing)

**After restructure:**
- Move integration tests into `crates/core/tests/` (they test core business logic)
- `crates/cli`: Add CLI arg parsing tests (Clap's `try_parse()`)
- `crates/web`: Add Axum route tests (using `axum::body::Body` + `tower::ServiceExt`)
- `crates/wasm`: Add wasm-bindgen tests (via `wasm-bindgen-test`)
- Leave HTTP mocking for future (no changes to current no-mock approach)

---

## 7. Implementation Plan (Phased Migration)

### Phase 1: Restructure Rust Crates
1. Create `crates/core/`, move all business logic from `src/` into it
2. Create `crates/cli/`, move CLI binary (current `src/main.rs` + `src/cli/`)
3. Create `crates/web/`, move Axum web server (current `src/web/`)
4. Create `crates/wasm/`, add thin wasm-bindgen wrapper
5. Update root `Cargo.toml` as workspace with all crates
6. Verify: `cargo build` succeeds, all existing commands work

### Phase 2: Set Up Tauri Desktop App
1. Run `cargo tauri init` (or manually create `src-tauri/`)
2. Configure `src-tauri/Cargo.toml` to depend on `crates/core`
3. Register Tauri commands that invoke `core` functions
4. Verify: Tauri app builds and can call core functions

### Phase 3: Scaffold Vue Frontend
1. Run `npm create vite@latest src -- --template vue-ts`
2. Create `src/api/index.ts`, `src/api/tauri.ts`, `src/api/web.ts`
3. Set up auto-switch logic based on environment (Tauri vs Web)
4. Verify: Vue dev server runs, can switch between API modes

---

## 8. Workspace `Cargo.toml` Structure

```toml
[workspace]
members = [
    "crates/core",
    "crates/cli",
    "crates/web",
    "crates/wasm",
    "src-tauri",
]

[workspace.dependencies]
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", features = ["json", "cookies", "stream"] }
scraper = "0.26"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"
chrono = "0.4"
futures = "0.3"
dotenvy = "0.15"
askama = "0.15"
regex = "1"

[profile.release]
opt-level = 3
```

---

## 9. Success Criteria

- [ ] `cargo build` succeeds from workspace root
- [ ] `cargo run -- run --format both` works (CLI mode)
- [ ] `cargo run -- web --port 8080` starts Axum server (Web mode)
- [ ] `cargo tauri dev` launches desktop app (Tauri mode)
- [ ] Vue frontend scaffolds and dev server runs
- [ ] All existing functionality preserved (no regressions)
- [ ] `cargo test` passes (excluding `#[ignore]` tests)
