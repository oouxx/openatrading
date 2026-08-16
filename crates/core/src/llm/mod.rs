//! LLM / agent glue re-exported for the openatrading layers.
//!
//! The pure pi-coding-agent session lifecycle (pool, factory, session-file
//! lookup) lives in [`openatrading_agent::session`]; this module re-exports it
//! alongside the MCP infrastructure (now provided by
//! `openatrading-extension::mcp`) and the domain-specific tool registry.
//!
//! Domain tools that couple to core's types (`TradeManager`, `MemoryStore`)
//! stay in [`tools`] — they cannot move to the agent crate without creating a
//! circular `agent → core` dependency.

pub mod tools;

// ── Session lifecycle (from the agent crate) ───────────────────────────────
pub use openatrading_agent::session::{
    create_factory, find_session_file_by_id, handle_slash_command, CreateSessionFactory,
    CreateSessionParams, FactoryOptions, SessionHandle, SessionPool, SessionStatus,
    SESSION_SLASH_COMMANDS,
};

// ── Event protocol (from the agent crate) ──────────────────────────────────
pub use openatrading_agent::events::{
    convert_agent_event, convert_session_event, EventEnvelope, ReplayGap, SessionEventPayload,
    SessionEventState, ToolResultPayload, TranscriptEvent, SCHEMA_VERSION,
};

// ── MCP infrastructure (from the extension crate) ──────────────────────────
// pi-rs v1.80.2 的 `mcp` feature 为占位符，MCP 协议由 openatrading-extension
// 基于 HookHandler 自实现并在此再导出，保持 `core::llm` 公开名稳定。
pub use openatrading_extension::mcp::{
    discover_mcp, serve_http, serve_stdio, DiscoveredServer, McpClient, McpError, McpExtension,
    McpServerConfig, McpServerHandler, McpTransportConfig, RemoteToolSpec,
};

// ── Extension registry builder ─────────────────────────────────────────────
pub use openatrading_extension::{create_registry, RegistryOptions};

// ── pi-rs types ────────────────────────────────────────────────────────────
pub use openatrading_agent::pi_ai_types;
pub use openatrading_agent::types::{AgentMessage, DynTool};

// ── Domain tool registry (stays in core) ──────────────────────────────────
pub use self::tools::ToolRegistry;
