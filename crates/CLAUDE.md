# Rust Workspace

## 测试（限定此目录）

```bash
cargo test -p trending-agent-core
cargo test -p trending-agent-cli
cargo test -p trending-agent-web
```

## Lint（限定此目录）

```bash
cargo clippy -p trending-agent-core -- -D warnings
cargo clippy -p trending-agent-cli -- -D warnings
cargo clippy -p trending-agent-web -- -D warnings
```

## 约定

- 业务逻辑只放在 `core/`，CLI/Web 不做数据逻辑
- `core/` 模块按功能分目录：`api/`、`cron/`、`gateway/`、`llm/`、`memory/`、`trade/` 等
- 配置三层优先级：项目根 `config.toml` > `.atrading/config.toml` > `~/.atrading/config.toml`
- 错误处理用 `anyhow::Result`，上层用 `.context()` 添加上下文
- 异步 runtime 是 `tokio`（multi-thread）
- DB 是 SQLite WAL 模式，路径 `.atrading/data.db`
- LLM 调用统一走 `core/src/llm/openrouter.rs`
- cron 表达式 7 字段：`秒 分 时 日 月 周 年`
