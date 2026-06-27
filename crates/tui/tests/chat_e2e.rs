use openatrading_tui::app::VirtualRuntime;
use openatrading_tui::chat::{ChatApp, ChatArgs, Msg};

const WIDTH: u16 = 80;
const HEIGHT: u16 = 24;

fn test_args() -> ChatArgs {
    ChatArgs {
        llm: openatrading_core::llm::LlmClient::from_config(None, None),
        tools: Vec::new(),
        system_prompt: String::new(),
    }
}

/// Helper: create a VirtualRuntime, type input, dispatch submit, return screen text.
async fn run_with_input(groups: Vec<(&str, u64)>) -> String {
    let mut runtime = VirtualRuntime::<ChatApp>::new(test_args(), WIDTH, HEIGHT);

    for (text, _delay) in groups {
        if text == "\x1b" {
            runtime.dispatch(Msg::Quit);
        } else if text.contains('\r') || text.contains('\n') {
            for part in text.split('\r') {
                for part2 in part.split('\n') {
                    runtime.type_str(part2);
                }
            }
            runtime.dispatch(Msg::SubmitInput);
        } else {
            runtime.type_str(text);
        }
    }

    runtime.screen()
}

// ── 1. Initialization ────────────────────────────────────────

#[tokio::test]
async fn test_init_shows_hints() {
    let screen = run_with_input(vec![]).await;

    assert!(
        screen.contains("Enter"),
        "hints should appear on init; got: {:?}",
        screen
    );
}

// ── 2. Exit ──────────────────────────────────────────────────

#[tokio::test]
async fn test_quit_immediately() {
    let screen = run_with_input(vec![("\x1b", 0)]).await;
    assert!(!screen.is_empty(), "screen should have content");
}
