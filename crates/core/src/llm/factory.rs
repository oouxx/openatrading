use pi_coding_agent::core::agent_session_runtime::*;
use pi_coding_agent::core::agent_session_services::*;
use pi_coding_agent::core::model_registry::ModelRegistry;
use pi_coding_agent::pi_ai_types::Model;
use pi_extension_api::ToolDefinition;
use std::sync::Arc;

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

    Arc::new(move |params: CreateAgentSessionRuntimeParams| {
        let agent_dir = agent_dir.clone();
        let model = model.clone();
        let tools = tools.clone();
        let custom_tools = custom_tools.clone();

        Box::pin(async move {
            // Ensure API providers are registered before any LLM calls
            pi_coding_agent::pi_agent_core::pi_ai::providers::register_builtins::register_built_in_api_providers();

            let services = create_agent_session_services(CreateAgentSessionServicesOptions {
                cwd: params.cwd.clone(),
                agent_dir: Some(agent_dir),
                auth_storage: None,
                settings_manager: None,
                model_registry: Some(ModelRegistry::new(ModelRegistry::builtin_models_list())),
                resource_loader_options: None,
            })
            .await;

            let diagnostics = services.diagnostics.clone();

            // Resolve model from settings if not explicitly provided
            let resolved_model = model.clone().or_else(|| {
                let settings = services.settings_manager.get_settings();
                let provider = settings.default_provider.as_deref();
                let model_id = settings.default_model.as_deref();
                match (provider, model_id) {
                    (Some(p), Some(m)) => services.model_registry.find(p, m),
                    _ => None,
                }
            });

            // Create a fresh extension registry for each session.
            let ext_registry = openatrading_extension::create_registry();

            let (session, services, result) =
                create_agent_session_from_services(CreateAgentSessionFromServicesOptions {
                    services,
                    session_manager: params.session_manager,
                    model: resolved_model,
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
                .unwrap_or_else(|e| panic!("Failed to create session: {e}"));

            CreateAgentSessionRuntimeResult {
                session,
                services,
                diagnostics,
                model_fallback_message: result.model_fallback_message,
            }
        })
    })
}


