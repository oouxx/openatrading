# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Workspace structure

```
trending-agent/
├── Cargo.toml                # workspace root (core + cli + web + wasm)
├── crates/
│   ├── core/                 # pure business logic (no framework deps)
│   ├── cli/                  # CLI binary (clap + rustyline)
│   ├── web/                  # Axum web server
│   └── wasm/                 # WASM bindings (wasm-bindgen)
├── src/                      # Vue 3 frontend (Vite + TypeScript + Tailwind)
├── src-tauri/                # Tauri desktop app
├── templates/                # Askama templates (markdown/ + html/)
├── dist/                     # static deploy output (committed to git)
└── tests/                    # integration tests
```

**Key principle:** `crates/core` is framework-agnostic — no Clap, no Axum, no Tauri. All delivery mechanisms (CLI/web/Tauri/WASM) are thin wrappers that call `core`.

## Build & Run

```bash
cargo build                          # build all workspace crates
cargo build --release
cargo run                            # CLI binary (default: full report → ./reports)
cargo run -- run --ai --web          # full report + AI + Web UI

# Single data source
cargo run -- <subcommand> -f html -o ./reports

# Sub-page generation (static deploy)
cargo run -- taoguba --users 2577911 --output ./dist/taoguba
cargo run -- xuangubao --output ./dist/xuangubao
cargo run -- index --dir ./dist

# Frontend (Vue 3)
cd src && npm install && npm run dev

# Tauri desktop
cargo tauri dev

# Tests
cargo test                           # unit tests (integration tests are #[ignore])
cargo test -- --ignored              # integration tests (needs JIUYAN_PHONE)

# Lint & format
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings

# Docker
docker compose up -d
```

## Architecture

**Data flow**: API clients (core) → `Report` struct (core) → Askama templates → multi-format output (terminal / file / Web UI / WASM)

### Crates

**`crates/core`** — All business logic. Dependencies: tokio, reqwest, scraper, serde, anyhow, chrono, askama, regex. NO framework deps (no Clap, Axum, Tauri).

| Module | Content |
|--------|---------|
| `api/` | `XuanguBaoClient` (public JSON API), `JiuYanSession` (phone login + token cache), `TgbClient` (HTML scraper) |
| `agents/` | `SentimentAgent`, `TechnicalAgent`, `MainAgent` |
| `llm/` | OpenRouter integration (streaming + non-streaming, SSE protocol) |
| `report/` | `Report` struct + `Reporter` (output to terminal/file) |
| `roles/` | Four analyst roles with persona-based prompts |
| `templates/` | Askama template structs (shared across CLI/Web/Tauri/WASM) |
| `utils/` | `html_to_markdown()` conversion |

**`crates/cli`** — Clap CLI binary. Thin wrapper: parses args, calls `core` functions, passes results to `core::Reporter`.

**`crates/web`** — Axum server. `Arc<Mutex<Option<Report>>>` state. Routes: `GET /` (SPA), `GET /report?format=markdown|html`.

**`crates/wasm`** — wasm-bindgen wrapper exposing `core` functions to JS/Web.

### Three data sources

- **XuanguBaoClient** — public JSON API, no auth. All structs derive `Serialize` + `Deserialize`. `XuanGubaoExtra` bundles limit_down + plate_abnormal for composite report.
- **JiuYanSession** — phone+password login, token in `RwLock<InnerSession>`, `AtomicBool` prevents concurrent re-login. Auto-retries on auth failure (errCode 1/9). Skipped if `JIUYAN_PHONE` unset.
- **TgbClient** — HTML scraper. Falls back to mobile site (`m.tgb.cn`) on PC login wall. `TgbUserSummary` provides per-user metadata.

### Report system

`Report::new()` auto-computes: risk level (高/中/低, from rise/fall ratio + bomb rate), opportunity signals (top board leader + hot sectors), decision summary.

`Reporter` outputs: `print()` to terminal, `save_markdown()` / `save_html()` / `save_text()` to files.

Templates in `templates/markdown/` and `templates/html/` — mirrored Markdown/HTML Askama templates. Composite templates (`ReportMdTemplate` / `ReportHtmlTemplate`) embed sub-templates.

### Role system

Four roles: `Trader` (游资操盘手), `Quant` (量化分析师), `RiskOfficer` (风控官), `Zarili` (量价低吸派, distilled from taoguba user methodology). `build_prompt()` — persona + market data, capped at 150 chars.

### LLM integration

OpenRouter (`https://openrouter.ai/api/v1/chat/completions`). Default model: `deepseek/deepseek-chat`, overridable via `OPENROUTER_MODEL`. Streaming via SSE (`data: [DONE]` delimiter). Parameters: `max_tokens=400`, `temperature=0.7`.

### CLI subcommands

- Data: `limit-up`, `limit-down`, `overview`, `plate-abnormal`, `field`, `timeline`, `community`
- Composite: `run` (full report), `agent` (interactive REPL)
- AI: `sentiment`, `technical`
- Sub-pages: `taoguba`, `xuangubao`, `index`
- Data fetch: `save-user`

## Code conventions

- Error handling: `anyhow::Result` everywhere, no custom error types
- Async: all I/O via `tokio`; independent fetches use `tokio::join!` for parallelism
- CLI output: Chinese + emoji prefixes (📡 ✅ ⚠️ 📝 📊 📈)
- Pre-commit hooks: `cargo fmt` (2x), `cargo clippy --fix`, `cargo test`

## Environment variables

Loaded via `dotenvy` from `.env`:
- `OPENROUTER_API_KEY` — required for AI analysis
- `OPENROUTER_MODEL` — default: `deepseek/deepseek-chat`
- `JIUYAN_PHONE` / `JIUYAN_PASSWORD` / `JIUYAN_INIT_TOKEN` — Jiuyan auth
- `TAOGUBA_USER` / `TAOGUBA_PASSWORD` — Taoguba auth
- `AI_ENABLED` — toggle AI features

## Static deploy

`dist/` is committed to git (`.gitignore` exception) for Vercel/Cloudflare Pages.

```bash
cargo run -- run --format html --output ./dist/reports/
cargo run -- index --dir ./dist/
cargo run -- taoguba --users 2577911,123456 --output ./dist/taoguba
cargo run -- xuangubao --output ./dist/xuangubao
git add dist/ && git commit -m "chore: update reports" && git push
```
