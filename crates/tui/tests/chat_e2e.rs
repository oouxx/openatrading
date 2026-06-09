use envision::harness::AppHarness;

use openatrading_tui::chat::{ChatApp, Msg};

const WIDTH: u16 = 80;
const HEIGHT: u16 = 24;

/// Helper: create an AppHarness, type input, render, return screen text.
async fn run_with_input(groups: Vec<(&str, u64)>) -> String {
    let mut harness = AppHarness::<ChatApp>::new(WIDTH, HEIGHT).unwrap();

    for (text, _delay) in groups {
        if text == "\x1b" {
            harness.dispatch(Msg::Quit);
        } else if text.contains('\r') || text.contains('\n') {
            // Type the text then submit
            for part in text.split('\r') {
                for part2 in part.split('\n') {
                    harness.type_str(part2);
                }
            }
            harness.dispatch(Msg::SubmitInput);
        } else {
            harness.type_str(text);
        }
    }

    harness.render().unwrap();
    harness.screen()
}

// ── 1. Initialization ────────────────────────────────────────

#[tokio::test]
async fn test_init_shows_welcome() {
    let screen = run_with_input(vec![]).await;

    assert!(
        screen.contains("AI Chat"),
        "welcome message should appear on init"
    );
}

// ── 2. Exit ──────────────────────────────────────────────────

#[tokio::test]
async fn test_quit_immediately() {
    let screen = run_with_input(vec![("\x1b", 0)]).await;
    assert!(screen.contains("AI Chat"), "at least the title is there");
}
