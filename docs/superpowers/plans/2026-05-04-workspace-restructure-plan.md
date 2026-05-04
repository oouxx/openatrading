# Workspace Restructure Implementation Plan
**Date:** 2026-05-04
**Branch:** `feature/workspace-restructure`
**Worktree Path:** `/Users/wxx/Desktop/github/trending-agent-workspace-restructure`

## Overview
Phased migration to restructure `trending-agent` into a Cargo workspace with:
- `crates/core`: Pure business logic (zero framework deps)
- `crates/cli`: Clap CLI binary
- `crates/web`: Axum web server
- `crates/wasm`: wasm-bindgen wrapper
- `src-tauri/`: Tauri desktop app
- `src/`: Vue 3 frontend

All work happens in the isolated worktree, no changes to main branch until merge.

---

## Phase 1: Restructure Rust Crates (Core + CLI + Web + WASM)
**Goal:** Migrate existing single-crate project to workspace layout, preserve all functionality.

### Step 1.1: Set up workspace root `Cargo.toml`
- [ ] Backup current `Cargo.toml` to `Cargo.toml.bak`
- [ ] Rewrite root `Cargo.toml` as workspace root:
  ```toml
  [workspace]
  members = [
      "crates/core",
      "crates/cli",
      "crates/web",
      "crates/wasm",
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
- [ ] **Verify:** `cargo build` fails (expected, no crates yet)

### Step 1.2: Create `crates/core/`
- [ ] Create directory `crates/core/`
- [ ] Write `crates/core/Cargo.toml`:
  ```toml
  [package]
  name = "trending-agent-core"
  version = "0.1.0"
  edition = "2021"

  [dependencies]
  tokio = { workspace = true }
  reqwest = { workspace = true }
  scraper = { workspace = true }
  serde = { workspace = true }
  serde_json = { workspace = true }
  anyhow = { workspace = true }
  chrono = { workspace = true }
  futures = { workspace = true }
  dotenvy = { workspace = true }
  askama = { workspace = true }
  regex = { workspace = true }
  ```
- [ ] Move all business logic from `src/` to `crates/core/src/`:
  - `src/api/` → `crates/core/src/api/`
  - `src/agents/` → `crates/core/src/agents/`
  - `src/llm/` → `crates/core/src/llm/`
  - `src/report/` → `crates/core/src/report/`
  - `src/roles/` → `crates/core/src/roles/`
  - `src/templates/` → `crates/core/src/templates/`
  - `src/utils/` → `crates/core/src/utils/`
  - `src/lib.rs` → `crates/core/src/lib.rs`
- [ ] Update `crates/core/src/lib.rs` to keep existing `pub mod` declarations
- [ ] **Verify:** `cd crates/core && cargo build` succeeds

### Step 1.3: Create `crates/cli/`
- [ ] Create directory `crates/cli/`
- [ ] Write `crates/cli/Cargo.toml`:
  ```toml
  [package]
  name = "trending-agent-cli"
  version = "0.1.0"
  edition = "2021"

  [[bin]]
  name = "trending-agent"
  path = "src/main.rs"

  [dependencies]
  trending-agent-core = { path = "../core" }
  clap = { version = "4", features = ["derive"] }
  rustyline = { version = "18.0.0", features = ["default"] }
  anyhow = { workspace = true }
  ```
- [ ] Move CLI-specific code to `crates/cli/src/`:
  - `src/cli/` → `crates/cli/src/cli/`
  - `src/main.rs` → `crates/cli/src/main.rs`
- [ ] Update `crates/cli/src/main.rs` to import from `trending_agent_core`
- [ ] **Verify:** `cargo run -- run --format markdown` works (CLI mode)

### Step 1.4: Create `crates/web/`
- [ ] Create directory `crates/web/`
- [ ] Write `crates/web/Cargo.toml`:
  ```toml
  [package]
  name = "trending-agent-web"
  version = "0.1.0"
  edition = "2021"

  [[bin]]
  name = "trending-agent-web"
  path = "src/main.rs"

  [dependencies]
  trending-agent-core = { path = "../core" }
  axum = "0.7"
  anyhow = { workspace = true }
  ```
- [ ] Move web-specific code to `crates/web/src/`:
  - `src/web/` → `crates/web/src/web/`
- [ ] Update `crates/web/src/main.rs` to import from `trending_agent_core`
- [ ] **Verify:** `cargo run -- web --port 8080` starts Axum server

### Step 1.5: Create `crates/wasm/`
- [ ] Create directory `crates/wasm/`
- [ ] Write `crates/wasm/Cargo.toml`:
  ```toml
  [package]
  name = "trending-agent-wasm"
  version = "0.1.0"
  edition = "2021"

  [lib]
  crate-type = ["cdylib"]

  [dependencies]
  trending-agent-core = { path = "../core" }
  wasm-bindgen = "0.2"
  wasm-bindgen-futures = "0.4"
  ```
- [ ] Write `crates/wasm/src/lib.rs` with thin wasm-bindgen wrappers:
  ```rust
  use wasm_bindgen::prelude::*;

  #[wasm_bindgen]
  pub async fn run_report() -> Result<JsValue, JsValue> {
      // Thin wrapper around core::report logic
      Ok(JsValue::NULL)
  }
  ```
- [ ] **Verify:** `cd crates/wasm && cargo build --target wasm32-unknown-unknown` (requires target, skip if no WASM env)

### Step 1.6: Clean up old `src/` directory
- [ ] Remove old `src/` directory (all code moved to crates)
- [ ] Remove old `Cargo.toml` (replaced by workspace root)
- [ ] **Verify:** `cargo build` from workspace root succeeds, all existing commands work

---

## Phase 2: Set Up Tauri Desktop App
**Goal:** Add Tauri v2 desktop app that depends on `crates/core`.

### Step 2.1: Initialize Tauri project
- [ ] Install Tauri CLI: `cargo install tauri-cli --version "^2"`
- [ ] Create `src-tauri/` directory
- [ ] Initialize Tauri: `cd src-tauri && cargo tauri init --name "trending-agent" --distDir "../src/dist" --devPath "../src" --pattern "modern"`
- [ ] Update `src-tauri/Cargo.toml` to depend on `trending-agent-core`:
  ```toml
  [dependencies]
  trending-agent-core = { path = "../crates/core" }
  tauri = { version = "2", features = ["devtools"] }
  ```
- [ ] **Verify:** `cd src-tauri && cargo tauri dev` launches skeleton app

### Step 2.2: Register Tauri commands
- [ ] Add Tauri commands in `src-tauri/src/main.rs` that invoke `core` functions
- [ ] **Verify:** Tauri app can call core functions via `invoke`

---

## Phase 3: Scaffold Vue Frontend
**Goal:** Set up Vue 3 + TypeScript frontend with auto-switching API layer.

### Step 3.1: Scaffold Vue project
- [ ] Run `npm create vite@latest src -- --template vue-ts` (in worktree root)
- [ ] Install dependencies: `cd src && npm install`
- [ ] **Verify:** `cd src && npm run dev` starts Vue dev server

### Step 3.2: Create API layer
- [ ] Create `src/api/` directory
- [ ] Write `src/api/index.ts` (auto-switches between Tauri/WASM mode)
- [ ] Write `src/api/tauri.ts` (calls Tauri commands)
- [ ] Write `src/api/web.ts` (calls WASM exports)
- [ ] **Verify:** API layer compiles without errors

---

## Final Verification
- [ ] `cargo build` from workspace root succeeds
- [ ] `cargo test` passes (excluding `#[ignore]` tests)
- [ ] CLI mode: `cargo run -- run --format both` works
- [ ] Web mode: `cargo run -- web --port 8080` starts server
- [ ] Tauri mode: `cd src-tauri && cargo tauri dev` launches app
- [ ] Vue dev server: `cd src && npm run dev` runs

---

## Notes
- All work is in `feature/workspace-restructure` branch, worktree at `../trending-agent-workspace-restructure`
- No changes to main branch until ready to merge
- Follow existing code style (Chinese CLI output, anyhow errors, etc.)
- Template data for Vue UI: reference `templates/html/` and `templates/markdown/` (UI optimization later)
