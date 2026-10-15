//! LLM / agent glue re-exported for the openatrading layers.
//!
//! The pure pi-coding-agent session lifecycle (pool, factory, session-file
//! lookup) lives in [`openatrading_agent::session`]; this module re-exports it
//! alongside the MCP infrastructure ([`mcp`]: pi-rs 的客户端能力 + 本项目的
//! 服务端) and the domain-specific tool registry.
//!
//! Domain tools that couple to core's types (`TradeManager`, `MemoryStore`)
//! stay in [`tools`] — they cannot move to the agent crate without creating a
//! circular `agent → core` dependency.

pub mod mcp;
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

// ── MCP infrastructure ─────────────────────────────────────────────────────
// 客户端复用 pi-rs SDK（见 [`mcp`]）；服务端（对外暴露本项目工具）仍是自研实现，
// 从 extension crate 再导出，保持 `core::llm` 公开名稳定。
pub use openatrading_extension::mcp::{serve_http, serve_stdio, McpServerHandler};

/// MCP 客户端：pi-rs 的配置/连接原语 + 管理界面用的读写投影。
pub use self::mcp::{
    add_server, connect_specs, global_config_path, load_servers, load_specs, project_config_path,
    remove_server, set_enabled, LoadedServer, McpConfigError, McpConfigStore, McpConnection,
    McpScope, McpServerPayload, McpServerSpec, McpServerView, McpTransportConfig, McpTransportSpec,
};

// ── Extension registry builder ─────────────────────────────────────────────
pub use openatrading_extension::{create_registry, RegistryOptions};

// ── pi-rs types ────────────────────────────────────────────────────────────
pub use openatrading_agent::pi_ai_types;
pub use openatrading_agent::types::{AgentMessage, DynTool};

// ── Domain tool registry (stays in core) ──────────────────────────────────
pub use self::tools::ToolRegistry;
