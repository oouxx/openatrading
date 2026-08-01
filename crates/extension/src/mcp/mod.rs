//! MCP 协议支持（客户端 + 服务端），基于 pi-extension-api 的 `HookHandler`。
//!
//! - [`client`]：[`McpClient`] 封装 rmcp 客户端传输，[`McpExtension`] 把外部 MCP
//!   server 的工具注册为 pi-rs 扩展工具。
//! - [`server`]：[`server::McpServerHandler`] 把 pi-rs 扩展注册表里的工具对外
//!   暴露为标准 MCP server，派发经 `HookRunner::dispatch_tool_call`。
//!
//! 两端均不依赖 `pi-coding-agent`/`DynTool`，仅在 extension 层完成 rmcp↔pi-rs 桥接。

pub mod client;
pub mod server;

pub use client::{
    discover_mcp, DiscoveredServer, McpClient, McpError, McpExtension, McpServerConfig,
    McpTransportConfig, RemoteToolSpec,
};
pub use server::{serve_http, serve_stdio, McpServerHandler};
