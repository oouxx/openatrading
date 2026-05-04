mod cli;

use cli::run;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    run().await
}
