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

/** 列表项：配置内容 + 它来自哪个作用域/文件 */
export interface McpServerConfig {
  name: string
  enabled: boolean
  /** 条目定义在哪个文件：全局（agent 目录）或项目（{cwd}/.pi-rs） */
  scope: "global" | "project"
  /** 定义该条目的配置文件路径 */
  source: string
  transport: McpTransportConfig
}

/** 写入载荷：新增/编辑都写全局 `$PI_RS_HOME/agent/mcp.json`，故不带 scope/source */
export interface McpServerPayload {
  name: string
  enabled: boolean
  transport: McpTransportConfig
}

/** 该条目是否可在本界面编辑（项目级只读：项目级按名覆盖全局，写全局不会生效） */
export function isReadOnly(server: McpServerConfig): boolean {
  return server.scope === "project"
}
