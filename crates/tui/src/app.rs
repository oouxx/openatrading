//! Minimal Elm-Architecture framework built on ratatui + crossterm.
//!
//! Provides `App` trait, `Command`, `FocusManager`, terminal runtime,
//! and virtual runtime for testing.

use crossterm::event::Event as CrosstermEvent;
use crossterm::terminal;
use ratatui::Terminal;

// ---------------------------------------------------------------------------
// Command
// ---------------------------------------------------------------------------

/// A side-effect returned from [`App::update`].
pub enum Command<Msg> {
    None,
    Quit,
    Message(Msg),
}

impl<Msg> Command<Msg> {
    pub fn none() -> Self {
        Command::None
    }
    pub fn quit() -> Self {
        Command::Quit
    }
    pub fn message(msg: Msg) -> Self {
        Command::Message(msg)
    }
}

// ---------------------------------------------------------------------------
// App trait
// ---------------------------------------------------------------------------

/// Elm-Architecture application trait.
pub trait App: Sized {
    type State;
    type Message;
    type Args;

    /// Initialise state and return an optional startup command.
    fn init(args: Self::Args) -> (Self::State, Command<Self::Message>);

    /// Pure update: given the current state and a message, mutate state and
    /// return a command.
    fn update(state: &mut Self::State, msg: Self::Message) -> Command<Self::Message>;

    /// Render the current state into the provided frame.
    fn view(state: &Self::State, frame: &mut ratatui::Frame);

    /// Handle events that don't need mutable state (global shortcuts).
    /// Called before `handle_event_with_state`.
    fn handle_event(_event: &CrosstermEvent) -> Option<Self::Message> {
        None
    }

    /// Handle events that may inspect current state.
    fn handle_event_with_state(
        _state: &Self::State,
        _event: &CrosstermEvent,
    ) -> Option<Self::Message> {
        None
    }
}

// ---------------------------------------------------------------------------
// FocusManager
// ---------------------------------------------------------------------------

/// Manages keyboard focus between a fixed set of items.
pub struct FocusManager<T> {
    items: Vec<T>,
    index: usize,
}

impl<T: PartialEq + Eq + std::fmt::Debug> FocusManager<T> {
    pub fn new(items: Vec<T>) -> Self {
        assert!(!items.is_empty(), "FocusManager needs at least one item");
        Self { items, index: 0 }
    }

    pub fn with_initial_focus(items: Vec<T>) -> Self {
        Self::new(items)
    }

    pub fn focus_next(&mut self) {
        if !self.items.is_empty() {
            self.index = (self.index + 1) % self.items.len();
        }
    }

    pub fn focus_prev(&mut self) {
        if !self.items.is_empty() {
            self.index = if self.index == 0 {
                self.items.len() - 1
            } else {
                self.index - 1
            };
        }
    }

    pub fn is_focused(&self, item: &T) -> bool {
        self.items.get(self.index).is_some_and(|i| i == item)
    }

    pub fn focused(&self) -> Option<&T> {
        self.items.get(self.index)
    }
}

// ---------------------------------------------------------------------------
// Terminal runtime
// ---------------------------------------------------------------------------

/// Real-terminal runtime that drives an [`App`] with crossterm input.
pub struct TerminalRuntime<A: App> {
    state: Option<A::State>,
    terminal: Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
    msg_tx: tokio::sync::mpsc::UnboundedSender<A::Message>,
    msg_rx: tokio::sync::mpsc::UnboundedReceiver<A::Message>,
}

impl<A: App> TerminalRuntime<A> {
    pub fn new(args: A::Args) -> anyhow::Result<Self> {
        let (state, _cmd) = A::init(args);

        terminal::enable_raw_mode()?;
        let mut stdout = std::io::stdout();
        crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
        let backend = ratatui::backend::CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        let (msg_tx, msg_rx) = tokio::sync::mpsc::unbounded_channel();

        Ok(Self {
            state: Some(state),
            terminal,
            msg_tx,
            msg_rx,
        })
    }

    pub fn state(&self) -> &A::State {
        self.state.as_ref().unwrap()
    }
    pub fn state_mut(&mut self) -> &mut A::State {
        self.state.as_mut().unwrap()
    }
    pub fn message_sender(&self) -> tokio::sync::mpsc::UnboundedSender<A::Message> {
        self.msg_tx.clone()
    }

