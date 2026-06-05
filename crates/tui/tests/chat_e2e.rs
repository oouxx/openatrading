use std::time::Duration;

use ratatui::backend::TestBackend;

/// E2E test: inject inputs, verify the chat loop renders and quits cleanly.
///
/// The LLM is called with a real API key (from environment). Marked ignore
/// so CI won't run it without credentials.
#[tokio::test]
#[ignore]
async fn test_chat_e2e_basic_interaction() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    // Spawn input sender with delays so the event loop has time to process
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(200)).await;
        for c in "say hello".chars() {
            tx.send(c.to_string()).unwrap();
        }
        tx.send("\r".to_string()).unwrap();
        // Wait for LLM to stream a response
        tokio::time::sleep(Duration::from_secs(8)).await;
        tx.send("\x1b".to_string()).unwrap();
    });

    let llm = openatrading_core::llm::LlmClient::from_config(None, None);
    openatrading_tui::chat::run_chat_inner(&mut terminal, &mut rx, llm)
        .await
        .unwrap();

    let buffer = terminal.backend().buffer();
    let text: String = buffer.content().iter().map(|c| c.symbol()).collect();
    assert!(
        text.contains("say hello"),
        "user input should appear in buffer"
    );
}

/// Test that pressing Escape immediately quits without needing LLM.
#[tokio::test]
#[ignore]
async fn test_chat_e2e_quit_immediately() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(200)).await;
        tx.send("\x1b".to_string()).unwrap();
    });

    let llm = openatrading_core::llm::LlmClient::from_config(None, None);
    openatrading_tui::chat::run_chat_inner(&mut terminal, &mut rx, llm)
        .await
        .unwrap();

    let buffer = terminal.backend().buffer();
    let text: String = buffer.content().iter().map(|c| c.symbol()).collect();
    assert!(
        text.contains("AI Chat"),
        "welcome message should be visible"
    );
}

/// Test that submitting empty input does nothing and doesn't crash.
#[tokio::test]
#[ignore]
async fn test_chat_e2e_empty_submit() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(200)).await;
        // Empty submit should be ignored
        tx.send("\r".to_string()).unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;
        // Then type and submit a real message
        tx.send("x".to_string()).unwrap();
        tx.send("\r".to_string()).unwrap();
        tokio::time::sleep(Duration::from_secs(3)).await;
        tx.send("\x1b".to_string()).unwrap();
    });

    let llm = openatrading_core::llm::LlmClient::from_config(None, None);
    openatrading_tui::chat::run_chat_inner(&mut terminal, &mut rx, llm)
        .await
        .unwrap();
}
