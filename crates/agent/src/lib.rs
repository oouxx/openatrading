//! openatrading-agent — pi-coding-agent glue used in-process (NOT via RPC).
//!
//! Houses the session lifecycle ([`session`]), the event protocol ([`events`])
//! and the low-level tool helpers ([`tools`]). MCP 能力不在此实现：客户端由 pi-rs
//! SDK 在会话装配时自动接入（读 `mcp.json`），服务端在
//! `openatrading-extension::mcp`。
//! Re-exports the pi-rs types the rest of the workspace consumes.

pub mod events;
pub mod session;
pub mod tools;

/// 项目级 Agent Skills 的标准目录，由 pi-rs 资源加载器解析。
pub const PROJECT_SKILLS_DIR: &str = ".agents/skills";

pub use pi_coding_agent::pi_agent_core::pi_ai_types;
pub use pi_coding_agent::pi_agent_core::types;
