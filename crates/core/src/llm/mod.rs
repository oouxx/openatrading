pub mod mcp_server;
pub mod mcp_tools;
pub mod session;
pub mod tools;

pub use openatrading_provider::llm::{
    ChatStreamProvider, LlmClient, StreamEvent, StreamFn, StreamFnOptions, StreamResult, ToolCall,
    ToolCallFunction, ToolCallItem, ToolCallMessage, ToolChatChoice, ToolChatResponse,
    ToolStreamResult,
};

pub use mcp_server::{serve_http, serve_stdio, MarketToolsServer};
pub use mcp_tools::{
    build_tools, McpClient, McpRegistry, McpServerConfig, McpTransportConfig, ToolConfig,
};
pub use tools::ToolRegistry;

pub use openatrading_agent::agent::{Agent, AgentOptions};
pub use openatrading_agent::pi_ai_types;
pub use openatrading_agent::types::{
    AfterToolCallContext, AfterToolCallResult, AgentContext, AgentEvent, AgentMessage, AgentState,
    AgentTool, AgentToolCall, AgentToolResult, BeforeToolCallContext, BeforeToolCallResult,
    ConvertToLlmFn, DynTool, QueueMode, ShouldStopAfterTurnContext,
};
