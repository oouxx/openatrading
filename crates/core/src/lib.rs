pub mod agents;
pub mod api;
pub mod cron;
pub mod gateway;
pub mod llm;
pub mod memory;
pub mod report;
pub mod roles;
pub mod social;
pub mod templates;
pub mod trade;
pub mod utils;
pub mod web;

/// Get current datetime in Beijing timezone (UTC+8).
pub fn beijing_now() -> chrono::DateTime<chrono::FixedOffset> {
    chrono::Utc::now().with_timezone(&chrono::FixedOffset::east_opt(8 * 3600).unwrap())
}
