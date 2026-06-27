//! Conversation view widget — renders a chat conversation with markdown support.
//!
//! Codex/Claude Code style: no borders, `>` prefix for user messages,
//! markdown rendering for AI responses.

use std::sync::Arc;

use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState};
use ratatui::Frame;

use ratatui_markdown::highlight::{HighlightHooks, TreeSitterHighlighter};
use ratatui_markdown::markdown::MarkdownRenderer;
use ratatui_markdown::ThemeConfig;

// ---------------------------------------------------------------------------
// Message block types
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub enum MessageBlock {
    Text(String),
    Error(String),
}

impl MessageBlock {
    pub fn text(text: &str) -> Self {
        MessageBlock::Text(text.to_string())
    }
    pub fn error(text: &str) -> Self {
        MessageBlock::Error(text.to_string())
    }
}

// ---------------------------------------------------------------------------
// Conversation messages
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConversationRole {
    User,
    Assistant,
}

#[derive(Clone, Debug)]
pub struct ConversationMessage {
    role: ConversationRole,
    blocks: Vec<MessageBlock>,
    streaming: bool,
}

impl ConversationMessage {
    pub fn new(role: ConversationRole) -> Self {
        Self {
            role,
            blocks: Vec::new(),
            streaming: false,
        }
    }

    pub fn role(&self) -> &ConversationRole {
        &self.role
    }

    pub fn set_blocks(&mut self, blocks: Vec<MessageBlock>) {
        self.blocks = blocks;
    }

    pub fn set_streaming(&mut self, streaming: bool) {
        self.streaming = streaming;
    }

