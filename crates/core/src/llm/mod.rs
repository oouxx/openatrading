// Re-exports from openatrading-agent (the canonical agent crate).
// Core's business-specific agent code lives in the sibling modules below.
pub mod agent_runtime;
pub mod runtime;
pub mod session;
pub mod tools;

pub use openatrading_agent::llm::{
    assistant_event_to_stream_events, LlmClient, StreamEvent, StreamFn, StreamFnOptions,
    StreamResult, ToolCall, ToolCallFunction, ToolCallItem, ToolCallMessage, ToolChatChoice,
    ToolChatResponse, ToolStreamResult,
};

pub use openatrading_agent::mcp_server::{serve_http, serve_stdio, MarketToolsServer};
pub use openatrading_agent::mcp_tools::{
    McpClient, McpRegistry, McpServerConfig, McpTransportConfig, ToolConfig,
};
pub use openatrading_agent::tools::{empty_schema, text_result, to_box_err, tool, ExecuteFn};

pub use openatrading_agent::agent::{Agent, AgentOptions};
pub use openatrading_agent::pi_ai_types;
pub use openatrading_agent::types::{
    AfterToolCallContext, AfterToolCallResult, AgentContext, AgentEvent, AgentMessage, AgentState,
    AgentTool, AgentToolCall, AgentToolResult, BeforeToolCallContext, BeforeToolCallResult,
    ConvertToLlmFn, DynTool, QueueMode, ShouldStopAfterTurnContext,
};

// Unified runtime — the single entry point for all LLM operations
pub use self::runtime::{AgentRuntime, SessionHandle, SessionResult};

// Legacy session creation (for backward compat during migration)
pub use self::agent_runtime::create_session;

// Business-specific tool registry (stays in core, references market tools)
pub use self::tools::ToolRegistry;

// build_tools is kept in core because it references ToolRegistry
pub use self::tools::build_tools;

// Extension → DynTool bridge
pub use self::tools::extension_tools_as_dyn;

// Per-role ToolDefinition helpers (for custom_tools in AgentRuntime)
pub use self::tools::{create_historical_context_tool_def, create_positions_tool_def};
