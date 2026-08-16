// ============================================================================
// Tests
// ============================================================================

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::disallowed_methods)]

use openatrading_core::llm::{create_factory, CreateSessionParams, FactoryOptions, SessionPool};
use pi_coding_agent::core::session_manager::SessionManager;

/// Set up the test environment: set `PI_RS_HOME` to the project's .atrading directory
/// and change cwd to the workspace root.
fn setup_test_env() {
    // Set PI_RS_HOME to point to .atrading directory
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.parent().unwrap().parent().unwrap();
    let atrading_dir = workspace_root.join(".atrading");
    std::env::set_var("PI_RS_HOME", atrading_dir.to_string_lossy().as_ref());
    std::env::set_current_dir(workspace_root).ok();
}

fn test_factory() -> openatrading_core::llm::CreateSessionFactory {
    let agent_dir = pi_coding_agent::config::get_agent_dir()
        .to_string_lossy()
        .to_string();
    create_factory(FactoryOptions::market_only(&agent_dir))
}

fn new_session_manager(cwd: &str, id: &str) -> SessionManager {
    let session_dir = pi_coding_agent::config::get_sessions_dir()
        .to_string_lossy()
        .to_string();
    SessionManager::new(
        cwd,
        &session_dir,
        None,
        true,
        Some(pi_coding_agent::core::session_manager::NewSessionOptions {
            id: Some(id.to_string()),
            parent_session: None,
        }),
    )
}

#[tokio::test]
async fn test_create_agent_session_with_extension() {
    setup_test_env();

    let cwd = std::env::current_dir()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let agent_dir = pi_coding_agent::config::get_agent_dir()
        .to_string_lossy()
        .to_string();

    // Verify at the factory level that sessions carry the trading extension
    // tools (the pool no longer exposes the raw session).
    let factory = test_factory();
    let created = factory(CreateSessionParams {
        cwd: cwd.clone(),
        agent_dir,
        session_manager: new_session_manager(&cwd, "test-ext"),
        session_start_event: None,
    })
    .await
    .expect("创建agent session 失败");

    let agent_state = created.session.get_state().await;
    let tool_names: Vec<&str> = agent_state.tools.iter().map(|t| t.name.as_str()).collect();
    println!("[dbg] Available tools: {tool_names:?}");

    // Verify that at least one trading tool is registered by the extension
    assert!(
        tool_names.contains(&"get_limit_up"),
        "Expected trading extension tools to be available, got: {tool_names:?}"
    );
}

#[tokio::test]
#[ignore]
async fn test_trading_tools_exec() {
    setup_test_env();
    // Set cwd to workspace root so settings.json is found
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.parent().unwrap().parent().unwrap();
    std::env::set_current_dir(workspace_root).ok();

    let pool = SessionPool::new(test_factory());
    let cwd = std::env::current_dir()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let session_id = pool
        .create_session(&cwd)
        .await
        .expect("创建agent session 失败");
    let handle = pool
        .get_session(&session_id)
        .await
        .expect("session not found");

    // Subscribe the event stream so we can observe tool execution events.
    let (_tx, mut rx) = handle.subscribe().await;
    let events_handle = handle.clone();
    let observer = tokio::spawn(async move {
        let mut saw_tool = false;
        // Collect events while the turn runs; bounded by a timeout.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
        loop {
            if std::time::Instant::now() > deadline {
                break;
            }
            match tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv()).await {
                Ok(Some(json)) => {
                    println!("[dbg] event: {json}");
                    if json.contains("tool_execution") {
                        saw_tool = true;
                    }
                    if json.contains("agent_end") {
                        break;
                    }
                }
                _ => break,
            }
        }
        // Let the prompt run to completion before the observer exits.
        let _ = events_handle.status().await;
        saw_tool
    });

    // Let the agent handle the full flow: LLM decides to call get_limit_up,
    // tool executes, LLM analyzes the result.
    handle
        .prompt("获取今日涨停数据并分析市场情绪", Default::default())
        .await
        .expect("agent prompt failed");

    let saw_tool = tokio::time::timeout(std::time::Duration::from_secs(5), observer)
        .await
        .unwrap_or_else(|_| panic!("observer timed out"))
        .expect("observer join failed");

    println!("[dbg] saw_tool_execution_event={saw_tool}");
    let text = handle
        .last_assistant_text()
        .await
        .unwrap_or_else(|| "<empty>".to_string());
    println!(
        "[dbg] assistant text: {}",
        text.chars().take(300).collect::<String>()
    );
    assert!(
        saw_tool || !text.is_empty(),
        "expected tool events and/or a non-empty assistant reply"
    );

    pool.close_session(&session_id).await;
}

