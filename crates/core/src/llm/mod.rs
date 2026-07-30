//! LLM / agent glue re-exported for the openatrading layers.
//!
//! The pure pi-coding-agent session lifecycle (pool, factory, session-file
//! lookup) lives in [`openatrading_agent::session`]; this module re-exports it
//! alongside the MCP infrastructure and the domain-specific tool registry.
//!
//! Domain tools that couple to core's types (`TradeManager`, `MemoryStore`)
//! stay in [`tools`] — they cannot move to the agent crate without creating a
//! circular `agent → core` dependency.

pub mod tools;

// ── Session lifecycle (from the agent crate) ───────────────────────────────
pub use openatrading_agent::session::{create_factory, find_session_file_by_id, SessionPool};

// ── MCP infrastructure (from the agent crate) ─────────────────────────────
pub use openatrading_agent::mcp_server::{serve_http, serve_stdio};
pub use openatrading_agent::mcp_tools::{
    McpClient, McpRegistry, McpServerConfig, McpTransportConfig, ToolConfig,
};

// ── pi-rs types ────────────────────────────────────────────────────────────
pub use openatrading_agent::pi_ai_types;
pub use openatrading_agent::types::{AgentMessage, DynTool};

// ── Domain tool registry (stays in core) ──────────────────────────────────
pub use self::tools::{build_tools, ToolRegistry};
