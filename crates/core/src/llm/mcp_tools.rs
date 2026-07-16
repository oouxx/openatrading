// Re-export all MCP infrastructure from the agent crate.
// build_tools lives in super::tools because it references ToolRegistry.
pub use openatrading_agent::mcp_tools::{
    McpClient, McpError, McpRegistry, McpServerConfig, McpToolSpec, McpTransportConfig, ToolConfig,
};
