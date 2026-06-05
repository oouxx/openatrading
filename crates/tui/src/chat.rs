use std::sync::Arc;
use std::sync::Mutex;

use futures::StreamExt;
use openatrading_core::llm::ChatStreamProvider;
use pi_tui::keybindings::init_keybindings;
use pi_tui::Terminal;
use ratatui::prelude::*;
use ratatui::widgets::*;

pub struct ChatState {
    pub display_messages: Vec<(String, String)>,
    pub input: String,
    pub streaming: bool,
    pub stream_buffer: String,
    pub should_quit: bool,
    llm_messages: Vec<serde_json::Value>,
}

impl Default for ChatState {
    fn default() -> Self {
        Self::new()
    }
}

impl ChatState {
    pub fn new() -> Self {
        Self {
            display_messages: vec![(
                "system".into(),
                "AI Chat — Type your message and press Enter. Press Esc to quit.".into(),
            )],
            llm_messages: vec![
                serde_json::json!({"role": "system", "content": "You are a helpful AI trading assistant. You can help with market analysis and trading questions."}),
            ],
            input: String::new(),
            streaming: false,
            stream_buffer: String::new(),
            should_quit: false,
        }
    }

    /// Process a raw input event. Returns the submitted text on Enter, None otherwise.
    pub fn handle_input(&mut self, data: &str) -> Option<String> {
        match data {
            "\r" | "\n" => {
                let text = std::mem::take(&mut self.input);
                if text.is_empty() {
                    return None;
                }
                self.display_messages.push(("user".into(), text.clone()));
                self.llm_messages
                    .push(serde_json::json!({"role": "user", "content": text}));
                self.streaming = true;
                self.stream_buffer = String::new();
                Some(text)
            },
            "\x1b" => {
                self.should_quit = true;
                None
            },
            "\x7f" => {
                self.input.pop();
                None
            },
            c if !c.is_empty() && c.chars().count() == 1 => {
                self.input.push_str(c);
                None
            },
            _ => None,
        }
    }

    /// Build the chat display text for rendering.
    pub fn format_display(&self) -> String {
        let mut display = String::new();
        for (role, content) in &self.display_messages {
            let prefix = match role.as_str() {
                "user" => "\n[You]\n",
                "system" => "",
                _ => "\n[AI]\n",
            };
            display.push_str(prefix);
            display.push_str(content);
            display.push('\n');
        }
        if self.streaming && !self.stream_buffer.is_empty() {
            display.push_str("\n[AI]\n");
            display.push_str(&self.stream_buffer);
        }
        display
    }

    pub fn llm_messages_json(&self) -> String {
        serde_json::to_string(&self.llm_messages).unwrap_or_default()
    }

    pub fn push_assistant_response(&mut self, response: String) {
        self.llm_messages
            .push(serde_json::json!({"role": "assistant", "content": response}));
        self.display_messages.push(("assistant".into(), response));
        self.stream_buffer = String::new();
        self.streaming = false;
    }

    pub fn push_assistant_error(&mut self, err: String) {
        self.display_messages.push(("assistant".into(), err));
        self.stream_buffer = String::new();
        self.streaming = false;
    }
}

/// Spawn a background task that calls the LLM and updates shared state.
fn spawn_llm_task<P: ChatStreamProvider>(llm: P, state: Arc<Mutex<ChatState>>, msgs_json: String) {
    tokio::spawn(async move {
        match llm.chat_stream_with_messages(&msgs_json).await {
            Ok(mut stream) => {
                let mut response = String::new();
                while let Some(chunk) = stream.next().await {
                    match chunk {
                        Ok(delta) => {
                            response.push_str(&delta);
                            state.lock().unwrap().stream_buffer = response.clone();
                        },
                        Err(e) => {
                            response.push_str(&format!("\n[Error: {}]", e));
                            state.lock().unwrap().stream_buffer = response.clone();
                            break;
                        },
                    }
                }
                state.lock().unwrap().push_assistant_response(response);
            },
            Err(e) => {
                state
                    .lock()
                    .unwrap()
                    .push_assistant_error(format!("Error: {}", e));
            },
        }
    });
}

