//! Agent session lifecycle: actor, pool, factory, and session-file lookup.
//!
//! This module groups the pi-coding-agent session glue that is independent of
//! the openatrading domain (no `TradeManager` / `MemoryStore` coupling):
//!
//! - [`actor`] — the per-session actor task (owns the session, turn queue,
//!   event fan-out; mirrors pi-rs' ACP `SessionTask` design).
//! - [`factory`] — builds [`factory::CreatedSession`] instances via a reusable
//!   closure (owned `Arc<AgentSession>` + keep-alive services).
//! - [`pool`] — supervisor: LRU map of session handles, resume-from-disk,
//!   per-session event state.
//! - [`slash`] — session-level slash commands executed inside the actor.
//! - [`runtime`] — locates session JSONL files on disk by ID.
//!
//! Domain-specific tool factories (positions, historical context) live in
//! `openatrading_core::llm::tools`, since they depend on core's domain types.

pub mod actor;
pub mod factory;
pub mod pool;
pub mod runtime;
pub mod slash;

pub use actor::{spawn_session_actor, SessionHandle, SessionMsg, SessionStatus};
pub use factory::{
    create_factory, CreateSessionFactory, CreateSessionParams, CreatedSession, FactoryOptions,
};
pub use pool::SessionPool;
pub use runtime::find_session_file_by_id;
pub use slash::{handle_slash_command, SESSION_SLASH_COMMANDS};
