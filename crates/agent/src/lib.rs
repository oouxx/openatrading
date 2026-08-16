//! openatrading-agent — pi-coding-agent glue used in-process (NOT via RPC).
//!
//! Houses the session lifecycle ([`session`]), the event protocol ([`events`])
//! and the low-level tool helpers ([`tools`]). MCP 协议（client/server）现由
//! `openatrading-extension::mcp` 基于 pi-rs 的 `HookHandler` 提供，本 crate
//! 不再持有 rmcp 拼装。
//! Re-exports the pi-rs types the rest of the workspace consumes.

pub mod events;
pub mod session;
pub mod tools;

pub use pi_coding_agent::pi_agent_core::pi_ai_types;
pub use pi_coding_agent::pi_agent_core::types;
