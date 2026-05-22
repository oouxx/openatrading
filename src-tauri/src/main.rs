#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use trending_agent_core::api::Client as ApiClient;
use trending_agent_core::gateway::Gateway;
use trending_agent_core::llm::openrouter::LlmClient;
use trending_agent_core::roles::BUILTIN_STRATEGIES;
use trending_agent_core::web::{self, AppState, RoleEntry};

fn find_project_root() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/"));

    if home.join(".atrading").exists() {
        return home;
    }

    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn build_roles() -> Vec<RoleEntry> {
    BUILTIN_STRATEGIES
        .iter()
        .map(|(name, display_name, strategy)| RoleEntry {
            name: name.to_string(),
            display_name: display_name.to_string(),
            strategy: strategy.to_string(),
        })
        .collect()
}

#[tauri::command]
async fn api_fetch(
    path: String,
    _state: tauri::State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    let url = format!("http://127.0.0.1:8080{}", path);
    let resp = reqwest::Client::new()
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;
    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("parse json failed: {e}"))?;
    Ok(json)
}

#[tauri::command]
async fn api_post(
    path: String,
    body: serde_json::Value,
    _state: tauri::State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    let url = format!("http://127.0.0.1:8080{}", path);
    let resp = reqwest::Client::new()
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;
    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("parse json failed: {e}"))?;
    Ok(json)
}

#[tauri::command]
async fn api_chat_stream(
    body: serde_json::Value,
    app: tauri::AppHandle,
    _state: tauri::State<'_, Arc<AppState>>,
) -> Result<String, String> {
    use tauri::Emitter;

    let url = "http://127.0.0.1:8080/api/ai/chat";
    let resp = reqwest::Client::new()
        .post(url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("server returned {}", resp.status()));
    }

    let event_name = format!(
        "chat-stream-{}",
        body.get("role")
            .and_then(|v| v.as_str())
            .unwrap_or("default")
    );

    let mut stream = resp.bytes_stream();
    use futures_util::StreamExt;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("stream error: {e}"))?;
        let text = String::from_utf8_lossy(&chunk);
        let _ = app.emit(&event_name, text.to_string());
    }

    let _ = app.emit(&event_name, "[DONE]".to_string());
    Ok(event_name)
}

fn main() {
    let project_root = find_project_root();
    let db_path = project_root.join(".atrading").join("data.db");

    let gateway = Gateway::init(&db_path).expect("failed to init gateway");
    let memory = gateway.memory.clone();
    let trade = gateway.trade.clone();

    let roles = build_roles();
    let strategies_dir = project_root.join(".atrading").join("strategies");

    let app_state = Arc::new(AppState {
        llm: LlmClient::from_env(),
        api: ApiClient::new(),
        memory,
        trade,
        roles: Arc::new(RwLock::new(roles)),
        strategies_dir,
    });

    let server_state = app_state.clone();
    let router = web::build_router(server_state);

    tauri::Builder::default()
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            api_fetch,
            api_post,
            api_chat_stream
        ])
        .setup(move |_app| {
            let port = 8080u16;

            tauri::async_runtime::spawn(async move {
                let addr = format!("0.0.0.0:{port}");
                match tokio::net::TcpListener::bind(&addr).await {
                    Ok(listener) => {
                        println!("🌐 Web API: http://0.0.0.0:{port}");
                        if let Err(e) = axum::serve(listener, router).await {
                            eprintln!("[tauri] web server error: {e}");
                        }
                    }
                    Err(e) => {
                        eprintln!("[tauri] failed to bind port {port}: {e}");
                    }
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
