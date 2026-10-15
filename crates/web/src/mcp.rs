//! MCP 服务器管理接口（配置读写）。
//!
//! 只是一层薄封装：真正的读取/校验/写入都在
//! [`openatrading_core::llm::McpConfigStore`]，协议与连接由 pi-rs SDK 负责。
//! 写入一律落在全局 `{agent_dir}/mcp.json`；定义在项目级文件里的条目只读
//! （项目级按名覆盖全局，写全局不会生效）。

use axum::response::IntoResponse;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use std::sync::Arc;

use openatrading_core::llm::{McpConfigError, McpServerPayload, McpServerView};

use crate::AppState;

/// `McpConfigError` → HTTP 状态码。
///
/// 状态码与文案要和前端 `mcpErrorMessage` 的映射保持一致（浏览器路径只拿得到
/// 状态码，Tauri 路径拿得到错误原文）。
fn status_for(err: &McpConfigError) -> StatusCode {
    match err {
        McpConfigError::Exists => StatusCode::CONFLICT,
        McpConfigError::NotFound => StatusCode::NOT_FOUND,
        McpConfigError::ProjectScoped(_) => StatusCode::FORBIDDEN,
        McpConfigError::Invalid(_) => StatusCode::BAD_REQUEST,
        McpConfigError::Config(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// GET /api/mcp/servers — list all MCP servers
pub async fn list_handler(State(state): State<Arc<AppState>>) -> Json<Vec<McpServerView>> {
    Json(list_mcp_servers_data(&state))
}

/// Data-layer variant shared by the HTTP handler and the Tauri IPC dispatch.
pub fn list_mcp_servers_data(state: &Arc<AppState>) -> Vec<McpServerView> {
    let (servers, errors) = state.mcp.servers();
    for error in &errors {
        tracing::warn!("[mcp] 配置解析失败: {error}");
    }
    servers
}

/// POST /api/mcp/servers — add a new MCP server
///
/// # Errors
/// `409 Conflict` if the name is taken, `400` if the payload is invalid,
/// `500` if the config file cannot be written.
pub async fn add_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<McpServerPayload>,
) -> Result<impl IntoResponse, StatusCode> {
    add_mcp_server_data(&state, payload).map_err(|e| status_for(&e))?;
    Ok(StatusCode::CREATED)
}

/// Data-layer variant shared by the HTTP handler and the Tauri IPC dispatch.
///
/// # Errors
/// See [`McpConfigStore::create`](openatrading_core::llm::McpConfigStore::create).
pub fn add_mcp_server_data(
    state: &Arc<AppState>,
    payload: McpServerPayload,
) -> Result<(), McpConfigError> {
    state.mcp.create(&payload)
}

/// DELETE /api/mcp/servers/:name — delete an MCP server
///
/// # Errors
/// `404 Not Found` if no server with the given name exists, `403` if it is
/// defined in the project config file.
pub async fn delete_handler(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    delete_mcp_server_data(&state, &name).map_err(|e| status_for(&e))?;
    Ok(StatusCode::NO_CONTENT)
}

/// Data-layer variant shared by the HTTP handler and the Tauri IPC dispatch.
///
/// # Errors
/// See [`McpConfigStore::delete`](openatrading_core::llm::McpConfigStore::delete).
pub fn delete_mcp_server_data(state: &Arc<AppState>, name: &str) -> Result<(), McpConfigError> {
    state.mcp.delete(name)
}

/// PUT /api/mcp/servers/:name — update an existing MCP server (edit flow)
///
/// # Errors
/// `404 Not Found` if the name is unknown, `409` if the new name is taken,
/// `403` if the entry is defined in the project config file.
pub async fn update_handler(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Json(payload): Json<McpServerPayload>,
) -> Result<impl IntoResponse, StatusCode> {
    update_mcp_server_data(&state, &name, payload).map_err(|e| status_for(&e))?;
    Ok(StatusCode::OK)
}

/// Data-layer variant shared by the HTTP handler and the Tauri IPC dispatch.
///
/// # Errors
/// See [`McpConfigStore::update`](openatrading_core::llm::McpConfigStore::update).
pub fn update_mcp_server_data(
    state: &Arc<AppState>,
    name: &str,
    payload: McpServerPayload,
) -> Result<(), McpConfigError> {
    state.mcp.update(name, &payload)
}

/// GET /api/mcp/servers/:name/toggle — toggle an MCP server's enabled state
///
/// # Errors
/// `404 Not Found` if the name is unknown, `403` if it is defined in the
/// project config file.
pub async fn toggle_handler(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    toggle_mcp_server_data(&state, &name).map_err(|e| status_for(&e))?;
    Ok(StatusCode::NO_CONTENT)
}

/// Data-layer variant shared by the HTTP handler and the Tauri IPC dispatch.
///
/// # Errors
/// See [`McpConfigStore::toggle`](openatrading_core::llm::McpConfigStore::toggle).
pub fn toggle_mcp_server_data(state: &Arc<AppState>, name: &str) -> Result<(), McpConfigError> {
    state.mcp.toggle(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 状态码是前端错误文案的契约（浏览器路径只拿得到状态码）：每个变体必须映射到
    /// 不同的码，否则会把「项目级只读」显示成「已存在同名服务器」。
    #[test]
    fn every_error_variant_maps_to_its_own_status() {
        let cases = [
            (McpConfigError::Exists, StatusCode::CONFLICT),
            (McpConfigError::NotFound, StatusCode::NOT_FOUND),
            (
                McpConfigError::ProjectScoped("/p/mcp.json".into()),
                StatusCode::FORBIDDEN,
            ),
            (
                McpConfigError::Invalid("bad".into()),
                StatusCode::BAD_REQUEST,
            ),
            (
                McpConfigError::Config("io".into()),
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
        ];
        let mut seen = std::collections::HashSet::new();
        for (err, expected) in cases {
            let actual = status_for(&err);
            assert_eq!(actual, expected, "{err:?}");
            assert!(seen.insert(actual), "状态码重复: {actual} ({err:?})");
        }
    }
}
