use pi_coding_agent::core::agent_session_runtime::*;
use pi_coding_agent::core::agent_session_services::*;
use pi_coding_agent::pi_ai_types::Model;
use pi_extension_api::ToolDefinition;

/// Create a factory closure that produces `AgentSessionRuntime` instances.
///
/// The factory captures configuration (agent_dir, model, tools, custom_tools)
/// once and reuses it for every session, avoiding repeated setup.
///
/// The extension registry is created fresh for each session via
/// `openatrading_extension::create_registry()` since `ExtensionRegistry`
/// does not implement `Clone` and cannot be shared across sessions.
pub fn create_factory(
    agent_dir: &str,
    model: Option<Model>,
    tools: Option<Vec<String>>,
    custom_tools: Option<Vec<ToolDefinition>>,
) -> CreateAgentSessionRuntimeFactory {
    let agent_dir = agent_dir.to_string();

    Box::new(move |params: CreateAgentSessionRuntimeParams| {
        let agent_dir = agent_dir.clone();
        let model = model.clone();
        let tools = tools.clone();
        let custom_tools = custom_tools.clone();

        Box::pin(async move {
            let services = create_agent_session_services(CreateAgentSessionServicesOptions {
                cwd: params.cwd.clone(),
                agent_dir: Some(agent_dir),
                auth_storage: None,
                settings_manager: None,
                model_registry: None,
                resource_loader_options: None,
            })
            .await;

            let diagnostics = services.diagnostics.clone();

            // Create a fresh extension registry for each session.
            let ext_registry = openatrading_extension::create_registry();

            let (session, services, result) =
                create_agent_session_from_services(CreateAgentSessionFromServicesOptions {
                    services,
                    session_manager: params.session_manager,
                    model,
                    thinking_level: None,
                    scoped_models: None,
                    tools,
                    no_tools: None,
                    exclude_tools: None,
                    custom_tools,
                    extension_registry: Some(ext_registry),
                    fallback_message: None,
                    session_start_event: params.session_start_event,
                })
                .await
                .expect("Failed to create session");

            CreateAgentSessionRuntimeResult {
                session,
                services,
                diagnostics,
                model_fallback_message: result.model_fallback_message,
            }
        })
    })
}

#[tokio::test]
async fn test_factory() {
    let dir = tempfile::tempdir().unwrap();
    let cwd = dir.path().to_string_lossy().to_string();
    let session_dir = dir.path().join("sessions");
    std::fs::create_dir_all(&session_dir).unwrap();

    let factory = create_factory("/tmp/pi", None, None, None);
    let session_manager = pi_coding_agent::core::session_manager::SessionManager::new(
        &cwd,
        &session_dir.to_string_lossy(),
        None,
        false,
        None,
    );

    let runtime = create_agent_session_runtime(
        factory,
        CreateAgentSessionRuntimeParams {
            cwd: cwd.clone(),
            agent_dir: "/tmp/pi".into(),
            session_manager,
            session_start_event: None,
        },
    )
    .await;

    assert!(!runtime.cwd().is_empty());
    assert!(!runtime.session().get_session_id().is_empty());
}
