# AGENTS.md — trending-agent 架构概览

## 项目概述

A股市场异动追踪工具。从选股宝、韭研公社、淘股吧三个数据源获取行情数据，结合 AI 多角色分析（游资操盘手/量化分析师/风控官），生成 Markdown/HTML 报告，支持终端输出、文件保存和 Web UI 预览。

**技术栈**：Rust + Tokio + Reqwest + Scraper + Askama + Axum + Clap

**数据流**：API 拉取 → Report 聚合 → 模板渲染 → 多格式输出（终端/文件/Web）

## 构建与命令

```bash
# 构建
cargo build
cargo build --release

# 运行（无参数默认执行 run，生成完整报告）
cargo run
cargo run -- run --format both --web --port 8080

# 单数据源
cargo run -- limit-up          # 涨停
cargo run -- limit-down        # 跌停
cargo run -- overview          # 市场概览
cargo run -- plate-abnormal    # 板块异动
cargo run -- field             # 韭研异动
cargo run -- timeline          # 时间线
cargo run -- community         # 社群文章

# AI 分析
cargo run -- sentiment         # 舆情分析
cargo run -- technical         # 技术分析
cargo run -- run --ai          # 完整报告 + AI 分析

# 交互模式
cargo run -- agent

# 淘股吧
cargo run -- save-user 2577911

# 测试（韭研公社测试需环境变量，默认 ignore）
cargo test
cargo test -- --ignored        # 运行集成测试

# Docker
docker compose up -d
```

## 代码风格

- **错误处理**：统一使用 `anyhow::Result`，不使用自定义 error 类型
- **异步**：所有 IO 操作为 async，运行时 `tokio`（full features）
- **并行**：独立数据源用 `tokio::join!` 并发拉取
- **命名**：模块用 snake_case，结构体/枚举用 PascalCase，函数用 snake_case
- **CLI 中文**：用户可见输出使用中文 + emoji 前缀（📡✅⚠️📝📊📈）
- **格式化**：使用 `cargo fmt`，长链式调用换行
- **模块组织**：每个子模块有 `mod.rs` 声明公共接口，实现放在同名文件中

## 核心架构

### API 层（`src/api/`）

三个数据源客户端聚合在 `api::Client` 中：

| 客户端 | 数据源 | 认证 | 方式 |
|--------|--------|------|------|
| `XuanguBaoClient` | 选股宝 | 无 | 公开 JSON API |
| `JiuYanSession` | 韭研公社 | 手机号+密码登录 | JSON API + token 缓存 |
| `TgbClient` | 淘股吧 | 可选 cookie | HTML 爬虫 |

**韭研公社认证机制**（最复杂）：
- `RwLock<InnerSession>` 保护 token/cookie
- `AtomicBool` 防止并发重复登录
- 认证失败自动重试一次（检测 errCode=1/9）
- 未配置 `JIUYAN_PHONE` 时跳过，不阻塞主流程

**淘股吧爬虫要点**：
- PC 端 `#first` / `.article-text` / `div.p_coten` 选择器提取正文
- 检测 `.login-view-button` 登录墙，有则回退移动端 `m.tgb.cn`（`.tzitem_text` 选择器）
- 图片懒加载：`src2` → `src` 替换，优先使用 `data-original`（高清大图）
- `html_to_markdown()`：正则管道处理（img→Markdown → br→换行 → 移除标签 → 解码实体）

### Agent 层（`src/agents/`）

| Agent | 数据源 | 输出 |
|-------|--------|------|
| `SentimentAgent` | 韭研公社社群文章 | 热门股票/文章、趋势标签、摘要 |
| `TechnicalAgent` | 选股宝涨停数据 | 连板梯队、热门板块、风险信号 |
| `MainAgent` | 组合以上两者 | 角色化 LLM 分析 |

### LLM 集成（`src/llm/openrouter.rs`）

- 默认模型 `deepseek/deepseek-chat`，可通过 `OPENROUTER_MODEL` 覆盖
- 支持流式（`chat_stream`）和非流式（`chat`）调用
- SSE 协议解析：`data: ` 前缀，`[DONE]` 结束标记
- 参数：`max_tokens=400`, `temperature=0.7`

### 角色系统（`src/roles/analyst.rs`）

三个角色各有 persona，`build_prompt()` 组合角色人设 + 市场数据 + 连板高度 + 热门行业，要求 150 字内分析。

### 报告系统（`src/report/` + `src/templates/`）

- `Report` 结构体聚合所有数据，`Reporter` 负责输出
- Askama 模板双轨：Markdown 和 HTML 各 8 个模板，完全对称
- 模板文件在 `templates/markdown/` 和 `templates/html/`
- `ReportMdTemplate`/`ReportHtmlTemplate` 为组合模板，嵌入各子模板渲染结果

### Web UI（`src/web/`）

- Axum 框架，`WebState` 通过 `Arc<Mutex<Option<Report>>>` 共享报告
- 路由：`GET /` 首页，`GET /report?format=markdown|html` 报告内容

## 测试

- **框架**：Rust 内置 `#[test]`，无额外测试框架
- **现有测试**：仅在 `jiuyangongshe.rs` 中，7 个集成测试（全部 `#[ignore]`）+ 1 个 JSON 解析单元测试
- **集成测试**需要环境变量 `JIUYAN_PHONE`，用 `cargo test -- --ignored` 运行
- **无 mock**：当前无 HTTP mock，集成测试直接请求真实 API

## 安全

- **敏感信息**：`.env` 文件包含 API key 和登录凭证，已在 `.gitignore` 中
- **环境变量**：`OPENROUTER_API_KEY`（LLM）、`JIUYAN_PHONE`（韭研公社登录）
- **HTTP 客户端**：淘股吧使用浏览器 User-Agent 模拟，支持 cookie 认证
- **无输入校验**：CLI 参数直接传递，无额外校验（依赖 clap 的类型检查）
- **Docker**：多阶段构建，运行时镜像仅含 `ca-certificates` + 二进制

## 配置

通过 `.env` 文件管理（`dotenvy` 加载）：

| 变量 | 必需 | 用途 |
|------|------|------|
| `OPENROUTER_API_KEY` | AI 分析必需 | OpenRouter LLM API key |
| `OPENROUTER_MODEL` | 可选 | 覆盖默认模型 |
| `JIUYAN_PHONE` | 韭研公社必需 | 登录手机号 |

**默认值**：
- 报告输出目录：`./reports`
- 淘股吧输出目录：`./methodology/tgb/{user_id}/`
- 报告格式：`markdown`
- Web 端口：`8080`
- LLM 模型：`deepseek/deepseek-chat`