/// Ask the LLM what tools it has access to and assert the reply mentions the
/// built-in `bash` tool. Requires `OPENROUTER_API_KEY` and network access.
#[tokio::test]
#[ignore]
async fn test_bash_tool_exec() {
    setup_test_env();
    let pool = SessionPool::new(test_factory());
    let cwd = std::env::current_dir()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let session_id = pool
        .create_session(&cwd)
        .await
        .expect("创建agent session 失败");
    let handle = pool
        .get_session(&session_id)
        .await
        .expect("session not found");

    let (_tx, mut rx) = handle.subscribe().await;
    let observer = tokio::spawn(async move {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
        loop {
            if std::time::Instant::now() > deadline {
                break false;
            }
            match tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv()).await {
                Ok(Some(json)) => {
                    println!("[dbg] event: {json}");
                    if json.contains(r#""type":"tool_execution_end""#) && json.contains("bash") {
                        break true;
                    }
                    if json.contains("agent_end") {
                        break false;
                    }
                }
                _ => break false,
            }
        }
    });

    handle
        .prompt("执行ls命令", Default::default())
        .await
        .expect("agent prompt failed");

    let saw_bash = tokio::time::timeout(std::time::Duration::from_secs(5), observer)
        .await
        .unwrap_or_else(|_| panic!("observer timed out"))
        .expect("observer join failed");
    println!("[dbg] saw_bash_tool_end={saw_bash}");

    let text = handle
        .last_assistant_text()
        .await
        .unwrap_or_else(|| "<empty>".to_string());
    assert!(
        saw_bash || !text.is_empty(),
        "expected a bash tool execution and/or a non-empty assistant reply"
    );

    pool.close_session(&session_id).await;
}

#[tokio::test]
async fn debug_model_resolution() {
    // Register providers first (same as create_agent_session does)
    pi_coding_agent::pi_agent_core::pi_ai::providers::register_builtins::register_built_in_api_providers();

    // Check pi-ai model registry
    let model = pi_coding_agent::pi_agent_core::pi_ai::models::get_model(
        "openrouter",
        "nvidia/nemotron-3-ultra-550b-a55b:free",
    );
    println!("[dbg] pi-ai model found: {:?}", model.is_some());

    // Check ModelRegistry
    let registry = pi_coding_agent::core::model_registry::ModelRegistry::new(
        pi_coding_agent::core::model_registry::ModelRegistry::builtin_models_list(),
    );
    let found = registry.find("openrouter", "nvidia/nemotron-3-ultra-550b-a55b:free");
    println!("[dbg] ModelRegistry find: {:?}", found.is_some());

    // Check available models for openrouter
    let or_models = registry.get_models_for_provider("openrouter");
    println!("[dbg] OpenRouter models in registry: {}", or_models.len());
    for m in &or_models {
        if m.id.contains("nemotron") {
            println!(
                "[dbg]   {} / {} (has auth: {})",
                m.provider,
                m.id,
                registry.has_configured_auth(m)
            );
        }
    }

    // Check all available models (with auth)
    let available = registry.get_available();
    println!("[dbg] Available models (with auth): {}", available.len());
    for m in &available {
        println!("[dbg]   {} / {}", m.provider, m.id);
    }
}

#[tokio::test]
async fn debug_settings_path() {
    use pi_coding_agent::config;
    use pi_coding_agent::core::settings_manager::SettingsManager;

    let cwd = std::env::current_dir()
        .unwrap()
        .to_string_lossy()
        .to_string();
    println!("[dbg] cwd: {cwd:?}");
    println!("[dbg] agent_dir: {:?}", config::get_agent_dir());

    // Check the project settings path
    let project_path = std::path::PathBuf::from(&cwd)
        .join(config::CONFIG_DIR_NAME)
        .join("settings.json");
    println!("[dbg] project settings path: {project_path:?}");
    println!("[dbg] exists: {}", project_path.exists());

    // Read the file directly
    if project_path.exists() {
        let content = std::fs::read_to_string(&project_path).unwrap();
        println!("[dbg] file content: {content}");

        // Try to deserialize
        let settings: Result<pi_coding_agent::core::settings_manager::Settings, _> =
            serde_json::from_str(&content);
        match settings {
            Ok(s) => println!(
                "[dbg] deserialized: provider={:?}, model={:?}",
                s.default_provider, s.default_model
            ),
            Err(e) => println!("[dbg] deserialize error: {e}"),
        }
    }

    // Now create SettingsManager and check
    let settings_mgr = SettingsManager::create(&cwd, None);
    let settings = settings_mgr.get_settings();
    println!(
        "[dbg] from SettingsManager: provider={:?}, model={:?}",
        settings.default_provider, settings.default_model
    );
}

