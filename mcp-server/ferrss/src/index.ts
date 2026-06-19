import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import {
  CallToolRequestSchema,
  ListToolsRequestSchema,
  Tool,
} from "@modelcontextprotocol/sdk/types.js";
import { execSync, exec } from "child_process";
import { promisify } from "util";
import { existsSync } from "fs";
import { join, dirname } from "path";
import { fileURLToPath } from "url";

const execAsync = promisify(exec);

// Resolve autocli binary path
function findAutocli(): string {
  const envBin = process.env.FERRSS_BIN_PATH;
  if (envBin) {
    if (existsSync(envBin)) return envBin;
    try {
      execSync(`"${envBin}" --version`, { stdio: "pipe" });
      return envBin;
    } catch {
      throw new Error(
        `FERRSS_BIN_PATH is set but binary not found or not working: ${envBin}`
      );
    }
  }
  const candidates = [
    // In a Ferrss clone alongside trading-agent
    join(dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..", "Ferrss", "target", "release", "autocli"),
    // From PATH
    "autocli",
  ];
  for (const c of candidates) {
    try {
      execSync(`${c} --version`, { stdio: "pipe" });
      return c;
    } catch {
      continue;
    }
  }
  throw new Error(
    "autocli binary not found. Set FERRSS_BIN_PATH env var, or build it first: cd Ferrss && cargo build --release"
  );
}

let autocliPath: string;

function getAutocliPath(): string {
  if (!autocliPath) autocliPath = findAutocli();
  return autocliPath;
}

// Tool definitions
const TOOLS: Tool[] = [
  {
    name: "ferrss_list_adapters",
    description: "List all available site adapters (e.g., hackernews, reddit, bilibili, zhihu)",
    inputSchema: {
      type: "object",
      properties: {},
      required: [],
    },
  },
  {
    name: "ferrss_run",
    description:
      "Execute a Ferrss site adapter command. Use ferrss_list_adapters first to discover available sites. Examples: 'hackernews top --limit 10', 'bilibili hot --limit 20', 'reddit search rust --limit 5'",
    inputSchema: {
      type: "object",
      properties: {
        site: {
          type: "string",
          description: "Site/adapter name (e.g., hackernews, bilibili, reddit, twitter, zhihu)",
        },
        command: {
          type: "string",
          description: "Command for the site (e.g., top, hot, search, new, best, ask, show)",
        },
        args: {
          type: "string",
          description: "Additional arguments like --limit 10 --format json",
        },
        format: {
          type: "string",
          description: "Output format: json (recommended for agent use), table, yaml, csv, md",
          enum: ["json", "table", "yaml", "csv", "md"],
          default: "json",
        },
      },
      required: ["site", "command"],
    },
  },
  {
    name: "ferrss_read",
    description:
      "Extract main article content from a webpage using Readability. Returns title, byline, content as markdown or text.",
    inputSchema: {
      type: "object",
      properties: {
        url: {
          type: "string",
          description: "URL of the article to read",
        },
        format: {
          type: "string",
          description: "Output format",
          enum: ["markdown", "text", "html", "json"],
          default: "markdown",
        },
      },
      required: ["url"],
    },
  },
  {
    name: "ferrss_explore",
    description:
      "Explore a website's API surface to discover endpoints and data patterns. Useful for understanding how to scrape a site.",
    inputSchema: {
      type: "object",
      properties: {
        url: {
          type: "string",
          description: "URL to explore",
        },
        goal: {
          type: "string",
          description: "Hint for capability naming (e.g., search, hot, trending)",
        },
      },
      required: ["url"],
    },
  },
  {
    name: "ferrss_generate",
    description:
      "One-shot: explore + synthesize + generate a new adapter for any URL. Creates working scraping rules automatically.",
    inputSchema: {
      type: "object",
      properties: {
        url: {
          type: "string",
          description: "URL to generate adapter for",
        },
        goal: {
          type: "string",
          description: "What you want (e.g., hot, search, trending)",
        },
        ai: {
          type: "boolean",
          description: "Use AI (LLM) to analyze and generate adapter (requires autocli.ai auth)",
          default: false,
        },
      },
      required: ["url"],
    },
  },
  {
    name: "ferrss_search_adapters",
    description:
      "Search for existing community adapters on autocli.ai marketplace by URL.",
    inputSchema: {
      type: "object",
      properties: {
        url: {
          type: "string",
          description: "URL to search adapters for",
        },
      },
      required: ["url"],
    },
  },
  {
    name: "ferrss_doctor",
    description:
      "Run Ferrss diagnostic checks. Verify that the CLI and its components are working correctly.",
    inputSchema: {
      type: "object",
      properties: {},
      required: [],
    },
  },
];

// Determine adapters directory
function adaptersDir(): string {
  const candidates = [
    join(dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "adapters"),
    join(dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..", "Ferrss", "adapters"),
  ];
  for (const d of candidates) {
    if (existsSync(d)) return d;
  }
  return "";
}

async function listAdapters(): Promise<string> {
  const ad = adaptersDir();
  if (!ad) {
    // Fall back to running autocli --help and parsing
    const { stdout } = await execAsync(`${getAutocliPath()} --help`);
    return stdout;
  }
  const { readdirSync, statSync } = await import("fs");
  const dirs = readdirSync(ad).filter((f: string) =>
    statSync(join(ad, f)).isDirectory()
  );
  return dirs.sort().join("\n");
}

async function runCommand(
  site: string,
  command: string,
  args: string,
  format: string
): Promise<string> {
  const fullArgs = [site, command];
  if (args) {
    fullArgs.push(...args.split(/\s+/).filter(Boolean));
  }
  if (format !== "table") {
    fullArgs.push("--format", format);
  }

  const { stdout, stderr } = await execAsync(
    `${getAutocliPath()} ${fullArgs.map((a) => `"${a.replace(/"/g, '\\"')}"`).join(" ")}`,
    { timeout: 60000 }
  );
  if (stderr && !stdout) throw new Error(stderr);
  return stdout || stderr;
}

