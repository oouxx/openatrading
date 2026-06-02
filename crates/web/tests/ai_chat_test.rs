//! Integration test for the `/api/ai/chat` SSE endpoint.
//!
//! Builds a real `AppState` (with SQLite, API client, LLM client) and sends
//! requests through the axum Router.
//!
//! ```sh
//! export DEEPSEEK_API_KEY=sk-...
//! cargo test -p openatrading-web --test ai_chat_test -- --nocapture
//! ```

use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Body;
use axum::http::Request;
use http_body_util::BodyExt;
use tokio::sync::RwLock;
use tower::Service;

use openatrading_core::api::Client as ApiClient;
use openatrading_core::llm::LlmClient;
use openatrading_core::memory::sqlite::SqliteMemoryStore;
use openatrading_core::trade::sqlite::SqliteTradeManager;
use openatrading_web::cache;
use openatrading_web::{build_router, AppState, RoleEntry};

fn api_key_available() -> bool {
    std::env::var("LLM_API_KEY").is_ok()
        || std::env::var("DEEPSEEK_API_KEY").is_ok()
        || std::env::var("OPENROUTER_API_KEY").is_ok()
}

fn setup_db() -> (Arc<SqliteMemoryStore>, Arc<SqliteTradeManager>) {
    let path = std::env::temp_dir().join("openatrading_ai_chat_test.db");
    let _ = std::fs::remove_file(&path);
    let memory = Arc::new(SqliteMemoryStore::open(&path).unwrap());
    let trade = Arc::new(SqliteTradeManager::open(&path).unwrap());
    (memory, trade)
}

async fn run_chat(message: &str) -> Vec<String> {
    let (memory, trade) = setup_db();

    let state = Arc::new(AppState {
        llm: LlmClient::with_config("openrouter".to_string(), "openrouter/free".to_string()),
        api: ApiClient::new(),
        memory,
        trade,
        roles: Arc::new(RwLock::new(Vec::<RoleEntry>::new())),
        strategies_dir: PathBuf::from("/tmp"),
        cache: cache::new_cache(),
    });

    let mut app = build_router(state);

    let body = serde_json::json!({
        "message": message,
        "role": null,
        "history": []
    })
    .to_string();

    let request = Request::builder()
        .uri("/api/ai/chat")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(body))
        .unwrap();

    let response = Service::call(&mut app, request).await.unwrap();
    assert_eq!(response.status(), 200, "expected 200 OK");

    // Collect SSE events from the response body
    let body = response.into_body();
    let collected = BodyExt::collect(body).await.unwrap();
    let bytes = collected.to_bytes();
    let raw = String::from_utf8_lossy(&bytes);

    // Split SSE format: "data: ...\n\n" or "event: ...\ndata: ...\n\n"
    raw.split("\n\n")
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

#[tokio::test]
#[ignore = "requires live LLM API"]
async fn test_ai_chat_returns_sse_events() {
    if !api_key_available() {
        eprintln!("[skip] no LLM_API_KEY / DEEPSEEK_API_KEY / OPENROUTER_API_KEY");
        return;
    }

    let events = run_chat("你好，请简单介绍一下你自己，不需要调用工具。").await;

    eprintln!("\n=== SSE events ({}) ===", events.len());
    for (i, ev) in events.iter().enumerate() {
        eprintln!("  [{i}] {ev}");
    }

    // Should have at least 2 events: text + [DONE]
    assert!(!events.is_empty(), "expected at least one SSE event");
    assert!(
        events.iter().any(|e| e.contains("[DONE]")),
        "expected [DONE] event"
    );
}

#[tokio::test]
#[ignore = "requires live LLM API"]
async fn test_ai_chat_calls_tool() {
    if !api_key_available() {
        eprintln!("[skip] no API key set");
        return;
    }

    let events = run_chat("请用 get_limit_up 工具查询今天的涨停股票，并简要总结前 5 只。").await;

    eprintln!("\n=== SSE events ({}) ===", events.len());
    for (i, ev) in events.iter().enumerate() {
        eprintln!("  [{i}] {ev}");
    }

    // Should have tool_call, text result, and [DONE]
    let has_tool_call = events.iter().any(|e| e.starts_with("event: tool"));
    let has_text = events
        .iter()
        .any(|e| e.starts_with("data: ") && !e.contains("[ERROR]") && !e.contains("[DONE]"));
    let has_done = events.iter().any(|e| e.contains("[DONE]"));

    assert!(
        has_tool_call,
        "expected tool_call event: tool_call not found"
    );
    assert!(has_text, "expected text content from assistant");
    assert!(has_done, "expected [DONE] event");
}