fn key_event_to_string(key: &crossterm::event::KeyEvent) -> String {
    use crossterm::event::{KeyCode::*, KeyModifiers};
    match key.code {
        Char(c) => {
            if key.modifiers.contains(KeyModifiers::CONTROL) && c.is_ascii_alphabetic() {
                (c.to_ascii_lowercase() as u8 - b'a' + 1).to_string()
            } else if key.modifiers.contains(KeyModifiers::ALT) {
                format!("\x1b{}", c)
            } else {
                c.to_string()
            }
        },
        Enter => "\r".to_string(),
        Tab => "\t".to_string(),
        Backspace => "\x7f".to_string(),
        Esc => "\x1b".to_string(),
        Delete => "\x1b[3~".to_string(),
        Up => "\x1b[A".to_string(),
        Down => "\x1b[B".to_string(),
        Left => "\x1b[D".to_string(),
        Right => "\x1b[C".to_string(),
        Home => "\x1b[H".to_string(),
        End => "\x1b[F".to_string(),
        PageUp => "\x1b[5~".to_string(),
        PageDown => "\x1b[6~".to_string(),
        Insert => "\x1b[2~".to_string(),
        F(n) => match n {
            1 => "\x1bOP".to_string(),
            2 => "\x1bOQ".to_string(),
            3 => "\x1bOR".to_string(),
            4 => "\x1bOS".to_string(),
            n if n <= 12 => format!("\x1b[{}~", 10 + n),
            _ => String::new(),
        },
        _ => String::new(),
    }
}

/// Inner chat loop decoupled from terminal setup, testable with any Backend.
pub async fn run_chat_inner<B: ratatui::backend::Backend, P: ChatStreamProvider>(
    ratatui_term: &mut ratatui::Terminal<B>,
    input_rx: &mut tokio::sync::mpsc::UnboundedReceiver<String>,
    llm: P,
) -> anyhow::Result<()> {
    let state = Arc::new(Mutex::new(ChatState::new()));
    let mut render_ticker = tokio::time::interval(tokio::time::Duration::from_millis(33));

    loop {
        if state.lock().unwrap().should_quit {
            break;
        }

        tokio::select! {
            _ = render_ticker.tick() => {}
            Some(data) = input_rx.recv() => {
                let mut st = state.lock().unwrap();
                if !st.streaming
                    && st.handle_input(&data).is_some() {
                        let msgs_json = st.llm_messages_json();
                        drop(st);
                        spawn_llm_task(llm.clone(), state.clone(), msgs_json);
                    }
            }
        }

        ratatui_term
            .draw(|frame| {
                let area = frame.area();
                let chunks =
                    Layout::vertical([Constraint::Min(1), Constraint::Length(3)]).split(area);
                let (chat_area, input_area) = (chunks[0], chunks[1]);

                let st = state.lock().unwrap();
                let display = st.format_display();

                let chat = Paragraph::new(display)
                    .wrap(Wrap { trim: false })
                    .style(Style::default());
                frame.render_widget(chat, chat_area);

                let input_suffix = if st.streaming { " (streaming...)" } else { "" };
                let input_style = if st.streaming {
                    Style::default().fg(Color::DarkGray)
                } else {
                    Style::default().fg(Color::Cyan)
                };
                let input_widget =
                    Paragraph::new(format!("❯ {}{}", st.input, input_suffix)).style(input_style);
                frame.render_widget(input_widget, input_area);

                if !st.streaming {
                    let cursor_x = 2u16
                        .saturating_add(st.input.len() as u16)
                        .min(area.width.saturating_sub(1));
                    frame.set_cursor_position((cursor_x, area.height.saturating_sub(1)));
                }
            })
            .map_err(|e| anyhow::anyhow!("render: {:?}", e))?;
    }

    Ok(())
}

