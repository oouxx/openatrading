pub mod cli;

// Platform-specific modules that require additional dependencies.
// Each is gated behind its own feature flag so the core crate doesn't
// pull in heavy dependencies (tauri, sqlx, axum/ws, teloxide, etc.)
// unless explicitly enabled.

#[cfg(feature = "tauri")]
pub mod desktop;

#[cfg(feature = "web")]
pub mod web;

#[cfg(feature = "web")]
pub mod web_sse;

#[cfg(feature = "telegram")]
pub mod telegram;

pub mod tui;