    /// Run the main event loop until `Command::Quit` is returned.
    pub async fn run(mut self) -> anyhow::Result<A::State> {
        loop {
            // Render
            let state_ref = self.state.as_ref().unwrap();
            self.terminal.draw(|frame| A::view(state_ref, frame))?;

            // Process internal messages first (from spawned tasks)
            if let Ok(msg) = self.msg_rx.try_recv() {
                let state = self.state.as_mut().unwrap();
                let cmd = A::update(state, msg);
                match cmd {
                    Command::Quit => break,
                    Command::Message(m) => {
                        let _ = self.msg_tx.send(m);
                    },
                    Command::None => {},
                }
                continue;
            }

            // Poll for external events
            if crossterm::event::poll(std::time::Duration::from_millis(50))? {
                let event = crossterm::event::read()?;

                if let Some(msg) = A::handle_event(&event) {
                    let state = self.state.as_mut().unwrap();
                    Self::process_cmd(state, msg, &self.msg_tx);
                    continue;
                }
                let state_ref = self.state.as_ref().unwrap();
                if let Some(msg) = A::handle_event_with_state(state_ref, &event) {
                    let state = self.state.as_mut().unwrap();
                    Self::process_cmd(state, msg, &self.msg_tx);
                }
            }
        }

        // Take state out before drop
        let state = self.state.take().unwrap();
        Ok(state)
    }

    fn process_cmd(
        state: &mut A::State,
        msg: A::Message,
        tx: &tokio::sync::mpsc::UnboundedSender<A::Message>,
    ) {
        let cmd = A::update(state, msg);
        match cmd {
            Command::Quit => {},
            Command::Message(m) => {
                let _ = tx.send(m);
            },
            Command::None => {},
        }
    }
}

impl<A: App> Drop for TerminalRuntime<A> {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
        let _ = crossterm::execute!(std::io::stdout(), crossterm::terminal::LeaveAlternateScreen);
    }
}

// ---------------------------------------------------------------------------
// Virtual runtime (for testing)
// ---------------------------------------------------------------------------

/// Virtual-terminal runtime using ratatui's `TestBackend`.
///
/// Supports programmatic event dispatch and screen capture.
pub struct VirtualRuntime<A: App> {
    state: A::State,
    terminal: Terminal<ratatui::backend::TestBackend>,
    msg_tx: tokio::sync::mpsc::UnboundedSender<A::Message>,
    msg_rx: tokio::sync::mpsc::UnboundedReceiver<A::Message>,
}

impl<A: App> VirtualRuntime<A> {
    pub fn new(args: A::Args, width: u16, height: u16) -> Self {
        let (state, _cmd) = A::init(args);
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

    pub fn state(&self) -> &A::State {
        &self.state
    }
    pub fn state_mut(&mut self) -> &mut A::State {
        &mut self.state
    }
    pub fn message_sender(&self) -> tokio::sync::mpsc::UnboundedSender<A::Message> {
        self.msg_tx.clone()
    }

    /// Dispatch a message into the update loop.
    pub fn dispatch(&mut self, msg: A::Message) {
        let cmd = A::update(&mut self.state, msg);
        if let Command::Message(m) = cmd {
            let _ = self.msg_tx.send(m);
        }
    }

    /// Drain all pending messages from the internal channel, simulating
    /// the chain-processing that `TerminalRuntime::run` does between ticks.
    ///
    /// Call after `dispatch` (or a series of dispatches) to ensure every
    /// `Command::Message(m)` that was returned by `A::update` is consumed
    /// and its downstream messages are also processed.
    pub fn drain(&mut self) {
        while let Ok(msg) = self.msg_rx.try_recv() {
            let cmd = A::update(&mut self.state, msg);
            if let Command::Message(m) = cmd {
                let _ = self.msg_tx.send(m);
            }
        }
    }

    /// Render the current state and return the screen content as a string.
    pub fn screen(&mut self) -> String {
        self.terminal.draw(|frame| A::view(&self.state, frame)).ok();
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
            if let Some(msg) = A::handle_event_with_state(&self.state, &event) {
                self.dispatch(msg);
            }
        }
    }
}
