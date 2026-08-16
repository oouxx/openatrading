//! Session-level slash command handling.
//!
//! These commands mutate `AgentSession` state that pi-rs `prompt()` does not
//! touch (model, name, compaction, …). They are executed by the session actor
//! itself so the session is never accessed concurrently from outside the
//! actor's thread.

use pi_coding_agent::core::agent_session::AgentSession;

/// Execute a session-level slash command against `session`.
///
/// All `AgentSession` methods used here are `&self` (pi-rs serializes access
/// internally), so the actor can run commands without exclusive `&mut` access
/// while still keeping session access single-threaded.
pub async fn handle_slash_command(
    cmd_name: &str,
    cmd_args: &str,
    session: &AgentSession,
) -> Result<String, String> {
    match cmd_name {
        "model" => {
            // Resolve model from "provider/model" format
            let model_registry = session.get_model_registry();
            let result = pi_coding_agent::core::model_resolver::resolve_cli_model(
                None,
                Some(cmd_args),
                model_registry,
            );
            match result.model {
                Some(model) => {
                    let display = format!("{}/{}", model.provider, model.id);
                    session
                        .set_model(model)
                        .await
                        .map(|_| format!("Model set to {display}"))
                        .map_err(|e| format!("Failed to set model: {e}"))
                }
                None => {
                    let err = result.error.unwrap_or_else(|| "Unknown model".to_string());
                    Err(format!("Failed to resolve model: {err}"))
                }
            }
        }
        "name" => {
            if cmd_args.is_empty() {
                return Err("Usage: /name <session name>".to_string());
            }
            session.set_session_name(cmd_args);
            Ok(format!("Session name set to \"{cmd_args}\""))
        }
        "compact" => match session.compact(None).await {
            Ok(_) => Ok("Session compacted".to_string()),
            Err(e) => Err(format!("Compaction failed: {e}")),
        },
        "session" => {
            let sid = session.get_session_id();
            let name = session.get_session_name().unwrap_or_default();
            let mgr = session.get_session_manager();
            let msg_count = mgr.get_entries().len();
            let file = mgr
                .get_session_file()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            drop(mgr);
            Ok(format!(
                "Session: {name} ({sid})\nMessages: {msg_count}\nFile: {file}"
            ))
        }
        "help" | "?" => {
            let commands = pi_coding_agent::core::slash_commands::builtin_slash_commands();
            let mut help = "Available slash commands:\n".to_string();
            for cmd in &commands {
                let hint = cmd.argument_hint.as_deref().unwrap_or("");
                help.push_str(&format!(
                    "  /{} {} — {}\n",
                    cmd.name,
                    if hint.is_empty() { "" } else { hint },
                    cmd.description
                ));
            }
            Ok(help)
        }
        _ => Err(format!("Unknown slash command: /{cmd_name}")),
    }
}

/// Session-level slash commands handled locally (they mutate AgentSession
/// state, which pi-rs `prompt()` does not do). `/skill:` is intentionally
/// absent — it flows through to pi-rs `prompt()`, which expands it itself.
pub const SESSION_SLASH_COMMANDS: &[&str] = &["model", "name", "compact", "session", "help", "?"];
