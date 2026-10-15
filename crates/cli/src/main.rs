mod cli;

use cli::{preflight, run};

/// 进程级前置准备（`.env` + 品牌根绑定）必须在 async runtime 起来之前、单线程阶段完成，
/// 因此这里不用 `#[tokio::main]`，而是显式建 runtime。
fn main() -> anyhow::Result<()> {
    preflight();

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(run())
}