pub async fn run_chat() -> anyhow::Result<()> {
    init_keybindings(None);

    let llm = openatrading_core::llm::LlmClient::from_config(None, None);

    let mut terminal = Terminal::new()?;
    let (key_rx, shutdown_guard) = terminal.start()?;

    // Clear the terminal so the TUI starts on a fresh screen.
    terminal.clear()?;

    // pi-tui v1.78.7 emits KeyEvent; bridge to String for the generic inner loop.
    let (input_tx, mut input_rx) = tokio::sync::mpsc::unbounded_channel();
    tokio::spawn(async move {
        let mut key_rx = key_rx;
        while let Some(key) = key_rx.recv().await {
            let s = key_event_to_string(&key);
            if !s.is_empty() {
                let _ = input_tx.send(s);
            }
        }
    });

    let ratatui_term = terminal.ratatui_terminal();
    let result = run_chat_inner(ratatui_term, &mut input_rx, llm).await;

    shutdown_guard.shutdown();
    let _ = crossterm::execute!(
        std::io::stdout(),
        crossterm::terminal::Clear(crossterm::terminal::ClearType::All),
        crossterm::cursor::MoveTo(0, 0),
    );

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── ChatState unit tests ─────────────────────────────────

    #[test]
    fn test_chat_state_initial_state() {
        let state = ChatState::new();
        assert!(!state.should_quit);
        assert!(!state.streaming);
        assert!(state.input.is_empty());
        assert_eq!(state.display_messages.len(), 1);
    }

    #[test]
    fn test_handle_input_character() {
        let mut state = ChatState::new();
        assert!(state.handle_input("a").is_none());
        assert!(state.handle_input("b").is_none());
        assert_eq!(state.input, "ab");
    }

    #[test]
    fn test_handle_input_backspace() {
        let mut state = ChatState::new();
        state.handle_input("h");
        state.handle_input("i");
        state.handle_input("\x7f");
        assert_eq!(state.input, "h");
    }

    #[test]
    fn test_handle_input_submit() {
        let mut state = ChatState::new();
        state.handle_input("h");
        let result = state.handle_input("\r");
        assert_eq!(result, Some("h".to_string()));
        assert!(state.streaming);
        assert_eq!(state.display_messages.last().unwrap().0, "user");
        assert_eq!(state.display_messages.last().unwrap().1, "h");
    }

    #[test]
    fn test_handle_input_submit_empty() {
        let mut state = ChatState::new();
        let result = state.handle_input("\r");
        assert!(result.is_none());
        assert!(!state.streaming);
    }

    #[test]
    fn test_handle_input_quit() {
        let mut state = ChatState::new();
        assert!(state.handle_input("\x1b").is_none());
        assert!(state.should_quit);
    }

    #[test]
    fn test_push_assistant_response() {
        let mut state = ChatState::new();
        state.streaming = true;
        state.push_assistant_response("Hello!".into());
        assert!(!state.streaming);
        assert_eq!(state.display_messages.last().unwrap().1, "Hello!");
    }

    #[test]
    fn test_push_assistant_error() {
        let mut state = ChatState::new();
        state.streaming = true;
        state.push_assistant_error("Error: timeout".into());
        assert!(!state.streaming);
        assert_eq!(state.display_messages.last().unwrap().1, "Error: timeout");
    }

    #[test]
    fn test_format_display_system_only() {
        let state = ChatState::new();
        let text = state.format_display();
        assert!(text.contains("AI Chat"));
        assert!(!text.contains("[You]"));
    }

    #[test]
    fn test_format_display_with_messages() {
        let mut state = ChatState::new();
        state.display_messages.push(("user".into(), "hello".into()));
        state
            .display_messages
            .push(("assistant".into(), "world".into()));
        let text = state.format_display();
        assert!(text.contains("[You]"));
        assert!(text.contains("hello"));
        assert!(text.contains("[AI]"));
        assert!(text.contains("world"));
    }

    #[test]
    fn test_format_display_streaming() {
        let mut state = ChatState::new();
        state.streaming = true;
        state.stream_buffer = "partial response".into();
        let text = state.format_display();
        assert!(text.contains("partial response"));
    }

    #[test]
    fn test_llm_messages_json() {
        let state = ChatState::new();
        let json = state.llm_messages_json();
        assert!(json.contains("system"));
        assert!(json.contains("trading assistant"));
    }

    #[test]
    fn test_handle_input_multi_byte() {
        let mut state = ChatState::new();
        state.handle_input("你");
        state.handle_input("好");
        assert_eq!(state.input, "你好");
        let result = state.handle_input("\r");
        assert_eq!(result, Some("你好".to_string()));
    }

    #[test]
    fn test_submit_clears_input() {
        let mut state = ChatState::new();
        for c in "test".chars() {
            state.handle_input(&c.to_string());
        }
        assert!(state.handle_input("\r").is_some());
        assert!(state.input.is_empty());
    }

    #[test]
    fn test_handle_input_streaming_flag_does_not_block_input() {
        let mut state = ChatState::new();
        state.streaming = true;
        // handle_input itself does NOT check streaming — that guard is in run_chat_inner.
        // Character input still works during streaming.
        assert!(state.handle_input("a").is_none());
        assert_eq!(state.input, "a");
        // Quit still works during streaming via handle_input.
        assert!(state.handle_input("\x1b").is_none());
        assert!(state.should_quit);
    }

    // ── render helper tests ──────────────────────────────────

    #[test]
    fn test_render_smoke() {
        use ratatui::backend::TestBackend;
        let backend = TestBackend::new(80, 24);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let state = Arc::new(Mutex::new(ChatState::new()));

        terminal
            .draw(|frame| {
                let area = frame.area();
                let chunks =
                    Layout::vertical([Constraint::Min(1), Constraint::Length(3)]).split(area);
                let (chat_area, input_area) = (chunks[0], chunks[1]);
                let st = state.lock().unwrap();
                let display = st.format_display();
                let chat = Paragraph::new(display)
                    .wrap(Wrap { trim: false })
                    .style(Style::default());
                frame.render_widget(chat, chat_area);
                let input_widget = Paragraph::new(format!("❯ {}", st.input))
                    .style(Style::default().fg(Color::Cyan));
                frame.render_widget(input_widget, input_area);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let cell = buffer.cell((0, 0)).unwrap();
        // Just verify rendering didn't panic and produced some output
        assert!(!cell.symbol().is_empty());
    }
}
