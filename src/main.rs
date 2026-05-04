use trending_agent::cli;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    cli::run().await
}
// test comment

fn test_func(_x: i32, _y: i32, _z: i32) {
    println!("hello")
}

fn test_func2(_x: i32, _y: i32) {}
