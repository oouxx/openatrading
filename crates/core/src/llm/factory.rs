use pi_coding_agent::core::agent_session_runtime::*;
use pi_coding_agent::core::agent_session_services::*;
use pi_coding_agent::pi_ai_types::Model;
use pi_extension_api::ExtensionRegistry;
fn create_custom_factory(
    agent_dir: &str,
    model: Option<Model>,
    tools: Option<Vec<String>>,
    extension_registry: Option<ExtensionRegistry>,
) -> CreateAgentSessionRuntimeFactory {
    let agent_dir = agent_dir.to_string();

    Box::new(move |params: CreateAgentSessionRuntimeParams| {
        let agent_dir = agent_dir.clone();
        let model = model.clone();
        let tools = tools.clone();
        let ext_reg = extension_registry.clone();

        Box::pin(async move {
            let services = create_agent_session_services(CreateAgentSessionServicesOptions {
                cwd: params.cwd.clone(),
                agent_dir: Some(agent_dir),
                ..Default::default()
            })
            .await;

            let (session, result) =
                create_agent_session_from_services(CreateAgentSessionFromServicesOptions {
                    services,
                    session_manager: params.session_manager,
                    model,
                    tools,
                    extension_registry: ext_reg,
                    session_start_event: params.session_start_event,
                    ..Default::default()
                })
                .await
                .expect("Failed to create session");

            CreateAgentSessionRuntimeResult {
                session,
                services: result.services,
                diagnostics: result.diagnostics,
                model_fallback_message: result.model_fallback_message,
            }
        })
    })
}
#[tokio::test]
async fn test_my_factory() {
    let dir = tempfile::tempdir().unwrap();
    let cwd = dir.path().to_string_lossy().to_string();
    let session_dir = dir.path().join("sessions");
    std::fs::create_dir_all(&session_dir).unwrap();

    let factory = create_my_factory("/tmp/pi");
    let session_manager =
        SessionManager::new(&cwd, &session_dir.to_string_lossy(), None, false, None);

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
    assert!(runtime.session().get_session_id().len() > 0);
}
