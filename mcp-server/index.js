#!/usr/bin/env bun

import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import {
  CallToolRequestSchema,
  ListToolsRequestSchema,
} from "@modelcontextprotocol/sdk/types.js";
import { accessSync, constants } from "node:fs";

// ── Resolve CLI binary ──────────────────────────────────────

const CLI =
  process.env.OPENATRADING_BIN ??
  (() => {
    const candidates = [
      "./target/release/openatrading",
      "./target/debug/openatrading",
    ];
    for (const c of candidates) {
      try {
        accessSync(c, constants.X_OK);
        return c;
      } catch {
        /* not found */
      }
    }
    return "openatrading"; // hope it's on PATH
  })();

function checkCli() {
  try {
    accessSync(CLI, constants.X_OK);
  } catch {
    throw new Error(
      `openatrading binary not found at "${CLI}". ` +
        `Build with "cargo build --release -p openatrading-cli" or set OPENATRADING_BIN env var.`
    );
  }
}

// ── Tool definitions ────────────────────────────────────────

/** @see crates/cli/src/cli/mod.rs — ALL_DATA_SOURCES */
const TOOL_DATA_MAP = {
  limit_up:        "limit_up",
  limit_down:      "limit_down",
  overview:         "overview",
  field_item:       "field_item",
  timeline:         "timeline",
  community:        "community",
  cls_telegraph:    "cls_telegraph",
  sentiment:        "sentiment",
  super_stock:      "super_stock",
  plate_abnormal:   "plate_abnormal",
  dapan_abnormal:   "dapan_abnormal",
  market_trend:     "market_trend",
};

const ALL_DATA_SOURCES = Object.values(TOOL_DATA_MAP).join(",");

function formatSchema(description = "输出格式") {
  return { type: "string", enum: ["json", "markdown", "html"], description };
}

function aiProp(description = "启用 AI 分析（需要 LLM_API_KEY 环境变量）") {
  return { type: "boolean", description };
}

function baseProps(opts = {}) {
  return {
    ...(opts.noAi ? {} : { ai: aiProp() }),
    format: formatSchema(),
    ...(opts.extra ?? {}),
  };
}

const TOOLS = {
  limit_up: {
    name: "limit_up",
    description: "获取A股涨停板数据（选股宝）—— 涨停股列表、封板时间、封单额、连板数",
    inputSchema: { type: "object", properties: baseProps() },
  },
  limit_down: {
    name: "limit_down",
    description: "获取A股跌停板数据（选股宝）—— 跌停股列表、封单额",
    inputSchema: { type: "object", properties: baseProps() },
  },
  overview: {
    name: "overview",
    description: "获取A股市场概览（选股宝）—— 情绪周期、空间板、龙头梯队、涨停/跌停统计",
    inputSchema: { type: "object", properties: baseProps() },
  },
  field_item: {
    name: "field_item",
    description: "获取韭研公社题材异动数据 —— 当日热门题材、异动板块",
    inputSchema: { type: "object", properties: baseProps() },
  },
  timeline: {
    name: "timeline",
    description: "获取韭研公社事件时间线 —— 当月重要事件日历",
    inputSchema: { type: "object", properties: baseProps() },
  },
  community: {
    name: "community",
    description: "获取韭研公社社群文章 —— 淘股吧用户帖子",
    inputSchema: {
      type: "object",
      properties: baseProps({
        extra: { tgb_user: { type: "string", description: "淘股吧用户 ID" } },
      }),
    },
  },
  cls_telegraph: {
    name: "cls_telegraph",
    description: "获取财联社电报/快讯 —— 实时财经新闻电报",
    inputSchema: {
      type: "object",
      properties: baseProps({
        extra: { category: { type: "string", description: "电报分类过滤" } },
      }),
    },
  },
  sentiment: {
    name: "sentiment",
    description: "获取A股舆情分析 —— 市场情绪指标",
    inputSchema: { type: "object", properties: baseProps({ noAi: true }) },
  },
  super_stock: {
    name: "super_stock",
    description: "获取强势股/妖股数据（选股宝）—— 当日强势股、连板妖股",
    inputSchema: { type: "object", properties: baseProps() },
  },
  plate_abnormal: {
    name: "plate_abnormal",
    description: "获取板块异常波动数据 —— 放量/异动板块",
    inputSchema: { type: "object", properties: baseProps() },
  },
  run: {
    name: "run",
    description:
      "运行完整市场报告 —— 拉取所有数据源，可选 AI 情绪周期分析。" +
      "当用户问「今天市场怎么样」「今天行情如何」「做个全面分析」时用这个。",
    inputSchema: {
      type: "object",
      properties: {
        data: {
          type: "string",
          description: `指定数据源（逗号分隔），默认全部。可选: ${ALL_DATA_SOURCES}`,
        },
        ai: aiProp(),
        format: formatSchema(),
      },
    },
  },
};

