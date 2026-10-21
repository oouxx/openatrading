use anyhow::{Context, Result};
use std::process::Command;

/// 默认下载源：GitHub Releases（每个 tag 的 CLI 产物都传到那里，见
/// `.github/workflows/release-cli.yml`）。自建下载服务器可覆盖：
///   OPENATRADING_DL_SERVER=https://your-domain/downloads
const DEFAULT_DL_SERVER: &str =
    "https://github.com/oouxx/openatrading/releases/latest/download";

/// 解析下载源（去掉尾部斜杠，避免拼出 `//`）。
fn dl_server() -> String {
    std::env::var("OPENATRADING_DL_SERVER")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| DEFAULT_DL_SERVER.to_string())
        .trim_end_matches('/')
        .to_string()
}

const fn asset_name() -> Option<&'static str> {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        return Some("openatrading-x86_64-unknown-linux-gnu");
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    {
        return Some("openatrading-aarch64-unknown-linux-gnu");
    }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        return Some("openatrading-x86_64-apple-darwin");
    }
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        Some("openatrading-aarch64-apple-darwin")
    }
    #[cfg(not(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "macos", target_arch = "aarch64"),
    )))]
    None
}

/// 检查更新，返回 (最新版本描述, 当前版本, 是否需要更新)
pub async fn cmd_update_check() -> Result<(String, String, bool)> {
    let current = env!("GIT_VERSION").to_string();
    let latest = format!("下载源 ({})", dl_server());
    Ok((latest, current, true))
}

/// 从静态服务器下载并替换当前二进制
pub async fn cmd_update() -> Result<()> {
    let name = asset_name().context("当前平台不支持更新，仅支持 Linux/macOS x86_64 和 aarch64")?;
    let url = format!("{}/{name}", dl_server());

    println!("⬇ 正在从服务器下载...");
    println!("   下载地址: {url}");

    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .send()
        .await
        .context("无法连接到下载服务器")?;

    if !resp.status().is_success() {
        anyhow::bail!("下载失败 (HTTP {}): 服务器上未找到 {}", resp.status(), name);
    }

    let bytes = resp.bytes().await?;

    let current_exe = std::env::current_exe().context("无法获取当前二进制路径")?;
    let tmp = current_exe.with_extension("tmp");
    std::fs::write(&tmp, &bytes)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o755);
        std::fs::set_permissions(&tmp, perms)?;
    }

    // 检查下载的二进制版本
    let output = Command::new(&tmp)
        .arg("--version")
        .output()
        .context("无法执行下载的二进制文件")?;
    let server_ver = String::from_utf8_lossy(&output.stdout)
        .trim()
        .strip_prefix("openatrading ")
        .context("下载的文件不是有效的 openatrading 二进制")?
        .to_string();
    let current_ver = env!("GIT_VERSION");

    if server_ver == current_ver {
        std::fs::remove_file(&tmp)?;
        println!("✅ 已是最新版本 ({current_ver})");
        return Ok(());
    }

    // 版本不同，替换当前二进制
    let backup = current_exe.with_extension("bak");
    if current_exe.exists() {
        std::fs::rename(&current_exe, &backup)?;
    }
    std::fs::rename(&tmp, &current_exe)?;

    println!("✅ 已从 {current_ver} 更新到 {server_ver}");
    println!("   旧版本备份: {}", backup.display());
    Ok(())
}
