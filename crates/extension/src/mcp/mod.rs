//! MCP 服务端：把 pi-rs 扩展注册表里的工具对外暴露为标准 MCP server。
//!
//! [`server::McpServerHandler`] 派发经 `HookRunner::dispatch_tool_call`，
//! 由 `serve_http` / `serve_stdio` 起 stdio 或 streamable HTTP 传输。
//!
//! MCP **客户端**（连接外部 server）不再由本 crate 提供：pi-rs v1.87 起
//! `pi_coding_agent::core::{mcp, mcp_config}` 已提供完整客户端能力，本项目经
//! `openatrading_core::llm::McpToolSet` 复用（pi-rs 无服务端能力，故这一半留在本 crate）。

pub mod server;

pub use server::{serve_http, serve_stdio, McpServerHandler};
