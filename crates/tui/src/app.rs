use crossterm::event::Event as CrosstermEvent;
use crossterm::terminal;
use ratatui::Terminal;

use crate::chat;

/// Returns `true` when the caller should quit the event loop.
fn apply_msg(
    state: &mut chat::AppState,
    msg: chat::Msg,
    tx: &tokio::sync::mpsc::UnboundedSender<chat::Msg>,
) -> bool {
    match chat::update(state, msg) {
        None => false,
        Some(chat::Msg::Quit) => true,
        Some(m) => {
            let _ = tx.send(m);
            false
        },
    }
}

// ---------------------------------------------------------------------------
// Real-terminal runtime
// ---------------------------------------------------------------------------

/// Real-terminal runtime for the TUI chat application.
pub struct ChatRuntime {
    state: chat::AppState,
    terminal: Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
    msg_tx: tokio::sync::mpsc::UnboundedSender<chat::Msg>,
    msg_rx: tokio::sync::mpsc::UnboundedReceiver<chat::Msg>,
}

impl ChatRuntime {
    pub fn new(args: chat::ChatArgs) -> anyhow::Result<Self> {
        let state = chat::init(args);

        terminal::enable_raw_mode()?;
        let mut stdout = std::io::stdout();
        crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
        let backend = ratatui::backend::CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        let (msg_tx, msg_rx) = tokio::sync::mpsc::unbounded_channel();

        Ok(Self {
            state,
            terminal,
            msg_tx,
            msg_rx,
        })
    }

    pub fn state(&self) -> &chat::AppState {
        &self.state
    }
    pub fn state_mut(&mut self) -> &mut chat::AppState {
        &mut self.state
    }
    pub fn message_sender(&self) -> tokio::sync::mpsc::UnboundedSender<chat::Msg> {
        self.msg_tx.clone()
    }

    /// Run the main event loop until `Msg::Quit` is returned.
    pub async fn run(mut self) -> anyhow::Result<()> {
        loop {
            // Render
            self.terminal.draw(|frame| chat::view(&self.state, frame))?;

            // Process internal messages first (from spawned tasks)
            if let Ok(msg) = self.msg_rx.try_recv() {
                if apply_msg(&mut self.state, msg, &self.msg_tx) {
                    break;
                }
                continue;
            }

            // Poll for external events
            if crossterm::event::poll(std::time::Duration::from_millis(50))? {
                let event = crossterm::event::read()?;

                if let Some(msg) = chat::handle_event(&event) {
                    if apply_msg(&mut self.state, msg, &self.msg_tx) {
                        break;
                    }
                    continue;
                }
                if let Some(msg) = chat::handle_event_with_state(&self.state, &event) {
                    if apply_msg(&mut self.state, msg, &self.msg_tx) {
                        break;
                    }
                }
            }
        }

        Ok(())
    }
}

impl Drop for ChatRuntime {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
        let _ = crossterm::execute!(std::io::stdout(), crossterm::terminal::LeaveAlternateScreen);
    }
}

// ---------------------------------------------------------------------------
// Virtual runtime (for testing)
// ---------------------------------------------------------------------------

/// Virtual-terminal runtime using ratatui's `TestBackend`.
pub struct VirtualChatRuntime {
    state: chat::AppState,
    terminal: Terminal<ratatui::backend::TestBackend>,
    msg_tx: tokio::sync::mpsc::UnboundedSender<chat::Msg>,
    msg_rx: tokio::sync::mpsc::UnboundedReceiver<chat::Msg>,
}

impl VirtualChatRuntime {
    pub fn new(args: chat::ChatArgs, width: u16, height: u16) -> Self {
        let state = chat::init(args);
        let backend = ratatui::backend::TestBackend::new(width, height);
        let terminal = Terminal::new(backend).expect("TestBackend never fails");
        let (msg_tx, msg_rx) = tokio::sync::mpsc::unbounded_channel();
        Self {
            state,
            terminal,
            msg_tx,
            msg_rx,
        }
    }

    pub fn state(&self) -> &chat::AppState {
        &self.state
    }
    pub fn state_mut(&mut self) -> &mut chat::AppState {
        &mut self.state
    }
    pub fn message_sender(&self) -> tokio::sync::mpsc::UnboundedSender<chat::Msg> {
        self.msg_tx.clone()
    }

    /// Dispatch a message into the update loop.
    pub fn dispatch(&mut self, msg: chat::Msg) {
        if let Some(m) = chat::update(&mut self.state, msg) {
            let _ = self.msg_tx.send(m);
        }
    }

    /// Drain all pending messages from the internal channel.
    pub fn drain(&mut self) {
        while let Ok(msg) = self.msg_rx.try_recv() {
            if let Some(m) = chat::update(&mut self.state, msg) {
                let _ = self.msg_tx.send(m);
            }
        }
    }

    /// Render the current state and return the screen content as a string.
    pub fn screen(&mut self) -> String {
        self.terminal
            .draw(|frame| chat::view(&self.state, frame))
            .ok();
        self.terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>()
    }

    /// Type a string character by character (each as a `KeyEvent`).
    pub fn type_str(&mut self, text: &str) {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        for ch in text.chars() {
            let event = CrosstermEvent::Key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
            if let Some(msg) = chat::handle_event_with_state(&self.state, &event) {
                self.dispatch(msg);
            }
        }
    }
}
