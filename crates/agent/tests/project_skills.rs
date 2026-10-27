//! Project Skills must be parsed and exposed by pi-rs from `.agents/skills`.

#![allow(clippy::expect_used)]

use openatrading_agent::session::{create_factory, CreateSessionParams, FactoryOptions};
use pi_coding_agent::core::session_manager::SessionManager;

#[test]
fn factory_exposes_agents_skills_as_pi_rs_commands() {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => panic!("failed to build runtime: {e}"),
    };

    runtime.block_on(async {
        let temp = tempfile::tempdir().expect("tempdir");
        let cwd = temp.path().join("project");
        let skill_dir = cwd.join(".agents/skills/project-analysis");
        std::fs::create_dir_all(&skill_dir).expect("create skill dir");
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: project-analysis\ndescription: Project analysis skill\n---\n\nUse project data.\n",
        )
        .expect("write skill");

        let agent_dir = temp.path().join("agent");
        let cwd = cwd.to_string_lossy().to_string();
        let agent_dir = agent_dir.to_string_lossy().to_string();
        let sessions_dir = temp
            .path()
            .join("sessions")
            .to_string_lossy()
            .to_string();
        let factory = create_factory(FactoryOptions::new(agent_dir.clone()));
        let created = factory(CreateSessionParams {
            session_manager: SessionManager::new(
                &cwd,
                &sessions_dir,
                None,
                false,
                None,
            ),
            cwd,
            agent_dir,
            exclude_tools: None,
            session_start_event: None,
        })
        .await
        .expect("create agent session");

        assert!(
            created
                .session
                .get_commands_info()
                .iter()
                .any(|command| command.name == "skill:project-analysis"),
            "pi-rs did not expose `.agents/skills` as a `/skill:` command"
        );
    });
}
