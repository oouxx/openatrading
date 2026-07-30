//! Agent session lifecycle: pool, factory, and session-file lookup.
//!
//! This module groups the pi-coding-agent session glue that is independent of
//! the openatrading domain (no `TradeManager` / `MemoryStore` coupling):
//!
//! - [`factory`] builds `AgentSessionRuntime` instances via a reusable closure.
//! - [`pool`] manages many concurrent sessions with LRU eviction.
//! - [`runtime`] locates session JSONL files on disk by ID.
//!
//! Domain-specific tool factories (positions, historical context) live in
//! `openatrading_core::llm::tools`, since they depend on core's domain types.

pub mod factory;
pub mod pool;
pub mod runtime;

pub use factory::create_factory;
pub use pool::SessionPool;
pub use runtime::find_session_file_by_id;
