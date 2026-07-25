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
    let guard = state.mcp_servers.lock().expect("MCP server lock poisoned");
    Json(guard.clone())
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
    {
        let mut guard = state.mcp_servers.lock().expect("MCP server lock poisoned");
        if guard.iter().any(|s| s.name == config.name) {
            return Err(StatusCode::CONFLICT);
        }
        guard.push(config.clone());
    }
    load_servers_and_reload(&state).await;
    Ok((StatusCode::CREATED, Json(config)))
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
    {
        let mut guard = state.mcp_servers.lock().expect("MCP server lock poisoned");
        let before = guard.len();
        guard.retain(|s| s.name != name);
        if guard.len() == before {
            return Err(StatusCode::NOT_FOUND);
        }
    }
    load_servers_and_reload(&state).await;
    Ok((StatusCode::NO_CONTENT, ""))
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
    let found = {
        let mut guard = state.mcp_servers.lock().expect("MCP server lock poisoned");
        guard.iter_mut().find(|s| s.name == name).map(|s| {
            s.enabled = !s.enabled;
        })
    };
    found.ok_or(StatusCode::NOT_FOUND)?;
    load_servers_and_reload(&state).await;
    Ok((StatusCode::NO_CONTENT, ""))
}

async fn load_servers_and_reload(state: &Arc<AppState>) {
    let servers = {
        let guard = state.mcp_servers.lock().expect("MCP server lock poisoned");
        guard.clone()
    };
    reload_and_persist(state, &servers).await;
}

async fn reload_and_persist(state: &Arc<AppState>, servers: &[McpServerConfig]) {
    let cfg = llm::ToolConfig {
        mcp_servers: servers.to_vec(),
    };
    let mut new_registry = llm::build_tools(&cfg).await;
    let ext_registry = openatrading_core::create_extension_registry();
    new_registry.set_extension_tools(&ext_registry);

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
                serde_json::to_value(s).expect("MCP server config serialization failed"),
            )
        })
        .collect();
    let root = serde_json::json!({"mcpServers": map});
    if let Ok(c) = serde_json::to_string_pretty(&root) {
        let _ = std::fs::write(&path, &c);
    }
}
