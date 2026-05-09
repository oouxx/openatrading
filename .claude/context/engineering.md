# Engineering

## Tech Stack

### 当前

| Layer | Tech | Notes |
|-------|------|-------|
| Frontend | Vue 3.5, TypeScript 5.7, Vite 6 | `src/` |
| Frontend deps | Pinia 3, lightweight-charts 5, TailwindCSS 3.4 | `src/package.json` |
| Package manager | Bun | `bun.lock` |
| Backend | Rust (workspace) | `Cargo.toml` |
| Crates | core, cli, web, wasm | `crates/` |
| Key Rust deps | tokio, reqwest, scraper, serde, chrono, askama | |
| LLM | OpenRouter | `crates/core/src/llm/openrouter.rs` |
| Deploy | Docker Compose, Vercel (static), RackNerd | |

### 规划中

当前架构需要重构，计划引入：

| 新增 | 用途 |
|------|------|
| Vue Query | 服务端状态管理、缓存、自动刷新 |
| motion-v | Vue 动画库（Motion for Vue） |
| Redis | 缓存层、实时数据 |
| PostgreSQL | 持久化存储、历史数据查询 |

## Architecture

```
.
├── crates/               # Rust workspace
│   ├── core/             #   shared lib: API clients, agents, LLM, report
│   ├── cli/              #   CLI binary (fetch data, run analysis)
│   ├── web/              #   Web server binary
│   └── wasm/             #   WASM target stub
├── src/                  # Vue 3 frontend (own package.json / tsconfig)
│   ├── src/
│   │   ├── components/   #   Vue components + tabs/
│   │   └── stores/       #   Pinia stores
│   ├── public/data/      #   JSON data for static mode
│   └── dist/             #   Vite build output
├── tests/                # Integration tests (jiuyan.rs, xuangubao.rs)
├── docs/                 # API docs, design docs
├── Makefile              # All project commands
├── docker-compose.yaml
└── deploy-racknerd.sh
```

## Commands

```bash
# Dev
make dev-frontend       # Vue dev server (cd src && bun run dev)
make run                # CLI fetch + report
make run-ai             # CLI + AI analysis
make run-web            # Web server

# Quality
make test               # cargo test
make lint               # cargo clippy
make fmt                # cargo fmt
make check              # fmt + lint + test

# Deploy
make deploy-racknerd    # Deploy to RackNerd
make docker-up          # Docker Compose
make deploy-static      # Build Vue + generate report.json

# Help
make help               # All targets
```

## Environment

`.env` at project root for API keys. See `.claude/context/glossary.md` for domain terms.

## Gotchas

- Frontend is in `src/`, NOT root. `package.json` / `tsconfig.json` are in `src/`.
- Some make targets call `cargo run --bin trending-agent` which reads `.env`.
- `deploy-static` generates JSON into `src/public/` then copies to `src/public/data/report.json`.
- `bun.lock` not `package-lock.json` as primary lock file.
- The project uses feature-based organization in the Vue frontend (tabs, components).
