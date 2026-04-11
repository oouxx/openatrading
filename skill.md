# Trending Agent

A股市场异动追踪工具，从选股宝和韭研公社获取数据，生成 Markdown/HTML 分析报告。

## 使用方法

```bash
# 基本命令
trending-agent limit-up     # 获取涨停数据
trending-agent limit-down   # 获取跌停数据
trending-agent overview     # 获取市场概览
trending-agent field        # 获取韭研公社异动数据
trending-agent timeline     # 获取韭研公社时间线数据
trending-agent community    # 获取韭研公社社群文章
trending-agent run          # 完整报告（所有数据源）

# 输出格式
trending-agent run -f markdown  # Markdown 格式（默认）
trending-agent run -f html     # HTML 格式
trending-agent run -f both     # 同时生成 MD 和 HTML

# 选项
trending-agent run --ai              # 启用 AI 分析
trending-agent run -o ./reports      # 指定输出目录
trending-agent run --web             # 启动 Web UI
trending-agent run --web --port 3000 # 自定义 Web UI 端口
```

## 环境变量

| 变量 | 必填 | 说明 |
|------|------|------|
| `OPENROUTER_API_KEY` | AI分析时 | OpenRouter API Key |
| `JIUYAN_PHONE` | 可选 | 韭研公社手机号 |
| `JIUYAN_PASSWORD` | 可选 | 韭研公社密码 |
| `JIUYAN_INIT_TOKEN` | 可选 | 韭研公社初始 Token |

## 项目结构

```
src/
├── main.rs          # 入口
├── cli/
│   ├── mod.rs       # CLI 逻辑
│   └── app.rs       # CLI 结构定义
├── api/             # 数据源客户端
├── llm/             # LLM 接口
├── report/          # 报告生成
├── roles/           # 角色分析
├── templates/       # 报告模板（Markdown/HTML）
└── web/             # Web UI 服务
```

## 输出

- 报告：`reports/report_YYYYMMDD_HHMM.md` 或 `.html`
- 格式：Markdown / HTML

## Web UI

启动后访问 `http://localhost:8080` 查看报告，支持 Markdown/HTML 格式切换。