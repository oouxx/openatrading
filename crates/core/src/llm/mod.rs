pub mod mcp_server;
pub mod mcp_tools;
pub mod session;
pub mod tools;

pub use openatrading_agent::types::{AgentTool, AgentToolResult, DynTool};
pub use openatrading_provider::llm::{
    ChatStreamProvider, LlmClient, StreamEvent, StreamResult, ToolCall, ToolCallFunction,
    ToolCallItem, ToolCallMessage, ToolChatChoice, ToolChatResponse, ToolStreamResult,
};

pub use mcp_server::{build_market_tools, serve_http, serve_stdio, MarketToolsServer};
pub use mcp_tools::{
    build_tools, McpClient, McpRegistry, McpServerConfig, McpTransportConfig, ToolConfig,
};
pub use openatrading_agent::agent::{Agent, AgentOptions};
pub use openatrading_agent::pi_ai_types;
pub use openatrading_agent::proxy;
pub use openatrading_agent::runner::{agent_loop, create_agent, AgentResult};
pub use openatrading_agent::types::{
    AfterToolCallContext, AfterToolCallResult, AgentContext, AgentEvent, AgentMessage, AgentState,
    AgentToolCall, BeforeToolCallContext, BeforeToolCallResult, ConvertToLlmFn, QueueMode,
    ShouldStopAfterTurnContext, StreamFn, StreamFnOptions,
};
