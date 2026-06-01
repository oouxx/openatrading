# OpenATrading

A股市场异动追踪工具，从选股宝和韭研公社获取数据，生成 Markdown/HTML 分析报告。

## 使用方法

```bash
# 指定数据源（逗号分隔）
openatrading -d limit_up                          # 涨停数据
openatrading -d limit_down                        # 跌停数据
openatrading -d overview                          # 市场概览
openatrading -d field_item                        # 韭研公社异动数据
openatrading -d timeline                          # 韭研公社时间线
openatrading -d community                         # 韭研公社社群文章
openatrading -d cls_telegraph                     # 财联社电报
openatrading -d sentiment                         # 舆情分析
openatrading                                      # 全量数据（所有数据源）

# 组合数据源 + 格式
openatrading -d limit_up,overview --json           # 涨停+概览，JSON 输出
openatrading -d limit_up,overview,sentiment --ai   # 多数据源 + AI 分析
openatrading --html -o report.html                 # HTML 格式输出到文件

# 选项
openatrading --ai              # 启用 AI 分析
openatrading --json            # JSON 格式输出
openatrading --html            # HTML 格式输出
openatrading --markdown        # Markdown 格式（默认）
openatrading -o ./reports      # 指定输出文件
openatrading --tgb-user <id>   # 淘股吧用户 ID（community 数据源）
openatrading --cls-category x  # 财联社电报分类
```

## 环境变量

| 变量                 | 必填      | 说明               |
| -------------------- | --------- | ------------------ |
| `OPENROUTER_API_KEY` | AI分析时  | OpenRouter API Key |
| `JIUYAN_PHONE`       | 可选      | 韭研公社手机号     |
| `JIUYAN_PASSWORD`    | 可选      | 韭研公社密码       |
| `JIUYAN_INIT_TOKEN`  | 可选      | 韭研公社初始 Token |

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

---

## MCP 接入（OpenClaw / Hermes Agent）

openatrading 通过 **MCP (Model Context Protocol)** 暴露为标准化工具，可被 OpenClaw 和 NousResearch Hermes Agent 调用。以下介绍如何搭建 MCP Server 包装层并配置接入。

### 整体架构

```
OpenClaw Gateway ──┐
                    ├── MCP Client ── openatrading-mcp (stdio MCP Server) ── openatrading CLI
Hermes Agent ──────┘
```

MCP Server 作为中间层，将 CLI 子命令映射为 MCP tools，由 OpenClaw/Hermes 通过 stdio 协议调用。

### Step 1：搭建 MCP Server

在项目根目录创建 `mcp-server/`，使用 Bun + `@modelcontextprotocol/sdk`：

```bash
mkdir -p mcp-server
cd mcp-server
npm init -y
bun add @modelcontextprotocol/sdk
```

`mcp-server/index.js`：

