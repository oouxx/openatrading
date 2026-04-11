mod api;
mod cli;
mod llm;
mod report;
mod roles;
mod templates;
mod web;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    cli::run().await
}
