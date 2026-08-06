use axum::response::IntoResponse;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use std::sync::Arc;

use openatrading_core::llm::{self, McpServerConfig};

use crate::AppState;

/// GET /api/mcp/servers — list all MCP servers
///
/// # Panics
/// Panics if the MCP server lock is poisoned.
pub async fn list_handler(State(state): State<Arc<AppState>>) -> Json<Vec<McpServerConfig>> {
    Json(list_mcp_servers_data(&state).await)
}

/// Data-layer variant shared by the HTTP handler and the Tauri IPC dispatch.
pub async fn list_mcp_servers_data(state: &Arc<AppState>) -> Vec<McpServerConfig> {
    let guard = state
        .mcp_servers
        .lock()
        .unwrap_or_else(|e| panic!("MCP server lock poisoned: {e}"));
    guard.clone()
}

/// POST /api/mcp/servers — add a new MCP server
///
/// # Errors
/// Returns `409 Conflict` if a server with the same name already exists.
///
/// # Panics
/// Panics if the MCP server lock is poisoned.
pub async fn add_handler(
    State(state): State<Arc<AppState>>,
    Json(config): Json<McpServerConfig>,
) -> Result<impl IntoResponse, StatusCode> {
    add_mcp_server_data(&state, config.clone())
        .await
        .map_err(|_| StatusCode::CONFLICT)?;
    Ok((StatusCode::CREATED, Json(config)))
}

/// Data-layer variant shared by the HTTP handler and the Tauri IPC dispatch.
pub async fn add_mcp_server_data(
    state: &Arc<AppState>,
    config: McpServerConfig,
) -> Result<(), String> {
    {
        let mut guard = state
            .mcp_servers
            .lock()
            .map_err(|_| "MCP server lock poisoned".to_string())?;
        if guard.iter().any(|s| s.name == config.name) {
            return Err("MCP server already exists".to_string());
        }
        guard.push(config);
    }
    load_servers_and_reload(state).await;
    Ok(())
}

/// DELETE /api/mcp/servers/:name — delete an MCP server
///
/// # Errors
/// Returns `404 Not Found` if no server with the given name exists.
///
/// # Panics
/// Panics if the MCP server lock is poisoned.
pub async fn delete_handler(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    delete_mcp_server_data(&state, &name)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    Ok((StatusCode::NO_CONTENT, ""))
}

/// Data-layer variant shared by the HTTP handler and the Tauri IPC dispatch.
pub async fn delete_mcp_server_data(state: &Arc<AppState>, name: &str) -> Result<(), String> {
    {
        let mut guard = state
            .mcp_servers
            .lock()
            .map_err(|_| "MCP server lock poisoned".to_string())?;
        let before = guard.len();
        guard.retain(|s| s.name != name);
        if guard.len() == before {
            return Err("MCP server not found".to_string());
        }
    }
    load_servers_and_reload(state).await;
    Ok(())
}

/// POST /api/mcp/servers/:name/toggle — toggle an MCP server's enabled state
///
/// # Errors
/// Returns `404 Not Found` if no server with the given name exists.
///
/// # Panics
/// Panics if the MCP server lock is poisoned.
pub async fn toggle_handler(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    toggle_mcp_server_data(&state, &name)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    Ok((StatusCode::NO_CONTENT, ""))
}

/// Data-layer variant shared by the HTTP handler and the Tauri IPC dispatch.
pub async fn toggle_mcp_server_data(state: &Arc<AppState>, name: &str) -> Result<(), String> {
    let found = {
        let mut guard = state
            .mcp_servers
            .lock()
            .map_err(|_| "MCP server lock poisoned".to_string())?;
        guard.iter_mut().find(|s| s.name == name).map(|s| {
            s.enabled = !s.enabled;
        })
    };
    found.ok_or_else(|| "MCP server not found".to_string())?;
    load_servers_and_reload(state).await;
    Ok(())
}

/// Update an existing MCP server (PUT — edit flow in the UI). Not exposed on
/// the HTTP router yet, used by the Tauri IPC dispatch.
pub async fn update_mcp_server_data(
    state: &Arc<AppState>,
    name: &str,
    config: McpServerConfig,
) -> Result<(), String> {
    {
        let mut guard = state
            .mcp_servers
            .lock()
            .map_err(|_| "MCP server lock poisoned".to_string())?;
        let slot = guard
            .iter_mut()
            .find(|s| s.name == name)
            .ok_or_else(|| "MCP server not found".to_string())?;
        *slot = config;
    }
    load_servers_and_reload(state).await;
    Ok(())
}

async fn load_servers_and_reload(state: &Arc<AppState>) {
    let servers = {
        let guard = state
            .mcp_servers
            .lock()
            .unwrap_or_else(|e| panic!("MCP server lock poisoned: {e}"));
        guard.clone()
    };
    reload_and_persist(state, &servers).await;
}

async fn reload_and_persist(state: &Arc<AppState>, servers: &[McpServerConfig]) {
    let discovered = llm::discover_mcp(servers).await;
    let mcp_names: Vec<String> = discovered
        .iter()
        .flat_map(|s| s.specs.iter().map(|t| t.name.clone()))
        .collect();
    let mut new_registry = llm::ToolRegistry::new();
    let ext_registry = openatrading_core::create_extension_registry(
        openatrading_core::llm::RegistryOptions::market_only(),
    );
    new_registry.set_extension_tools(&ext_registry);
    new_registry.set_mcp_names(mcp_names);

    state.ctx.tool_registry.store(Arc::new(new_registry));

    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    let path = std::path::PathBuf::from(home)
        .join(".atrading")
        .join(".mcp.json");
    if let Some(p) = path.parent() {
        let _ = std::fs::create_dir_all(p);
    }
    let map: std::collections::BTreeMap<_, _> = servers
        .iter()
        .map(|s| {
            (
                s.name.clone(),
                serde_json::to_value(s)
                    .unwrap_or_else(|e| panic!("MCP server config serialization failed: {e}")),
            )
        })
        .collect();
    let root = serde_json::json!({"mcpServers": map});
    if let Ok(c) = serde_json::to_string_pretty(&root) {
        let _ = std::fs::write(&path, &c);
    }
}