```js
import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import {
  CallToolRequestSchema,
  ListToolsRequestSchema,
} from "@modelcontextprotocol/sdk/types.js";
import { execSync } from "node:child_process";

const CLI = "openatrading";

// tool → --data 参数映射
const TOOL_DATA_MAP: Record<string, string> = {
  limit_up: "limit_up",
  limit_down: "limit_down",
  overview: "overview",
  field_item: "field_item",
  timeline: "timeline",
  community: "community",
  cls_telegraph: "cls_telegraph",
  sentiment: "sentiment",
  super_stock: "super_stock",
  plate_abnormal: "plate_abnormal",
  dapan_abnormal: "dapan_abnormal",
  market_trend: "market_trend",
};

const ALL_DATA_SOURCES = Object.values(TOOL_DATA_MAP).join(",");

const TOOLS = {
  limit_up: {
    name: "limit_up",
    description: "获取A股涨停板数据（选股宝）",
    inputSchema: {
      type: "object",
      properties: {
        ai: { type: "boolean", description: "启用 AI 分析" },
        format: { type: "string", enum: ["json", "markdown", "html"], description: "输出格式" },
      },
    },
  },
  limit_down: {
    name: "limit_down",
    description: "获取A股跌停板数据（选股宝）",
    inputSchema: {
      type: "object",
      properties: {
        ai: { type: "boolean" },
        format: { type: "string", enum: ["json", "markdown", "html"] },
      },
    },
  },
  overview: {
    name: "overview",
    description: "获取A股市场概览（选股宝情绪周期、空间板、龙头梯队）",
    inputSchema: {
      type: "object",
      properties: {
        ai: { type: "boolean" },
        format: { type: "string", enum: ["json", "markdown", "html"] },
      },
    },
  },
  field_item: {
    name: "field_item",
    description: "获取韭研公社题材异动数据",
    inputSchema: {
      type: "object",
      properties: {
        ai: { type: "boolean" },
        format: { type: "string", enum: ["json", "markdown", "html"] },
      },
    },
  },
  timeline: {
    name: "timeline",
    description: "获取韭研公社事件时间线",
    inputSchema: {
      type: "object",
      properties: {
        ai: { type: "boolean" },
        format: { type: "string", enum: ["json", "markdown", "html"] },
      },
    },
  },
  community: {
    name: "community",
    description: "获取韭研公社社群文章",
    inputSchema: {
      type: "object",
      properties: {
        ai: { type: "boolean" },
        format: { type: "string", enum: ["json", "markdown", "html"] },
        tgb_user: { type: "string", description: "淘股吧用户 ID" },
      },
    },
  },
  cls_telegraph: {
    name: "cls_telegraph",
    description: "获取财联社电报（快讯）",
    inputSchema: {
      type: "object",
      properties: {
        ai: { type: "boolean" },
        format: { type: "string", enum: ["json", "markdown", "html"] },
        category: { type: "string", description: "电报分类" },
      },
    },
  },
  sentiment: {
    name: "sentiment",
    description: "获取A股舆情分析数据",
    inputSchema: {
      type: "object",
      properties: {
        format: { type: "string", enum: ["json", "markdown", "html"] },
      },
    },
  },
  super_stock: {
    name: "super_stock",
    description: "获取强势股/妖股数据（选股宝）",
    inputSchema: {
      type: "object",
      properties: {
        ai: { type: "boolean" },
        format: { type: "string", enum: ["json", "markdown", "html"] },
      },
    },
  },
  plate_abnormal: {
    name: "plate_abnormal",
    description: "获取板块异动数据",
    inputSchema: {
      type: "object",
      properties: {
        ai: { type: "boolean" },
        format: { type: "string", enum: ["json", "markdown", "html"] },
      },
    },
  },
  run: {
    name: "run",
    description: "运行完整报告（所有数据源），可选 AI 分析",
    inputSchema: {
      type: "object",
      properties: {
        ai: { type: "boolean", description: "启用 AI 分析" },
        data: { type: "string", description: "数据源，逗号分隔。默认全部。可选: limit_up,limit_down,super_stock,field_item,overview,plate_abnormal,timeline,community,sentiment,cls_telegraph,dapan_abnormal,market_trend" },
        format: { type: "string", enum: ["json", "markdown", "html"], description: "输出格式" },
      },
    },
  },
};

const server = new Server(
  { name: "openatrading", version: "1.0.0" },
  { capabilities: { tools: {} } }
);

server.setRequestHandler(ListToolsRequestSchema, async () => ({
  tools: Object.values(TOOLS),
}));

server.setRequestHandler(CallToolRequestSchema, async (request) => {
  const { name, arguments: args } = request.params;
  const tool = TOOLS[name];
  if (!tool) throw new Error(`Unknown tool: ${name}`);

  const cargs = (args ?? {}) as Record<string, unknown>;
  const cmdParts = [CLI];

  // 数据源：run 默认全部，其他 tool 映射到对应数据源
  if (name === "run") {
    if (cargs.data) cmdParts.push("-d", cargs.data as string);
    // 不传 data 则 CLI 默认获取全量
  } else {
    const ds = TOOL_DATA_MAP[name];
    if (ds) cmdParts.push("-d", ds);
  }

  if (cargs.ai) cmdParts.push("--ai");
  if (cargs.format) {
    if (cargs.format === "json") cmdParts.push("--json");
    else if (cargs.format === "html") cmdParts.push("--html");
    else cmdParts.push("--markdown");
  }
  if (cargs.tgb_user) cmdParts.push("--tgb-user", cargs.tgb_user as string);
  if (cargs.category) cmdParts.push("--cls-category", cargs.category as string);

  const cmd = cmdParts.join(" ");
  const result = execSync(cmd, {
    encoding: "utf-8",
    maxBuffer: 10 * 1024 * 1024,
    timeout: 120_000,
  });

  return { content: [{ type: "text", text: result }] };
});

const transport = new StdioServerTransport();
await server.connect(transport);
```

在 `package.json` 中设置 `"type": "module"` 并添加启动脚本：

```json
{
  "type": "module",
  "scripts": {
    "start": "bun index.js"
  }
}
```

### Step 2：接入 OpenClaw

OpenClaw 通过 `~/.openclaw/openclaw.json` 配置 MCP servers。支持两种方式：

**方式 A — 通过 mcporter 管理（推荐）**

```bash
bun add -g mcporter

mcporter config add openatrading \
  --stdio "node /path/to/openatrading/mcp-server/index.js"

# 重启 gateway
openclaw gateway restart
```