// ── Build CLI command from tool call ────────────────────────

/**
 * @param {string} name - tool name
 * @param {Record<string, unknown>} args - tool arguments
 * @returns {string[]}
 */
function buildCommand(name, args = {}) {
  const parts = [CLI];

  if (name === "run") {
    if (args.data) parts.push("-d", String(args.data));
  } else {
    const ds = TOOL_DATA_MAP[name];
    if (ds) parts.push("-d", ds);
  }

  if (args.ai) parts.push("--ai");

  if (args.format) {
    switch (args.format) {
      case "json":
        parts.push("--json");
        break;
      case "html":
        parts.push("--html");
        break;
      default:
        break; // default = markdown, no flag needed
    }
  }

  if (args.tgb_user) parts.push("--tgb-user", String(args.tgb_user));
  if (args.category) parts.push("--cls-category", String(args.category));

  return parts;
}

/** Strip ANSI escape codes */
function stripAnsi(str) {
  // eslint-disable-next-line no-control-regex
  return str.replace(/\x1b\[[0-9;]*m/g, "");
}

// ── Execute CLI ─────────────────────────────────────────────

async function execCli(cmdParts) {
  const displayCmd = cmdParts.map((p) => (p.includes(" ") ? `"${p}"` : p)).join(" ");

  console.error(`[openatrading-mcp] $ ${displayCmd}`);

  const proc = Bun.spawn(cmdParts, {
    stdout: "pipe",
    stderr: "pipe",
    timeout: 120_000, // 2 minutes
  });

  const [outBuf, errBuf] = await Promise.all([
    new Response(proc.stdout).arrayBuffer(),
    new Response(proc.stderr).arrayBuffer(),
  ]);

  const decoder = new TextDecoder();
  const stdout = stripAnsi(decoder.decode(outBuf)).trimEnd();
  const stderr = stripAnsi(decoder.decode(errBuf)).trimEnd();

  if (proc.exitCode !== 0) {
    throw Object.assign(new Error(`CLI exited with code ${proc.exitCode}`), {
      stderr,
      stdout,
      code: proc.exitCode,
    });
  }

  if (proc.killed) {
    throw Object.assign(new Error("CLI command timed out after 120s"), {
      stderr,
      stdout,
      code: "TIMEOUT",
    });
  }

  return { stdout, stderr };
}

// ── Server setup ────────────────────────────────────────────

const server = new Server(
  { name: "openatrading", version: "1.0.0" },
  { capabilities: { tools: {} } }
);

server.setRequestHandler(ListToolsRequestSchema, async () => ({
  tools: Object.values(TOOLS),
}));

server.setRequestHandler(CallToolRequestSchema, async (request) => {
  const { name, arguments: args } = request.params;

  if (!TOOLS[name]) {
    throw new Error(`Unknown tool: ${name}. Available: ${Object.keys(TOOLS).join(", ")}`);
  }

  checkCli();

  const cmdParts = buildCommand(name, args ?? {});

  try {
    const { stdout } = await execCli(cmdParts);

    return {
      content: [
        { type: "text", text: stdout || "(CLI returned empty output)" },
      ],
    };
  } catch (err) {
    const message = err.stderr
      ? `${err.message}\n\nStderr:\n${err.stderr}`
      : err.message;

    console.error(`[openatrading-mcp] ERROR: ${message}`);
    return {
      content: [{ type: "text", text: `Error: ${message}` }],
      isError: true,
    };
  }
});

// ── Entry point ─────────────────────────────────────────────

const transport = new StdioServerTransport();
await server.connect(transport);

console.error(`[openatrading-mcp] Server started (bun ${Bun.version})`);
console.error(`[openatrading-mcp] CLI binary: ${CLI}`);