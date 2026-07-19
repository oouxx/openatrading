//! Legacy one-shot agent invocation.
//!
//! `invoke_agent` was removed as part of the `AgentRuntime` unification
//! (see `docs/agentruntime.md`). All LLM calls now go through
//! `openatrading_core::llm::AgentRuntime` (create_session → subscribe → send).
//!
//! This module is kept as a marker so the `pub mod session` re-export in
//! `lib.rs` stays valid during the migration; it contains no public API.
