# trending-agent

## 预览

[预览](./docs/preview.png)

## 本地开发

### 安装hook

pre-commit install

### build

cargo build

### 提交前代码前运行

pre-commit run --all-files

## 项目描述
trending-agent 是一个用于追踪 A 股市场异动的分析工具。它从选股宝、韭研公社、淘股吧三个数据源获取行情数据，结合 AI 的多角色分析（游资操盘手/量化分析师/风控官），生成 Markdown/HTML 报告，支持终端输出、文件保存和 Web UI 预览。

## 技术栈
- **语言**: Rust (Edition 2021)
- **异步运行时**: Tokio
- **HTTP 客户端**: Reqwest
- **HTML 解析**: Scraper
- **CLI 框架**: Clap
- **模板引擎**: Askama (Markdown/HTML 双轨渲染)
- **Web 框架**: Axum
- **错误处理**: anyhow
- **其他依赖**: Serde, Chrono, Regex

## 快速开始
以下命令来自项目文档 AGENTS.md，供快速本地构建与运行使用。

### 构建
```bash
cargo build
cargo build --release
```

### 运行
```bash
cargo run
cargo run -- run --format both --web --port 8080
```

### 单数据源示例
```bash
cargo run -- limit-up
cargo run -- limit-down
cargo run -- overview
cargo run -- plate-abnormal
cargo run -- field
cargo run -- timeline
cargo run -- community
```

### AI 分析
```bash
cargo run -- sentiment
cargo run -- technical
cargo run -- run --ai
```

### 互动模式
```bash
cargo run -- agent
```

### 淘股吧
```bash
cargo run -- save-user 2577911
```

### 测试
> 注意：韭研公社测试需要环境变量 JIUYAN_PHONE，默认 ignore
```bash
cargo test
cargo test -- --ignored
```

### Docker
```bash
docker compose up -d
```

## 配置
请在 .env 中配置以下环境变量（示例值请用实际信息替换，切勿提交敏感数据）
```dotenv
OPENROUTER_API_KEY=your_openrouter_api_key
OPENROUTER_MODEL=deepseek/deepseek-chat
JIUYAN_PHONE=your_phone_number
REPORTS_DIR=./reports
TGB_OUTPUT_DIR=./methodology/tgb/{user_id}/
REPORT_FORMAT=markdown
WEB_PORT=8080
```
说明：
- OPENROUTER_API_KEY 为 OpenRouter 的 API Key，必填
- OPENROUTER_MODEL 为要使用的模型，非必填，默认为 deepseek/deepseek-chat
- JIUYAN_PHONE 为韭研公社登录所需手机号，若未配置将跳过登录相关流程
- REPORTS_DIR 与 TGB_OUTPUT_DIR 为输出目录，默认如上
- REPORT_FORMAT 是输出格式，常用 markdown
- WEB_PORT 是本地 Web 服务端口

## 架构概览
trending-agent 的核心分为 API 层、Agent 层和报告渲染层。
- API 层：聚合来自三个数据源的客户端，形成统一的 API 接口。
- 数据源客户端：
  - XuanguBaoClient（选股宝，公开 API，无需登录）
  - JiuYanSession（韭研公社，手机号登录，令牌缓存）
  - TgbClient（淘股吧，HTML 爬虫）
- Agent 层：负责从各数据源抽取信息、进行情感分析与技术分析并输出结构化数据。
  - SentimentAgent（情感分析） 
  - TechnicalAgent（技术分析） 
  - MainAgent（整合输出，生成角色化观点）
- LLM 集成：通过 OpenRouter 接口与多角色对话，产出分析摘要
- 模板渲染：Askama 模板渲染 Markdown/HTML 报告
- 输出与 Web 展示：终端输出、文件输出、Web UI

核心模块示意：
- src/api/：数据源聚合与 API 客户端
- src/agents/：情感/技术分析代理
- src/llm/openrouter.rs：LLM 接入
- src/report/：报告聚合与渲染
- templates/：Markdown/HTML 模板
- web/：Axum Web 服务