**方式 B — 直接写入 openclaw.json**

编辑 `~/.openclaw/openclaw.json`：

```json5
{
  "mcp": {
    "servers": [
      {
        "name": "openatrading",
        "transport": "stdio",
        "command": "bun",
        "args": ["/path/to/openatrading/mcp-server/index.js"]
      }
    ]
  }
}
```

重启生效：

```bash
openclaw gateway restart
```

确保 OpenClaw 的 exec 工具已启用（允许 MCP server 调用本地 CLI）：

```bash
openclaw config set tools.profile full
openclaw config set tools.exec.security allowlist
openclaw gateway restart
```

### Step 3：接入 Hermes Agent

编辑 `~/.hermes/config.yaml`，添加 `mcp_servers`：

```yaml
mcp_servers:
  openatrading:
    command: "bun"
    args:
      - "/path/to/openatrading/mcp-server/index.js"
    tools:
      include:
        - limit_up
        - limit_down
        - overview
        - field_item
        - timeline
        - community
        - cls_telegraph
        - sentiment
        - super_stock
        - plate_abnormal
        - run
```

启动 Hermes 并重载 MCP：

```bash
hermes chat
```

在对话中输入：

```
/reload-mcp
/tools list
```

验证 `openatrading-*` 工具已出现在 tools 列表中。

### 可用 MCP Tools

| Tool | 对应 CLI 调用 | 说明 |
|------|-------------|------|
| `limit_up` | `openatrading -d limit_up` | 涨停板数据（选股宝） |
| `limit_down` | `openatrading -d limit_down` | 跌停板数据（选股宝） |
| `overview` | `openatrading -d overview` | 市场概览（情绪周期、空间板、龙头梯队） |
| `field_item` | `openatrading -d field_item` | 韭研公社题材异动 |
| `timeline` | `openatrading -d timeline` | 韭研公社事件时间线 |
| `community` | `openatrading -d community` | 韭研公社社群文章 |
| `cls_telegraph` | `openatrading -d cls_telegraph` | 财联社电报/快讯 |
| `sentiment` | `openatrading -d sentiment` | A股舆情分析 |
| `super_stock` | `openatrading -d super_stock` | 强势股/妖股 |
| `plate_abnormal` | `openatrading -d plate_abnormal` | 板块异动 |
| `run` | `openatrading` (默认全量) 或 `openatrading -d ...` | 全量报告 + 可选 AI 分析 |

### 典型工作流

**在 OpenClaw 中：**

```
> 帮我看看今天A股涨停情况，AI分析一下
→ OpenClaw 调用 limit_up tool (ai=true)，获取涨停数据 + AI 解读
→ 返回格式化后的涨停报告

> 生成一份完整的市场报告，包括情绪周期
→ OpenClaw 调用 run tool (ai=true, data="limit_up,overview,sentiment,market_trend")
→ 返回全量 AI 分析报告
```

**在 Hermes Agent 中：**

```
> 获取今天的市场概览，关注空间板和龙头梯队
→ Hermes 调用 openatrading-overview
→ 解析并呈现情绪周期数据

> 韭研公社最近有什么异动？
→ Hermes 调用 openatrading-field_item + openatrading-timeline
→ 汇总题材和事件
```

### 环境变量透传

MCP Server 继承调用方的环境变量。确保以下变量在 OpenClaw / Hermes 环境中已设置：

```bash
# LLM（AI 分析时需要）
export LLM_API_KEY=sk-xxxxx
export LLM_MODEL=deepseek-chat

# 韭研公社（field/timeline/community 需要）
export JIUYAN_PHONE=138xxxx
export JIUYAN_PASSWORD=xxxx
```

对于 OpenClaw，可在 `openclaw.json` 中为 MCP server 指定环境变量：

```json5
{
  "mcp": {
    "servers": [
      {
        "name": "openatrading",
        "transport": "stdio",
        "command": "bun",
        "args": ["/path/to/openatrading/mcp-server/index.js"],
        "env": {
          "LLM_API_KEY": "${LLM_API_KEY}",
          "LLM_MODEL": "deepseek-chat",
          "JIUYAN_PHONE": "${JIUYAN_PHONE}",
          "JIUYAN_PASSWORD": "${JIUYAN_PASSWORD}"
        }
      }
    ]
  }
}
```

Hermes Agent 中在 `mcp_servers` 下添加 `env` 字段：

```yaml
mcp_servers:
  openatrading:
    command: "bun"
    args: ["/path/to/openatrading/mcp-server/index.js"]
    env:
      LLM_API_KEY: ${LLM_API_KEY}
      LLM_MODEL: deepseek-chat
```
