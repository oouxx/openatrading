use anyhow::Result;
use std::io::{self, Write};

pub fn cmd_uninstall() -> Result<()> {
    let current_exe = std::env::current_exe().ok();

    println!("🗑  正在卸载 openatrading...\n");

    // 停止并移除 systemd 服务
    if std::process::Command::new("systemctl")
        .arg("--user")
        .arg("status")
        .arg("openatrading")
        .output()
        .is_ok_and(|o| o.status.success())
    {
        println!("  停止 systemd 服务...");
        let _ = std::process::Command::new("systemctl")
            .args(["--user", "stop", "openatrading"])
            .status();
        let _ = std::process::Command::new("systemctl")
            .args(["--user", "disable", "openatrading"])
            .status();

        let service_path = std::env::var("HOME")
            .map(|h| std::path::PathBuf::from(h).join(".config/systemd/user/openatrading.service"))
            .ok();
        if let Some(ref path) = service_path {
            if path.exists() {
                let _ = std::fs::remove_file(path);
                println!("  ✓ 已移除 systemd 服务文件");
            }
        }

        let _ = std::process::Command::new("systemctl")
            .args(["--user", "daemon-reload"])
            .status();
    }

    // 询问是否删除配置
    print!("  是否删除用户配置 ~/.atrading ？ [y/N] ");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    if input.trim().eq_ignore_ascii_case("y") {
        let home = openatrading_core::atrading_home();
        if home.exists() {
            std::fs::remove_dir_all(&home)?;
            println!("  ✓ 已删除 {}", home.display());
        }
    }

    // 删除自身
    if let Some(exe) = current_exe {
        if exe.exists() {
            let _ = std::fs::remove_file(&exe);
            println!("  ✓ 已移除 {}", exe.display());
        }
    }

    println!("\n✅ openatrading 已卸载");
    Ok(())
}