    pub fn text_content(&self) -> String {
        self.blocks
            .iter()
            .filter_map(|b| match b {
                MessageBlock::Text(t) => Some(t.as_str()),
                MessageBlock::Error(_) => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn is_streaming(&self) -> bool {
        self.streaming
    }
}

// ---------------------------------------------------------------------------
// ConversationViewState
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct ConversationViewState {
    messages: Vec<ConversationMessage>,
    scroll_offset: usize,
    auto_scroll: bool,
    /// Cached rendered lines — lazily rebuilt when dirty.
    pub(super) cache: Cache,
}

// Manual Clone + Debug for Cache since MarkdownRenderer/ThemeConfig don't impl them
pub(super) struct Cache {
    lines: Vec<Line<'static>>,
    dirty: bool,
    renderer: MarkdownRenderer,
    theme: ThemeConfig,
}

impl Clone for Cache {
    fn clone(&self) -> Self {
        Self {
            lines: self.lines.clone(),
            dirty: true,
            renderer: MarkdownRenderer::new(80),
            theme: self.theme,
        }
    }
}

impl std::fmt::Debug for Cache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cache")
            .field("lines", &self.lines.len())
            .field("dirty", &self.dirty)
            .finish()
    }
}

impl Default for ConversationViewState {
    fn default() -> Self {
        Self::new()
    }
}

impl ConversationViewState {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            scroll_offset: 0,
            auto_scroll: true,
            cache: Cache {
                lines: Vec::new(),
                dirty: true,
                renderer: MarkdownRenderer::new(80),
                theme: ThemeConfig::default(),
            },
        }
    }

    /// Handle scroll messages.
    pub fn handle_scroll(&mut self, msg: &ConversationViewMessage) {
        match msg {
            ConversationViewMessage::ScrollUp => {
                self.auto_scroll = false;
                self.scroll_offset = self.scroll_offset.saturating_sub(3);
            },
            ConversationViewMessage::ScrollDown => {
                self.scroll_offset = self.scroll_offset.saturating_add(3);
                self.auto_scroll = false;
            },
            ConversationViewMessage::ScrollToBottom => {
                self.auto_scroll = true;
            },
        }
    }

    pub fn push_user(&mut self, text: &str) {
        let mut msg = ConversationMessage::new(ConversationRole::User);
        msg.set_blocks(vec![MessageBlock::text(text)]);
        self.messages.push(msg);
        self.mark_dirty();
        self.auto_scroll = true;
    }

    /// Return a handle (index) to the new assistant message.
    pub fn push_assistant(&mut self, text: &str) -> usize {
        let mut msg = ConversationMessage::new(ConversationRole::Assistant);
        msg.set_blocks(vec![MessageBlock::text(text)]);
        self.messages.push(msg);
        self.mark_dirty();
        self.auto_scroll = true;
        self.messages.len() - 1
    }

    pub fn update_by_handle<F>(&mut self, idx: usize, f: F)
    where
        F: FnOnce(&mut ConversationMessage),
    {
        if let Some(msg) = self.messages.get_mut(idx) {
            f(msg);
            self.mark_dirty();
        }
    }

    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    pub fn messages(&self) -> &[ConversationMessage] {
        &self.messages
    }

    pub fn auto_scroll(&self) -> bool {
        self.auto_scroll
    }

    fn mark_dirty(&mut self) {
        self.cache.dirty = true;
    }

    /// Rebuild the cached rendered lines from all messages.
    fn rebuild_cache(&mut self, width: usize) {
        let content_width = width.saturating_sub(2); // 2 chars for `> ` prefix or indent
        let highlighter = Arc::new(TreeSitterHighlighter::new());
        let hooks = HighlightHooks::new(highlighter, content_width);
        self.cache.renderer =
            MarkdownRenderer::new(content_width).with_render_hooks(Box::new(hooks));
        let mut lines: Vec<Line<'static>> = Vec::new();

        for msg in &self.messages {
            match msg.role() {
                ConversationRole::User => {
                    // User messages: render with `> ` prefix (Claude Code style)
                    for block in &msg.blocks {
                        if let MessageBlock::Text(text) = block {
                            for line_text in text.lines() {
                                if line_text.is_empty() {
                                    lines.push(Line::from(Span::styled(
                                        ">",
                                        Style::default().fg(Color::Cyan),
                                    )));
                                } else {
                                    lines.push(Line::from(Span::styled(
                                        format!("> {}", line_text),
                                        Style::default().fg(Color::Cyan),
                                    )));
                                }
                            }
                        }
                    }
                },
                ConversationRole::Assistant => {
                    // AI messages: render markdown directly
                    for block in &msg.blocks {
                        match block {
                            MessageBlock::Text(text) => {
                                if text.is_empty() && msg.is_streaming() {
                                    lines.push(Line::from(Span::styled(
                                        "▊",
                                        Style::default().fg(Color::Green),
                                    )));
                                } else if !text.is_empty() {
                                    let blocks = self.cache.renderer.parse(text);
                                    let rendered =
                                        self.cache.renderer.render(&blocks, &self.cache.theme);
                                    lines.extend(rendered);
                                }
                            },
                            MessageBlock::Error(text) => {
                                lines.push(Line::from(Span::styled(
                                    format!("Error: {}", text),
                                    Style::default().fg(Color::Red),
                                )));
                            },
                        }
                    }

                    // Streaming cursor
                    if msg.is_streaming() {
                        if let Some(last) = lines.last_mut() {
                            let spans = std::mem::take(&mut last.spans);
                            let mut owned: Vec<Span<'static>> = spans.to_owned();
                            owned.push(Span::styled("▊", Style::default().fg(Color::Green)));
                            *last = Line::from(owned);
                        }
                    }
                },
            }

            // Blank line between messages
            lines.push(Line::from(""));
        }

        self.cache.lines = lines;
        self.cache.dirty = false;
    }

    /// Ensure the cached rendered lines are up to date.
    pub fn ensure_cache(&mut self, width: usize) {
        if self.cache.dirty {
            self.rebuild_cache(width);
        }
    }
}

// ---------------------------------------------------------------------------
// Messages for the parent App
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub enum ConversationViewMessage {
    ScrollUp,
    ScrollDown,
    ScrollToBottom,
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

/// Render the conversation view inside the given area (no borders).
pub fn render_conversation(
    state: &mut ConversationViewState,
    frame: &mut Frame,
    area: Rect,
    _focused: bool,
) {
    let width = area.width as usize;
    let vis_height = area.height as usize;

    // Ensure cache is built
    state.ensure_cache(width);
    let total = state.cache.lines.len();
    let max_offset = total.saturating_sub(vis_height);

    if state.auto_scroll {
        state.scroll_offset = max_offset;
    }
    state.scroll_offset = state.scroll_offset.min(max_offset);

    // Render visible lines
    let visible: Vec<Line<'static>> = state
        .cache
        .lines
        .iter()
        .skip(state.scroll_offset)
        .take(vis_height)
        .cloned()
        .collect();

    let paragraph = Paragraph::new(visible);
    frame.render_widget(paragraph, area);

    // Scrollbar (right edge)
    if total > vis_height {
        let mut scrollbar_state =
            ScrollbarState::new(total.saturating_sub(1)).position(state.scroll_offset);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None),
            area,
            &mut scrollbar_state,
        );
    }
}
