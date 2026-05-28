pub mod agent;
pub mod tools;

pub use trending_agent_provider::llm::{
    LlmClient, StreamEvent, StreamResult, ToolCall, ToolCallFunction, ToolCallItem,
    ToolCallMessage, ToolChatChoice, ToolChatResponse, ToolStreamResult,
};
pub use trending_agent_provider::tool::{ExtractParam, SchemaType, Tool, ToolError, ToolSet};
pub use trending_agent_provider::define_tool;