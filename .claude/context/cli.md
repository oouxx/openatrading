# CLI

CLI 二进制名 `openatrading`，提供命令行接口，同时支持作为 skill 插件在 openclaw、hermes agent 中使用。

## 命令总览

| 命令 | 说明 |
|------|------|
| `openatrading` | 默认获取全量数据，输出 Markdown 到 stdout |
| `openatrading --json` | 输出 JSON |
| `openatrading --markdown` | 输出 Markdown |
| `openatrading --html` | 输出 HTML |
| `openatrading -d limit_up,overview` | 指定数据源 |
| `openatrading --ai` | AI 分析（默认角色） |
| `openatrading --role zarili --ai` | AI 分析（指定角色） |
| `openatrading --list-roles` | 列出所有角色 |
| `openatrading init` | 初始化 ~/.atrading/ |
| `openatrading gateway install` | 安装 systemd 服务 |
| `openatrading gateway uninstall` | 卸载 systemd 服务 |
| `openatrading gateway start` | 启动后台服务 |
| `openatrading gateway stop` | 停止后台服务 |
| `openatrading gateway status` | 查看服务状态 |
| `openatrading gateway run` | 前台运行（调试用） |
| `openatrading trade add` | 添加持仓 |
| `openatrading trade list` | 列出持仓 |
| `openatrading trade close` | 平仓 |
| `openatrading update` | 更新到最新版本 |
| `openatrading update --check` | 检查新版本 |

## 输出格式

- `--json` — JSON 序列化，默认 stdout
- `--markdown` — Markdown 格式（默认）
- `--html` — HTML 格式
- `-o <file>` — 输出到文件

JSON 直接序列化，Markdown/HTML 使用模板渲染。

## 数据源

`-d, --data <list>` 可选值: overview, limit_up, limit_down, super_stock, field_item, plate_abnormal, timeline, community, sentiment, technical, cls_telegraph, dapan_abnormal, market_trend

## 环境变量

- `LLM_API_KEY` — LLM API 密钥
- `LLM_MODEL` — 模型名称（默认 deepseek-chat）
- `LLM_API_ENDPOINT` — API 端点
- `JIUYAN_PHONE` — 韭研公社登录手机号

## 配置

三层优先级: 项目根 config.toml > 项目根 .atrading/config.toml > ~/.atrading/config.toml

Gateway 模式使用 systemd user-level 管理后台进程（cron 采集 + 决策 + Telegram bot）。

## 自更新

`openatrading update` 从 GitHub Releases 下载最新 Linux 二进制，备份旧版本后替换。仅支持 Linux x86_64 / aarch64。
