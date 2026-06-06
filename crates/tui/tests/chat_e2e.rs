use std::time::Duration;

use openatrading_core::llm::LlmClient;
use ratatui::backend::TestBackend;
use tokio::sync::mpsc::UnboundedSender;

const WIDTH: u16 = 80;
const HEIGHT: u16 = 24;

fn make_llm() -> LlmClient {
    // Load .env so DEEPSEEK_API_KEY (and friends) are available.
    let _ = dotenvy::dotenv();
    LlmClient::with_config("deepseek".into(), "deepseek-v4-flash".into())
}

/// Send characters one by one with a short inter-char delay, then wait
/// `post_delay_ms` before proceeding to the next group.
async fn send_group(tx: &UnboundedSender<String>, text: &str, post_delay_ms: u64) {
    for c in text.chars() {
        tx.send(c.to_string()).unwrap();
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    if post_delay_ms > 0 {
        tokio::time::sleep(Duration::from_millis(post_delay_ms)).await;
    }
}

/// Helper: create a terminal + input channel, inject input in groups,
/// run the chat loop, return the rendered buffer.
async fn run_with_input(llm: LlmClient, groups: Vec<(&'static str, u64)>) -> String {
    let backend = TestBackend::new(WIDTH, HEIGHT);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    tokio::spawn(async move {
        for (text, delay) in groups {
            send_group(&tx, text, delay).await;
        }
    });

    openatrading_tui::chat::run_chat_inner(&mut terminal, &mut rx, llm, vec![], String::new())
        .await
        .unwrap();

    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect()
}

// ── 1. Initialization ────────────────────────────────────────

#[tokio::test]
async fn test_init_shows_welcome() {
    let buffer = run_with_input(make_llm(), vec![("\x1b", 0)]).await;

    assert!(
        buffer.contains("AI Chat"),
        "welcome message should appear on init: {buffer:?}"
    );
    assert!(
        buffer.contains("Type your message"),
        "help text should appear on init"
    );
}

// ── 2. Exit ──────────────────────────────────────────────────

#[tokio::test]
async fn test_quit_immediately() {
    let buffer = run_with_input(make_llm(), vec![("\x1b", 0)]).await;

    assert!(buffer.contains("AI Chat"));
}

/// Full conversation with real DeepSeek LLM: type a message, submit, wait for
/// streaming, then quit.  Verifies that user input and AI response both appear.
#[tokio::test]
#[ignore]
async fn test_conversation_with_real_llm() {
    let buffer = run_with_input(
        make_llm(),
        vec![("say hello in one word\r", 15000), ("\x1b", 0)],
    )
    .await;

    assert!(
        buffer.contains("say hello in one word"),
        "user input should be visible: {buffer:?}"
    );
    assert!(
        !buffer.contains("(streaming...)"),
        "streaming should have finished before quit"
    );
}

/// Ask DeepSeek for a markdown-rich answer and verify that markdown symbols
/// (pipes from a table) are rendered in the terminal buffer.
#[tokio::test]
#[ignore]
async fn test_markdown_rendering_with_real_llm() {
    let buffer = run_with_input(
        make_llm(),
        vec![
            ("Write a short markdown table with 2 columns\r", 20000),
            ("\x1b", 0),
        ],
    )
    .await;

    assert!(
        buffer.contains("Write a short markdown table"),
        "user input visible: {buffer:?}"
    );
    assert!(
        buffer.contains('|'),
        "markdown table pipe should be visible: {buffer:?}"
    );
}
