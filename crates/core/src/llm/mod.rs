// Re-exports from openatrading-agent (the canonical agent crate).
// Core's business-specific agent code lives in the sibling modules below.
pub mod agent_runtime;
pub mod decision;
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
pub use openatrading_agent::session::invoke_agent;
pub use openatrading_agent::tools::{
    empty_schema, str_arg, str_schema, text_result, to_box_err, tool, ExecuteFn,
};

pub use openatrading_agent::agent::{Agent, AgentOptions};
pub use openatrading_agent::pi_ai_types;
pub use openatrading_agent::types::{
    AfterToolCallContext, AfterToolCallResult, AgentContext, AgentEvent, AgentMessage, AgentState,
    AgentTool, AgentToolCall, AgentToolResult, BeforeToolCallContext, BeforeToolCallResult,
    ConvertToLlmFn, DynTool, QueueMode, ShouldStopAfterTurnContext,
};

// AgentRuntime — manages agent session lifecycle
pub use self::agent_runtime::{AgentResult, AgentRuntime};

// Decision — structured output extraction on top of AgentRuntime
pub use self::decision::run_decision;

// Business-specific tool registry (stays in core, references market tools)
pub use self::tools::ToolRegistry;

// build_tools is kept in core because it references ToolRegistry
pub use self::tools::build_tools;