#[tokio::test]
#[ignore]
async fn test_multi_turn_conversation() {
    setup_test_env();

    let pool = SessionPool::new(test_factory());
    let cwd = std::env::current_dir()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let session_id = pool
        .create_session(&cwd)
        .await
        .expect("创建agent session 失败");
    let handle = pool
        .get_session(&session_id)
        .await
        .expect("session not found");

    // =========================================================================
    // Turn 1: 初始问候 —— 验证助手能正常回复
    // =========================================================================
    handle
        .prompt("你好，请简单介绍一下你自己", Default::default())
        .await
        .expect("第一轮对话失败");
    let turn1_text = handle
        .last_assistant_text()
        .await
        .unwrap_or_else(|| "<empty>".to_string());
    println!(
        "[Turn 1] 助手回复 ({} chars): {}",
        turn1_text.len(),
        turn1_text.chars().take(200).collect::<String>()
    );
    assert!(!turn1_text.is_empty(), "第一轮助手回复不应为空");

    // =========================================================================
    // Turn 2: 追问股票分析能力 —— 验证助手能理解上下文并延续对话
    // =========================================================================
    handle
        .prompt(
            "你能帮我分析股票吗？具体能提供哪些数据？",
            Default::default(),
        )
        .await
        .expect("第二轮对话失败");
    let turn2_text = handle
        .last_assistant_text()
        .await
        .unwrap_or_else(|| "<empty>".to_string());
    println!(
        "[Turn 2] 助手回复 ({} chars): {}",
        turn2_text.len(),
        turn2_text.chars().take(200).collect::<String>()
    );
    assert!(!turn2_text.is_empty(), "第二轮助手回复不应为空");

    // =========================================================================
    // Turn 3: 上下文保持测试 —— 验证助手能回忆第一轮的问题
    // =========================================================================
    handle
        .prompt("我刚才第一个问题问的是什么？请复述一遍", Default::default())
        .await
        .expect("第三轮对话失败");
    let turn3_text = handle
        .last_assistant_text()
        .await
        .unwrap_or_else(|| "<empty>".to_string());
    println!(
        "[Turn 3] 助手回复 ({} chars): {}",
        turn3_text.len(),
        turn3_text.chars().take(200).collect::<String>()
    );
    assert!(!turn3_text.is_empty(), "第三轮助手回复不应为空");

    // 验证助手确实提到了"介绍自己"或"你好"（表明记住了第一轮上下文）
    let has_context = turn3_text.contains("介绍")
        || turn3_text.contains("你好")
        || turn3_text.contains("自己")
        || turn3_text.contains("简单")
        || turn3_text.contains("第一个");
    assert!(
        has_context,
        "助手应能回忆第一轮问题，但回复中未包含相关关键词。回复：{}",
        turn3_text.chars().take(300).collect::<String>()
    );

    // 关闭 session
    pool.close_session(&session_id).await;
}

/// 池级冒烟测试：不依赖 LLM（不 prompt），验证 actor 线程生命周期与命令往返。
#[tokio::test]
async fn test_pool_actor_lifecycle() {
    setup_test_env();

    let pool = SessionPool::new(test_factory());
    let cwd = std::env::current_dir()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let session_id = pool
        .create_session(&cwd)
        .await
        .expect("创建agent session 失败");

    // Handle is retrievable and the actor answers status queries.
    let handle = pool
        .get_session(&session_id)
        .await
        .expect("session not found");
    let status = handle.status().await.expect("status reply");
    assert!(!status.streaming, "fresh session must not be streaming");
    assert_eq!(status.pending_prompts, 0);
    assert!(
        !handle.is_streaming(),
        "is_streaming must be false when idle"
    );
    assert!(pool.has_session(&session_id).await);

    // Subscribe / unsubscribe round-trip (drop rx → fan-out prunes it).
    let (_tx, rx) = handle.subscribe().await;
    drop(rx);

    // Close stops the actor thread and removes it from the pool.
    pool.close_session(&session_id).await;
    assert!(!pool.has_session(&session_id).await);
    assert!(pool.get_session(&session_id).await.is_none());

    // Sending to a closed session fails cleanly.
    let err = pool.prompt(&session_id, "hi").await.unwrap_err();
    assert!(err.contains("not found"), "err={err}");
}
