export interface McpServerConfig {
  name: string
  transport: McpTransportConfig
  enabled: boolean
}

export interface McpTransportConfig {
  type: "streamable-http" | "stdio" | "sse"
  url?: string
  auth_token?: string
  post_url?: string
  command?: string
  args?: string[]
  env?: Record<string, string>
}
