// ============================================================================
// Tests
// ============================================================================

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::disallowed_methods)]

use openatrading_core::llm::{create_factory, FactoryOptions, SessionPool};
use pi_coding_agent::pi_agent_core::agent::PromptInput;
use pi_coding_agent::pi_agent_core::types::AgentMessage;
use pi_coding_agent::pi_ai_types::ContentBlock;

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

#[tokio::test]
async fn test_create_agent_session_with_extension() {
    setup_test_env();

    let agent_dir = pi_coding_agent::config::get_agent_dir()
        .to_string_lossy()
        .to_string();
    let pool = SessionPool::new(create_factory(FactoryOptions::market_only(&agent_dir)));
    let cwd = std::env::current_dir()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let session_id = pool
        .create_session(&cwd)
        .await
        .expect("创建agent session 失败");
    let rt = pool
        .get_session(&session_id)
        .await
        .expect("runtime not found");
    let rt = rt.lock().await;
    let session = rt.session();
    // Verify that the session has trading extension tools available.
    let agent_state = session.get_state().await;
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

    let agent_dir = pi_coding_agent::config::get_agent_dir()
        .to_string_lossy()
        .to_string();
    let pool = SessionPool::new(create_factory(FactoryOptions::market_only(&agent_dir)));
    let cwd = std::env::current_dir()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let session_id = pool
        .create_session(&cwd)
        .await
        .expect("创建agent session 失败");
    let rt = pool
        .get_session(&session_id)
        .await
        .expect("runtime not found");
    let rt = rt.lock().await;
    let session = rt.session();

    // Let the agent handle the full flow: LLM decides to call get_limit_up,
    // tool executes, LLM analyzes the result.
    let messages = session
        .get_agent()
        .prompt(PromptInput::Text("获取今日涨停数据并分析市场情绪"))
        .await
        .expect("agent prompt failed");

    println!("[dbg] === All messages ({}) ===", messages.len());
    for (i, msg) in messages.iter().enumerate() {
        match msg {
            AgentMessage::Assistant {
                content,
                stop_reason,
                error_message,
                model,
                ..
            } => {
                println!(
                    "[dbg]   [{i}] Assistant ({} blocks) model={} stop_reason={:?} error={:?}:",
                    content.len(),
                    model,
                    stop_reason,
                    error_message
                );
                for (j, block) in content.iter().enumerate() {
                    match block {
                        ContentBlock::Text { text, .. } => {
                            let display = if text.len() > 300 {
                                let truncated: String = text.chars().take(300).collect();
                                format!("{}... (truncated, {} chars)", truncated, text.len())
                            } else {
                                text.clone()
                            };
                            println!("[dbg]     [{j}] Text: {display}");
                        }
                        ContentBlock::ToolCall {
                            id,
                            name,
                            arguments,
                            ..
                        } => {
                            println!(
                                "[dbg]     [{j}] ToolCall: id={id} name={name} args={arguments}"
                            );
                        }
                        ContentBlock::Thinking { thinking, .. } => {
                            let display = if thinking.len() > 300 {
                                let truncated: String = thinking.chars().take(300).collect();
                                format!("{}... (truncated, {} chars)", truncated, thinking.len())
                            } else {
                                thinking.clone()
                            };
                            println!("[dbg]     [{j}] Thinking: {display}");
                        }
                        _ => {
                            println!("[dbg]     [{j}] Other: {block:?}");
                        }
                    }
                }
            }
            AgentMessage::ToolResult {
                tool_call_id,
                tool_name,
                content,
                is_error,
                ..
            } => {
                let text: String = content
                    .iter()
                    .filter_map(|b| match b {
                        ContentBlock::Text { text, .. } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect();
                let display = if text.len() > 100 {
                    let truncated: String = text.chars().take(100).collect();
                    format!("{}... (truncated, {} chars)", truncated, text.len())
                } else {
                    text
                };
                println!("[dbg]   [{i}] ToolResult: tool={tool_name} id={tool_call_id} is_error={is_error} text={display}");
            }
            _ => {
                println!("[dbg]   [{i}] Other: {msg:?}");
            }
        }
    }
}

/// Ask the LLM what tools it has access to and assert the reply mentions the
/// built-in `bash` tool. Requires `OPENROUTER_API_KEY` and network access.
#[tokio::test]
#[ignore]
async fn test_bash_tool_exec() {
    setup_test_env();
    let agent_dir = pi_coding_agent::config::get_agent_dir()
        .to_string_lossy()
        .to_string();
    let pool = SessionPool::new(create_factory(FactoryOptions::market_only(&agent_dir)));
    let cwd = std::env::current_dir()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let session_id = pool
        .create_session(&cwd)
        .await
        .expect("创建agent session 失败");
    let rt = pool
        .get_session(&session_id)
        .await
        .expect("runtime not found");
    let rt = rt.lock().await;
    let session = rt.session();

    let messages = session
        .get_agent()
        .prompt(PromptInput::Text("执行ls命令"))
        .await
        .expect("agent prompt failed");

    // Print all messages for debugging
    println!("[dbg] === All messages ({}) ===", messages.len());
    for (i, msg) in messages.iter().enumerate() {
        match msg {
            AgentMessage::Assistant {
                content,
                stop_reason,
                error_message,
                model,
                ..
            } => {
                println!(
                    "[dbg]   [{i}] Assistant ({} blocks) model={} stop_reason={:?} error={:?}:",
                    content.len(),
                    model,
                    stop_reason,
                    error_message
                );
                for (j, block) in content.iter().enumerate() {
                    match block {
                        ContentBlock::Text { text, .. } => {
                            println!("[dbg]     [{j}] Text: {text}");
                        }
                        ContentBlock::ToolCall {
                            id,
                            name,
                            arguments,
                            ..
                        } => {
                            println!(
                                "[dbg]     [{j}] ToolCall: id={id} name={name} args={arguments}"
                            );
                        }
                        ContentBlock::Thinking { thinking, .. } => {
                            println!("[dbg]     [{j}] Thinking: {thinking}");
                        }
                        _ => {
                            println!("[dbg]     [{j}] Other: {block:?}");
                        }
                    }
                }
            }
            AgentMessage::ToolResult {
                tool_call_id,
                tool_name,
                content,
                is_error,
                ..
            } => {
                let text = content
                    .iter()
                    .filter_map(|b| match b {
                        ContentBlock::Text { text, .. } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("");
                println!("[dbg]   [{i}] ToolResult: tool={tool_name} id={tool_call_id} is_error={is_error} text={text}");
            }
            AgentMessage::BashExecution {
                command,
                output,
                exit_code,
                ..
            } => {
                println!(
                    "[dbg]   [{i}] BashExecution: cmd={command} exit={exit_code:?} output={output}"
                );
            }
            _ => {
                println!("[dbg]   [{i}] Other: {msg:?}");
            }
        }
    }

    // Verify that the LLM made at least one tool call (bash with ls)
    let has_tool_call = messages.iter().any(|m| match m {
        AgentMessage::Assistant { content, .. } => content
            .iter()
            .any(|b| matches!(b, ContentBlock::ToolCall { .. })),
        _ => false,
    });
    assert!(
        has_tool_call,
        "Expected at least one tool call in assistant messages"
    );

    // Verify that at least one tool result came back
    let has_tool_result = messages
        .iter()
        .any(|m| matches!(m, AgentMessage::ToolResult { .. }));
    assert!(has_tool_result, "Expected at least one tool result");
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
async fn debug_session_creation() {
    use pi_coding_agent::core::model_registry::ModelRegistry;
    use pi_coding_agent::core::settings_manager::SettingsManager;

    let cwd = std::env::current_dir()
        .unwrap()
        .to_string_lossy()
        .to_string();

    // Create settings manager (same as pool.create_session)
    let settings_mgr = SettingsManager::create(&cwd, None);
    let settings = settings_mgr.get_settings();
    println!("[dbg] default_provider: {:?}", settings.default_provider);
    println!("[dbg] default_model: {:?}", settings.default_model);

    // Register providers
    pi_coding_agent::pi_agent_core::pi_ai::providers::register_builtins::register_built_in_api_providers();

    // Create model registry
    let model_registry = ModelRegistry::new(ModelRegistry::builtin_models_list());

    // Try to find the model
    if let (Some(provider), Some(model_id)) = (&settings.default_provider, &settings.default_model)
    {
        let found = model_registry.find(provider, model_id);
        println!(
            "[dbg] find({:?}, {:?}) = {:?}",
            provider,
            model_id,
            found.is_some()
        );
        if let Some(m) = found {
            println!(
                "[dbg]   -> id={}, provider={}, api={}",
                m.id, m.provider, m.api
            );
        }
    }

    // Check what get_available returns
    let available = model_registry.get_available();
    println!("[dbg] available models: {}", available.len());
    for m in &available {
        if m.provider == "openrouter" || m.provider == "deepseek" {
            println!("[dbg]   {} / {}", m.provider, m.id);
        }
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

    let agent_dir = pi_coding_agent::config::get_agent_dir()
        .to_string_lossy()
        .to_string();
    let pool = SessionPool::new(create_factory(FactoryOptions::market_only(&agent_dir)));
    let cwd = std::env::current_dir()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let session_id = pool
        .create_session(&cwd)
        .await
        .expect("创建agent session 失败");
    let rt = pool
        .get_session(&session_id)
        .await
        .expect("runtime not found");
    let rt = rt.lock().await;
    let session = rt.session();

    // =========================================================================
    // Turn 1: 初始问候 —— 验证助手能正常回复
    // =========================================================================
    let turn1_messages = session
        .get_agent()
        .prompt(PromptInput::Text("你好，请简单介绍一下你自己"))
        .await
        .expect("第一轮对话失败");

    let turn1_assistant_messages: Vec<&AgentMessage> = turn1_messages
        .iter()
        .filter(|m| matches!(m, AgentMessage::Assistant { .. }))
        .collect();
    assert!(
        !turn1_assistant_messages.is_empty(),
        "第一轮应至少有一条 assistant 消息"
    );

    // 提取第一条 assistant 的文本内容
    let turn1_text: String = turn1_assistant_messages
        .iter()
        .flat_map(|msg| {
            if let AgentMessage::Assistant { content, .. } = msg {
                content
                    .iter()
                    .filter_map(|block| {
                        if let ContentBlock::Text { text, .. } = block {
                            Some(text.as_str())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<&str>>()
            } else {
                vec![]
            }
        })
        .collect();
    println!(
        "[Turn 1] 助手回复 ({} chars): {}",
        turn1_text.len(),
        turn1_text.chars().take(200).collect::<String>()
    );
    assert!(!turn1_text.is_empty(), "第一轮助手回复不应为空");

    // 验证 conversation 中已有 1 条 user + 1 条 assistant
    let conv1 = session.get_messages().await;
    let user_count_1 = conv1.iter().filter(|m| m.role() == "user").count();
    let asst_count_1 = conv1.iter().filter(|m| m.role() == "assistant").count();
    println!(
        "[Turn 1] 对话状态: user={user_count_1}, assistant={asst_count_1}, total={}",
        conv1.len()
    );
    assert!(user_count_1 >= 1, "第一轮后至少应有 1 条 user 消息");
    assert!(asst_count_1 >= 1, "第一轮后至少应有 1 条 assistant 消息");

    // =========================================================================
    // Turn 2: 追问股票分析能力 —— 验证助手能理解上下文并延续对话
    // =========================================================================
    let turn2_messages = session
        .get_agent()
        .prompt(PromptInput::Text(
            "你能帮我分析股票吗？具体能提供哪些数据？",
        ))
        .await
        .expect("第二轮对话失败");

    let turn2_assistant_messages: Vec<&AgentMessage> = turn2_messages
        .iter()
        .filter(|m| matches!(m, AgentMessage::Assistant { .. }))
        .collect();
    assert!(
        !turn2_assistant_messages.is_empty(),
        "第二轮应至少有一条 assistant 消息"
    );

    let turn2_text: String = turn2_assistant_messages
        .iter()
        .flat_map(|msg| {
            if let AgentMessage::Assistant { content, .. } = msg {
                content
                    .iter()
                    .filter_map(|block| {
                        if let ContentBlock::Text { text, .. } = block {
                            Some(text.as_str())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<&str>>()
            } else {
                vec![]
            }
        })
        .collect();
    println!(
        "[Turn 2] 助手回复 ({} chars): {}",
        turn2_text.len(),
        turn2_text.chars().take(200).collect::<String>()
    );
    assert!(!turn2_text.is_empty(), "第二轮助手回复不应为空");

    // 验证 conversation 已增长
    let conv2 = session.get_messages().await;
    let user_count_2 = conv2.iter().filter(|m| m.role() == "user").count();
    let asst_count_2 = conv2.iter().filter(|m| m.role() == "assistant").count();
    println!(
        "[Turn 2] 对话状态: user={user_count_2}, assistant={asst_count_2}, total={}",
        conv2.len()
    );
    assert!(
        user_count_2 > user_count_1,
        "第二轮后 user 消息应增加：之前 {user_count_1}，现在 {user_count_2}"
    );
    assert!(
        asst_count_2 > asst_count_1,
        "第二轮后 assistant 消息应增加：之前 {asst_count_1}，现在 {asst_count_2}"
    );

    // =========================================================================
    // Turn 3: 上下文保持测试 —— 验证助手能回忆第一轮的问题
    // =========================================================================
    let turn3_messages = session
        .get_agent()
        .prompt(PromptInput::Text("我刚才第一个问题问的是什么？请复述一遍"))
        .await
        .expect("第三轮对话失败");

    let turn3_assistant_messages: Vec<&AgentMessage> = turn3_messages
        .iter()
        .filter(|m| matches!(m, AgentMessage::Assistant { .. }))
        .collect();
    assert!(
        !turn3_assistant_messages.is_empty(),
        "第三轮应至少有一条 assistant 消息"
    );

    let turn3_text: String = turn3_assistant_messages
        .iter()
        .flat_map(|msg| {
            if let AgentMessage::Assistant { content, .. } = msg {
                content
                    .iter()
                    .filter_map(|block| {
                        if let ContentBlock::Text { text, .. } = block {
                            Some(text.as_str())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<&str>>()
            } else {
                vec![]
            }
        })
        .collect();
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

    // =========================================================================
    // 最终验证：整体 conversation 结构
    // =========================================================================
    let final_conv = session.get_messages().await;
    let total_user = final_conv.iter().filter(|m| m.role() == "user").count();
    let total_asst = final_conv
        .iter()
        .filter(|m| m.role() == "assistant")
        .count();
    let total_tool_results = final_conv
        .iter()
        .filter(|m| matches!(m, AgentMessage::ToolResult { .. }))
        .count();

    println!(
        "[Final] 三轮对话总览: user={total_user}, assistant={total_asst}, tool_result={total_tool_results}, total={}",
        final_conv.len()
    );

    // 三轮对话后至少有 3 条 user 消息和 3 条 assistant 消息
    assert!(
        total_user >= 3,
        "三轮对话应至少 3 条 user 消息，实际 {total_user}"
    );
    assert!(
        total_asst >= 3,
        "三轮对话应至少 3 条 assistant 消息，实际 {total_asst}"
    );

    // 关闭 session
    pool.close_session(&session_id).await;
}
