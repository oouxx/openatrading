//! openatrading-agent — pi-coding-agent glue used in-process (NOT via RPC).
//!
//! Houses the session lifecycle ([`session`]), MCP infrastructure
//! ([`mcp_tools`], [`mcp_server`]), and the low-level tool helpers ([`tools`]).
//! Re-exports the pi-rs types the rest of the workspace consumes.

pub mod mcp_server;
pub mod mcp_tools;
pub mod session;
pub mod tools;

pub use pi_coding_agent::pi_agent_core::pi_ai_types;
pub use pi_coding_agent::pi_agent_core::types;
