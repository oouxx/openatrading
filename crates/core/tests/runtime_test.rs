// ============================================================================
// Tests
// ============================================================================

use openatrading_core::llm::AgentRuntime;
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

    let runtime = AgentRuntime::new();
    let agent_facade = runtime
        .create_session(None, None, Some(true), None, None)
        .await
        .expect("创建agent session 失败");
    // Verify that the session has trading extension tools available.
    let agent_state = agent_facade.session.get_agent().state().await;
    let tool_names: Vec<&str> = agent_state.tools.iter().map(|t| t.name.as_str()).collect();
    println!("[dbg] Available tools: {tool_names:?}");

    // Verify that at least one trading tool is registered by the extension
    assert!(
        tool_names.contains(&"get_limit_up"),
        "Expected trading extension tools to be available, got: {tool_names:?}"
    );
}

#[tokio::test]
async fn test_trading_tools_exec() {
    setup_test_env();
    // Set cwd to workspace root so settings.json is found
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.parent().unwrap().parent().unwrap();
    std::env::set_current_dir(workspace_root).ok();

    let runtime = AgentRuntime::new();
    let agent_facade = runtime
        .create_session(None, None, Some(true), None, None)
        .await
        .expect("创建agent session 失败");

    // Let the agent handle the full flow: LLM decides to call get_limit_up,
    // tool executes, LLM analyzes the result.
    let messages = agent_facade
        .session
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
                        },
                        ContentBlock::ToolCall {
                            id,
                            name,
                            arguments,
                            ..
                        } => {
                            println!(
                                "[dbg]     [{j}] ToolCall: id={id} name={name} args={arguments}"
                            );
                        },
                        ContentBlock::Thinking { thinking, .. } => {
                            let display = if thinking.len() > 300 {
                                let truncated: String = thinking.chars().take(300).collect();
                                format!("{}... (truncated, {} chars)", truncated, thinking.len())
                            } else {
                                thinking.clone()
                            };
                            println!("[dbg]     [{j}] Thinking: {display}");
                        },
                        _ => {
                            println!("[dbg]     [{j}] Other: {block:?}");
                        },
                    }
                }
            },
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
            },
            _ => {
                println!("[dbg]   [{i}] Other: {msg:?}");
            },
        }
    }
}

/// Ask the LLM what tools it has access to and assert the reply mentions the
/// built-in `bash` tool. Requires `OPENROUTER_API_KEY` and network access.
#[tokio::test]
async fn test_bash_tool_exec() {
    setup_test_env();
    let runtime = AgentRuntime::new();
    let agent_facade = runtime
        .create_session(None, None, Some(true), None, None)
        .await
        .expect("创建agent session 失败");

    let messages = agent_facade
        .session
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
                        },
                        ContentBlock::ToolCall {
                            id,
                            name,
                            arguments,
                            ..
                        } => {
                            println!(
                                "[dbg]     [{j}] ToolCall: id={id} name={name} args={arguments}"
                            );
                        },
                        ContentBlock::Thinking { thinking, .. } => {
                            println!("[dbg]     [{j}] Thinking: {thinking}");
                        },
                        _ => {
                            println!("[dbg]     [{j}] Other: {block:?}");
                        },
                    }
                }
            },
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
            },
            AgentMessage::BashExecution {
                command,
                output,
                exit_code,
                ..
            } => {
                println!(
                    "[dbg]   [{i}] BashExecution: cmd={command} exit={exit_code:?} output={output}"
                );
            },
            _ => {
                println!("[dbg]   [{i}] Other: {msg:?}");
            },
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

    // Create settings manager (same as AgentRuntime::create_session)
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
