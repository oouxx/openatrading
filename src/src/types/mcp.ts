/**
 * 外部 MCP 服务器配置（pi-rs 的 `mcp.json` 形状）。
 *
 * 配置来源与协议实现都在 pi-rs SDK：全局 `$PI_RS_HOME/agent/mcp.json`，
 * 项目级 `{cwd}/.pi-rs/mcp.json`（同名覆盖全局）。会话侧由 pi-rs 在装配时读取，
 * 因此新增/修改对**新建**的会话立即生效。
 */

export type McpTransportType = "stdio" | "http" | "sse"

/** stdio：以子进程方式启动的本地 server */
export interface McpStdioTransport {
  type: "stdio"
  command: string
  args: string[]
  env: Record<string, string>
  /** 子进程工作目录，留空则用会话 cwd */
  cwd?: string
}

/** streamable HTTP：远端 server */
export interface McpHttpTransport {
  type: "http"
  url: string
  /** 鉴权等自定义头，值支持 `${VAR}` 环境变量引用 */
  headers: Record<string, string>
}

/** 旧版 HTTP+SSE 传输（协议版本 2024-11-05） */
export interface McpSseTransport {
  type: "sse"
  url: string
  headers: Record<string, string>
}

export type McpTransportConfig = McpStdioTransport | McpHttpTransport | McpSseTransport

/**
 * 工具暴露方式（pi-rs `exposure`）。
 *
 * - `direct`：直接出现在模型工具列表里（本应用新建条目的默认值）
 * - `deferred` / `codemode`：由 `tool_search` 按需加载
 * - `hidden`：注册但不可达
 *
 * `codemode` 在 pi-rs 里与 `deferred` 等价（本应用没有 codemode 脚本）。
 */
export type McpExposure = "direct" | "deferred" | "codemode" | "hidden"

/** 列表项：配置内容 + 它来自哪个作用域/文件 */
export interface McpServerConfig {
  name: string
  enabled: boolean
  /** 条目生效于哪个作用域：全局（agent 目录）或项目（{cwd}/.pi-rs） */
  scope: "global" | "project"
  /** 决定该条目生效状态的配置文件路径（项目 override 时为项目文件） */
  source: string
  transport: McpTransportConfig
  /** 生效的工具暴露方式 */
  exposure: McpExposure
  /** 配置里是否显式写了 exposure；false 表示走 pi-rs 默认值（按需加载） */
  exposureExplicit: boolean
}

/** 列表响应：`errors` 是 pi-rs 跳过/解析失败的配置项 */
export interface McpServerListResponse {
  servers: McpServerConfig[]
  errors: string[]
}

/** 一次性连接测试结果（连接失败也是正常响应，`ok: false`） */
export interface McpServerTestResult {
  ok: boolean
  toolCount: number
  error: string | null
  durationMs: number
}

/** 写入载荷：新增/编辑都写全局 `$PI_RS_HOME/agent/mcp.json`，故不带 scope/source */
export interface McpServerPayload {
  name: string
  enabled: boolean
  transport: McpTransportConfig
  /** 新增时缺省为 direct；编辑时缺省保留原值 */
  exposure?: McpExposure
}

/** 该条目是否可在本界面编辑（项目级只读：项目级按名覆盖全局，写全局不会生效） */
export function isReadOnly(server: McpServerConfig): boolean {
  return server.scope === "project"
}

/** 切换传输类型时生成该类型的空配置（判别联合，整体替换） */
export function emptyTransport(type: McpTransportType): McpTransportConfig {
  switch (type) {
    case "stdio":
      return { type: "stdio", command: "", args: [], env: {} }
    case "sse":
      return { type: "sse", url: "", headers: {} }
    default:
      return { type: "http", url: "", headers: {} }
  }
}

/** 传输方式的展示标签 */
export function mcpTransportLabel(transport: McpTransportConfig): string {
  switch (transport.type) {
    case "stdio":
      return "STDIO"
    case "sse":
      return "SSE"
    default:
      return "HTTP"
  }
}

/** 列表/搜索用的目标摘要：stdio 为命令 + 参数，HTTP/SSE 为 URL */
export function mcpTransportTarget(transport: McpTransportConfig): string {
  if (transport.type === "stdio") {
    return [transport.command, ...transport.args].filter(Boolean).join(" ")
  }
  return transport.url
}
