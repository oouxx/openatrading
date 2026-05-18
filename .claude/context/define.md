# OpenATrading

AI-native A 股短线交易决策工作台。「AI 情绪周期分析终端」

## 核心规则

- **红涨绿跌**（A 股规则），不可颠倒
- **AI-first**：AI 是「交易副驾驶」，主动解释/提醒/追问/总结/识别风险
- **禁止明确买卖建议、预测具体涨跌点位、使用夸张营销语言**
- 深色金融终端风格（Bloomberg Terminal 方向），禁止白色/SaaS/娱乐化 UI

## 代码地图

| 目录 | 说明 |
|------|------|
| `crates/core/` | Rust 共享库：全部业务逻辑（API、LLM、cron、gateway、策略） |
| `crates/cli/` | CLI 二进制 `openatrading`：数据抓取、AI 分析、gateway 管理 |
| `crates/web/` | Web 二进制 `trending-agent-web`：axum API + Vue 前端 |
| `src/` | Vue 3 前端（独立 package.json / tsconfig / bun.lock） |
| `src-tauri/` | Tauri 桌面应用（非重点维护） |
| `.claude/context/` | 产品定义、设计系统、工程细节、术语表 |

## 常用命令

```bash
# Rust 整体
cargo build / cargo test / cargo fmt --all / cargo clippy --all-targets -- -D warnings

# CLI
cargo run -p trending-agent-cli           # 数据抓取
cargo run -p trending-agent-cli -- --ai   # AI 分析

# Web
make run-web                               # Web 服务（自动 kill 旧进程）

# 前端
make dev-frontend                          # Vue dev server
cd src && bun run build                    # 前端生产构建（含类型检查）
cd src && npx vue-tsc --noEmit             # 仅前端类型检查

# 部署
make deploy                                # 部署全部（gateway + web）
make deploy-static                         # 构建 Vue + 生成静态 report.json

# 全量检查（提交前）
make check                                 # fmt + lint + test
```

## Gotchas

- 前端在 `src/` 下，不在项目根
- CLI 二进制名 `openatrading`，Web 二进制名 `trending-agent-web`
- `bun.lock` 不是 `package-lock.json`
- cron 表达式必须 7 字段（`秒 分 时 日 月 周 年`）
- `.env` 文件在项目根，存放 API keys（LLM_API_KEY 等）
- 配置三层优先级：项目根 `config.toml` > `.atrading/config.toml` > `~/.atrading/config.toml`
- 更多细节 → `.claude/context/`

## agent执行

默认使用subagent执行