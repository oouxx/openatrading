use std::sync::Arc;

use pi_coding_agent::core::agent_session::AgentSession;
use pi_coding_agent::core::agent_session_services::{
    create_agent_session_from_services, create_agent_session_services, AgentSessionServices,
    CreateAgentSessionFromServicesOptions, CreateAgentSessionServicesOptions,
};
use pi_coding_agent::core::model_registry::ModelRegistry;
use pi_coding_agent::core::sdk::SessionStartEvent;
use pi_coding_agent::core::session_manager::SessionManager;
use pi_coding_agent::pi_ai_types::Model;
use pi_extension_api::ToolDefinition;

/// Options for [`create_factory`].
///
/// Bundling these into a struct keeps `create_factory` to a single parameter
/// (per the project's ≤3-params rule).
pub struct FactoryOptions {
    /// pi-rs agent directory (`$PI_RS_HOME/agent`).
    pub agent_dir: String,
    /// Override the model resolved from settings; `None` = use settings.
    pub model: Option<Model>,
    /// Restrict the tool set to these names; `None` = all tools.
    pub tools: Option<Vec<String>>,
    /// Extra custom tool definitions to register.
    pub custom_tools: Option<Vec<ToolDefinition>>,
}

impl FactoryOptions {
    /// 默认选项：模型由 settings 解析，注册全部工具。
    #[must_use]
    pub fn new(agent_dir: impl Into<String>) -> Self {
        Self {
            agent_dir: agent_dir.into(),
            model: None,
            tools: None,
            custom_tools: None,
        }
    }
}

/// Parameters for creating one session via the factory.
///
/// Bundled into a struct (per the project's ≤3-params rule).
pub struct CreateSessionParams {
    pub cwd: String,
    pub agent_dir: String,
    /// 会话级工具排除清单(如只读自动化排除 bash/edit/write);None = 不限制。
    pub exclude_tools: Option<Vec<String>>,
    pub session_manager: SessionManager,
    /// Session start event metadata, passed through to pi-rs so extensions
    /// receive `session_start` with the correct reason and previous session
    /// file (used on resume).
    pub session_start_event: Option<SessionStartEvent>,
}

/// A freshly created session, ready to be owned by a session actor.
///
/// `services` must be kept alive for the session's lifetime (it holds the
/// cwd-bound services and extension registry the session depends on).
pub struct CreatedSession {
    pub session: Arc<AgentSession>,
    pub services: AgentSessionServices,
    pub cwd: String,
}

/// Factory type for creating one [`CreatedSession`].
///
/// The factory closes over process-global fixed inputs and recreates
/// cwd-bound services for the effective cwd, then creates the session.
pub type CreateSessionFactory = Arc<
    dyn Fn(
            CreateSessionParams,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<CreatedSession, String>> + Send>,
        > + Send
        + Sync,
>;

/// Create a factory closure that produces [`CreatedSession`] instances.
///
/// The factory captures configuration once and reuses it for every session,
/// avoiding repeated setup. The extension registry is created fresh for each
/// session via `openatrading_extension::create_registry()` since
/// `ExtensionRegistry` does not implement `Clone`.
pub fn create_factory(opts: FactoryOptions) -> CreateSessionFactory {
    let agent_dir = opts.agent_dir;
    let model = opts.model;
    let tools = opts.tools;
    let custom_tools = opts.custom_tools;

    Arc::new(move |params: CreateSessionParams| {
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

            for d in &services.diagnostics {
                eprintln!("[session-factory] {d:?}");
            }

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

            let (session, services, _result) =
                create_agent_session_from_services(CreateAgentSessionFromServicesOptions {
                    services,
                    session_manager: params.session_manager,
                    model: resolved_model,
                    thinking_level: None,
                    scoped_models: None,
                    tools,
                    no_tools: None,
                    exclude_tools: params.exclude_tools.clone(),
                    custom_tools,
                    tools_options: None,
                    extension_registry: Some(ext_registry),
                    fallback_message: None,
                    session_start_event: params.session_start_event,
                    ui_context: None,
                })
                .await
                .map_err(|e| format!("Failed to create session: {e}"))?;

            Ok(CreatedSession {
                session: Arc::new(session),
                services,
                cwd: params.cwd,
            })
        })
    })
}