// Server setup
const server = new Server(
  { name: "ferrss-mcp", version: "1.0.0" },
  { capabilities: { tools: {} } }
);

server.setRequestHandler(ListToolsRequestSchema, async () => ({
  tools: TOOLS,
}));

server.setRequestHandler(CallToolRequestSchema, async (request) => {
  const { name, arguments: args } = request.params;

  try {
    switch (name) {
      case "ferrss_list_adapters": {
        const adapters = await listAdapters();
        return {
          content: [
            {
              type: "text" as const,
              text: `Available Ferrss adapters:\n\n${adapters}`,
            },
          ],
        };
      }

      case "ferrss_run": {
        const { site, command, args: extraArgs, format = "json" } = args as {
          site: string;
          command: string;
          args?: string;
          format?: string;
        };
        const result = await runCommand(site, command, extraArgs ?? "", format);
        return {
          content: [{ type: "text" as const, text: result }],
        };
      }

      case "ferrss_read": {
        const { url, format = "markdown" } = args as {
          url: string;
          format?: string;
        };
        const { stdout, stderr } = await execAsync(
          `${getAutocliPath()} read "${url.replace(/"/g, '\\"')}" --format ${format}`,
          { timeout: 60000 }
        );
        if (stderr && !stdout) throw new Error(stderr);
        return {
          content: [{ type: "text" as const, text: stdout || stderr }],
        };
      }

      case "ferrss_explore": {
        const { url, goal } = args as { url: string; goal?: string };
        let cmd = `${getAutocliPath()} explore "${url.replace(/"/g, '\\"')}"`;
        if (goal) cmd += ` --goal "${goal.replace(/"/g, '\\"')}"`;
        const { stdout, stderr } = await execAsync(cmd, { timeout: 120000 });
        if (stderr && !stdout) throw new Error(stderr);
        return {
          content: [{ type: "text" as const, text: stdout || stderr }],
        };
      }

      case "ferrss_generate": {
        const { url, goal, ai } = args as {
          url: string;
          goal?: string;
          ai?: boolean;
        };
        let cmd = `${getAutocliPath()} generate "${url.replace(/"/g, '\\"')}"`;
        if (goal) cmd += ` --goal "${goal.replace(/"/g, '\\"')}"`;
        if (ai) cmd += " --ai";
        const { stdout, stderr } = await execAsync(cmd, { timeout: 180000 });
        if (stderr && !stdout) throw new Error(stderr);
        return {
          content: [{ type: "text" as const, text: stdout || stderr }],
        };
      }

      case "ferrss_search_adapters": {
        const { url } = args as { url: string };
        const { stdout, stderr } = await execAsync(
          `${getAutocliPath()} search "${url.replace(/"/g, '\\"')}"`,
          { timeout: 30000 }
        );
        if (stderr && !stdout) throw new Error(stderr);
        return {
          content: [{ type: "text" as const, text: stdout || stderr }],
        };
      }

      case "ferrss_doctor": {
        const { stdout, stderr } = await execAsync(`${getAutocliPath()} doctor`, {
          timeout: 30000,
        });
        if (stderr && !stdout) throw new Error(stderr);
        return {
          content: [{ type: "text" as const, text: stdout || stderr }],
        };
      }

      default:
        throw new Error(`Unknown tool: ${name}`);
    }
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return {
      content: [{ type: "text" as const, text: `Error: ${message}` }],
      isError: true,
    };
  }
});

// Start
async function main() {
  // Validate autocli is available
  getAutocliPath();
  const transport = new StdioServerTransport();
  await server.connect(transport);
  console.error("Ferrss MCP server running on stdio");
}

main().catch((err) => {
  console.error("Fatal error:", err);
  process.exit(1);
});
