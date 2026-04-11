mod api;
mod cli;
mod llm;
mod report;
mod roles;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    cli::run().await
}
