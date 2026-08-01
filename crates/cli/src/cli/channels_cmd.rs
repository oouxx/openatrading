//! `openatrading channels` 子命令。

use anyhow::Result;

/// 列出所有通道的编译状态。
pub fn cmd_channels_list() -> Result<()> {
    let entries = openatrading_core::channels::listing::compiled_channels();
    println!("通道编译状态:\n");
    for entry in &entries {
        let status = if entry.compiled { "✅" } else { "❌" };
        println!("  {status} {} ({})", entry.name, entry.config_key);
    }
    println!();
    let compiled_count = entries.iter().filter(|e| e.compiled).count();
    let total = entries.len();
    println!("已编译 {compiled_count}/{total} 个通道");
    Ok(())
}
