//! A simple text input widget — Codex/Claude Code `> ` prompt style.
//!
//! Supports CJK characters with correct cursor positioning via `unicode-width`.

use ratatui::layout::Rect;
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use unicode_width::UnicodeWidthChar;

/// A single-line text input with `> ` prompt style.
#[derive(Clone, Debug)]
pub struct TextInput {
    buffer: String,
    /// Cursor position as **byte index** into buffer (not char index).
    cursor: usize,
    placeholder: String,
}

impl TextInput {
    pub fn new(placeholder: &str) -> Self {
        Self {
            buffer: String::new(),
            cursor: 0,
            placeholder: placeholder.to_string(),
        }
    }

    /// Get the current input value.
    pub fn value(&self) -> String {
        self.buffer.clone()
    }

    /// Clear the input buffer.
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.cursor = 0;
    }

    /// Insert a character at cursor position.
    pub fn insert_char(&mut self, c: char) {
        self.buffer.insert(self.cursor, c);
        self.cursor += c.len_utf8();
    }

    /// Delete the character before the cursor (backspace) — handles multi-byte CJK.
    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            let s = &self.buffer[..self.cursor];
            let ch = s.chars().next_back().unwrap();
            self.cursor -= ch.len_utf8();
            self.buffer.remove(self.cursor);
        }
    }

    /// Delete the character at the cursor (delete) — handles multi-byte CJK.
    pub fn delete(&mut self) {
        if self.cursor < self.buffer.len() {
            self.buffer.remove(self.cursor);
        }
    }

    /// Move cursor left by one grapheme — handles multi-byte CJK.
    pub fn cursor_left(&mut self) {
        if self.cursor > 0 {
            let s = &self.buffer[..self.cursor];
            let ch = s.chars().next_back().unwrap();
            self.cursor -= ch.len_utf8();
        }
    }

    /// Move cursor right by one grapheme — handles multi-byte CJK.
    pub fn cursor_right(&mut self) {
        if self.cursor < self.buffer.len() {
            let s = &self.buffer[self.cursor..];
            let ch = s.chars().next().unwrap();
            self.cursor += ch.len_utf8();
        }
    }

    /// Move cursor to the beginning.
    pub fn home(&mut self) {
        self.cursor = 0;
    }

    /// Move cursor to the end.
    pub fn end(&mut self) {
        self.cursor = self.buffer.len();
    }

    /// Visual column width of text before cursor (accounts for CJK).
    fn visual_cursor_col(&self) -> usize {
        self.buffer[..self.cursor]
            .chars()
            .map(|c| UnicodeWidthChar::width(c).unwrap_or(0))
            .sum()
    }

    /// Visual column width of the full buffer.
    fn visual_width(&self) -> usize {
        self.buffer
            .chars()
            .map(|c| UnicodeWidthChar::width(c).unwrap_or(0))
            .sum()
    }

    /// Render the text input as a single line with `> ` prompt.
    ///
    /// Handles CJK: correct cursor column positioning, multi-byte chars display.
    pub fn render(&self, frame: &mut Frame, area: Rect, focused: bool) {
        let prefix_style = Style::default().fg(Color::Yellow);
        let max_cols = area.width as usize;
        let avail = max_cols.saturating_sub(2); // `> ` prefix

        if avail == 0 {
            return;
        }

        let content = if self.buffer.is_empty() && !focused {
            // Show placeholder
            Line::from(vec![
                Span::styled("> ", prefix_style),
                Span::styled(
                    &self.placeholder,
                    Style::default().fg(Color::DarkGray).italic(),
                ),
            ])
        } else {
            let _vis_cursor = self.visual_cursor_col();
            let vis_total = self.visual_width();
            let mut spans = vec![Span::styled("> ", prefix_style)];

            if vis_total <= avail {
                // Everything fits — normal render with cursor
                let before: String = self.buffer[..self.cursor].to_string();
                let after: String = self.buffer[self.cursor..].to_string();
                let cursor_char = self.buffer[self.cursor..].chars().next();

                if !before.is_empty() {
                    spans.push(Span::raw(before));
                }
                if focused {
                    if let Some(c) = cursor_char {
                        spans.push(Span::styled(
                            c.to_string(),
                            Style::default().bg(Color::White).fg(Color::Black),
                        ));
                    } else {
                        spans.push(Span::styled(
                            " ",
                            Style::default().bg(Color::White).fg(Color::Black),
                        ));
                    }
                }
                if !after.is_empty() {
                    spans.push(Span::raw(after));
                }
            } else {
                // Content overflows — show viewport around cursor
                // Collect all chars with byte positions and widths
                let chars: Vec<(char, usize, usize)> = self
                    .buffer
                    .chars()
                    .scan(0usize, |offset, c| {
                        let pos = *offset;
                        let w = UnicodeWidthChar::width(c).unwrap_or(0);
                        *offset += c.len_utf8();
                        Some((c, pos, w))
                    })
                    .collect();

                // Find the cursor char index
                let cursor_idx = chars.iter().position(|&(_, pos, _)| pos == self.cursor);

                // Compute visible range: `avail` columns ending at cursor
                let end_idx = cursor_idx.unwrap_or(chars.len());
                let mut col = 0;
                let mut start_idx = end_idx;
                for i in (0..end_idx).rev() {
                    let w = chars[i].2;
                    if col + w > avail {
                        break;
                    }
                    col += w;
                    start_idx = i;
                }

                let visible: String = chars[start_idx..]
                    .iter()
                    .take_while(|&&(_, _, w)| {
                        let fits = col + w <= avail;
                        if fits {
                            col += w;
                        }
                        fits
                    })
                    .map(|&(c, _, _)| c)
                    .collect();

                // Find cursor position within visible for highlighting
                let cursor_in_visible = if focused {
                    cursor_idx
                        .filter(|&idx| idx >= start_idx)
                        .map(|idx| idx - start_idx)
                } else {
                    None
                };

                if let Some(ci) = cursor_in_visible {
                    let before_vis: String = visible.chars().take(ci).collect();
                    let after_vis: String = visible.chars().skip(ci + 1).collect();
                    let cursor_ch = visible.chars().nth(ci);

                    if !before_vis.is_empty() {
                        spans.push(Span::raw(before_vis));
                    }
                    if let Some(c) = cursor_ch {
                        spans.push(Span::styled(
                            c.to_string(),
                            Style::default().bg(Color::White).fg(Color::Black),
                        ));
                    }
                    if !after_vis.is_empty() {
                        spans.push(Span::raw(after_vis));
                    }
                } else {
                    spans.push(Span::raw(visible));
                }
            }

            Line::from(spans)
        };

        let paragraph = Paragraph::new(content);
        frame.render_widget(paragraph, area);
    }
}
