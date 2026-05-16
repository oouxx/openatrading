# Engineering

## Tech Stack

| Layer | Tech | Notes |
|-------|------|-------|
| Frontend | Vue 3.5, TypeScript 5.7, Vite 6 | `src/` |
| Frontend deps | Pinia 3, lightweight-charts 5, TailwindCSS 3.4 | `src/package.json` |
| Package manager | Bun | `bun.lock` |
| Backend | Rust (workspace) | `Cargo.toml` |
| Crates | core, cli, web, wasm, src-tauri | `crates/`, `src-tauri/` |
| Key Rust deps | tokio, reqwest, serde, chrono, clap, rusqlite, tokio-cron-scheduler, askama, async-trait, teloxide (optional) | |
| DB | SQLite (rusqlite, bundled) | `.atrading/data.db` |
| LLM | OpenRouter / DeepSeek API-compatible | `crates/core/src/llm/openrouter.rs` |
| Scheduler | tokio-cron-scheduler | cron 表达式，采集 + 决策 job |
| Telegram | teloxide (optional feature) | `crates/core/src/social/telegram.rs` |
| Deploy | Docker Compose, GitHub Releases (Linux binary + Docker image), deploy.sh | |
| Service mgmt | systemd (user-level) | `~/.config/systemd/user/openatrading.service` |

## Architecture

```
.
├── crates/               # Rust workspace
│   ├── core/             #   shared lib: all business logic
│   │   ├── agents/       #     sentiment, technical analysis
│   │   ├── api/          #     xuangubao, jiuyangongshe, cls, taoguba
│   │   ├── cron/         #     CollectionTask, DecisionTask, scheduler
│   │   ├── gateway/      #     Gateway lifecycle manager
│   │   ├── llm/          #     OpenRouter/DeepSeek client + function calling
│   │   ├── memory/       #     SQLite market snapshot store (10-day retention)
│   │   ├── report/       #     report generation
│   │   ├── roles/        #     RoleConfig, MergeMode, builtin strategies, prompt builder
│   │   ├── social/       #     NotificationSender trait + TelegramSender
│   │   ├── templates/    #     HTML/Markdown templates
│   │   ├── trade/        #     Position/TradeLog models + SQLite CRUD
│   │   └── utils/        #     HTML helpers
│   ├── cli/              #   CLI binary (openatrading)
│   │   └── cli/
│   │       ├── config.rs     # AppConfig loading, 3-layer merge
│   │       ├── gateway_cmd.rs# systemd service management
│   │       ├── trade_cmd.rs  # trade add/list/close
│   │       ├── update.rs     # self-update from GitHub Releases
│   │       └── serve.rs → gateway_cmd.rs (renamed)
│   ├── web/              #   Web server binary (independent deployment)
│   └── wasm/             #   WASM target stub
├── src/                  # Vue 3 frontend (own package.json / tsconfig)
│   ├── src/
│   │   ├── components/   #   Vue components + tabs/
│   │   └── stores/       #   Pinia stores
│   ├── public/data/      #   JSON data for static mode
│   └── dist/             #   Vite build output
├── src-tauri/            # Tauri desktop app
├── tests/                # Integration tests
├── docs/                 # API docs, design docs, plans
├── Dockerfile            # CLI gateway Docker image
├── Dockerfile.backend    # Web backend Docker image
├── Dockerfile.frontend   # nginx + frontend Docker image
├── docker-compose.yaml   # openatrading gateway service
├── docker-compose-web.yaml # web backend + frontend
├── deploy.sh             # Deploy script (gateway | web | all)
├── Makefile              # All project commands
└── nginx.conf            # nginx template for frontend
```

## Architecture Notes

- **Gateway** = CLI `openatrading gateway run` → single process running cron scheduler + Telegram bot sessions
- **Web** = independently deployed binary (`trending-agent-web`), serves API + Vue frontend
- **Prompt** = system (strategy + positions + tools) + user (market data + instruction)
- **Function calling** = LLM can call `query_market_memory` tool to access historical market data
- **Config** = 3-layer priority: project root `config.toml` > `.atrading/config.toml` > `~/.atrading/config.toml`
- **DB** = `.atrading/data.db` (SQLite WAL mode), shared by memory + trade modules
- **Roles** = defined in config.toml `[[roles]]`, strategies in `.atrading/strategies/<role>/base.md` + `supplement.md`
- **Builtin strategies** = embedded via `include_str!()` for web fallback and init seed files

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

# Docker
make docker-build       # Build all Docker images
make docker-up          # Start all services
make docker-down        # Stop all services

# Deploy
make deploy             # Deploy everything (gateway + web)
make deploy-gateway     # Deploy gateway only
make deploy-web         # Deploy web + frontend
make deploy-static      # Build Vue + generate report.json

# Version
make sync-version       # Sync git tag version to tauri + cargo

# Help
make help               # All targets
```

## Environment

`.env` at project root for API keys. See `.claude/context/glossary.md` for domain terms.

## Gotchas

- Frontend is in `src/`, NOT root. `package.json` / `tsconfig.json` are in `src/`.
- CLI binary is named `openatrading` (not `trending-agent`).
- Web binary is `trending-agent-web` (from `crates/web`).
- `deploy-static` generates JSON into `src/public/` then copies to `src/public/data/report.json`.
- `bun.lock` not `package-lock.json` as primary lock file.
- `gateway_cmd.rs` was renamed from `serve.rs`.
- Tauri app not actively maintained; focus is CLI + Docker deployment.
- Telegram feature is optional (`teloxide` behind `telegram` Cargo feature).
- `sync-version` Makefile target removed; release workflow reads version from git tag.
