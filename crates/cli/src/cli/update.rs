use anyhow::{Context, Result};

const REPO: &str = "oouxx/openatrading";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

fn asset_name() -> Option<&'static str> {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        return Some("openatrading-x86_64-unknown-linux-gnu");
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    {
        return Some("openatrading-aarch64-unknown-linux-gnu");
    }
    #[allow(unreachable_code)]
    None
}

/// 查询最新版本，返回 (tag_name, download_url)
async fn fetch_latest_release() -> Result<(String, String)> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let client = reqwest::Client::new();
    let resp: serde_json::Value = client
        .get(&url)
        .header("User-Agent", "openatrading-update")
        .send()
        .await?
        .json()
        .await?;

    let tag = resp["tag_name"]
        .as_str()
        .context("无法解析 tag_name")?
        .to_string();

    let assets = resp["assets"].as_array().context("无法解析 assets")?;
    let name = asset_name().context("当前平台不支持更新，仅支持 Linux x86_64 / aarch64")?;
    let download_url = assets
        .iter()
        .find(|a| a["name"].as_str() == Some(name))
        .and_then(|a| a["browser_download_url"].as_str())
        .with_context(|| format!("未找到适用于 {name} 的二进制文件"))?
        .to_string();

    Ok((tag, download_url))
}

/// 检查更新，返回 (最新版本, 当前版本, 是否需要更新)
pub async fn cmd_update_check() -> Result<(String, String, bool)> {
    let (latest_tag, _) = fetch_latest_release().await?;
    let current = format!("v{CURRENT_VERSION}");
    let needs_update = latest_tag != current;
    Ok((latest_tag, current, needs_update))
}

/// 下载并替换当前二进制
pub async fn cmd_update() -> Result<()> {
    let (latest_tag, download_url) = fetch_latest_release().await?;
    let current = format!("v{CURRENT_VERSION}");

    if latest_tag == current {
        println!("✅ 已是最新版本 ({current})");
        return Ok(());
    }

    println!("⬇ 发现新版本: {latest_tag} (当前: {current})");
    println!("正在下载...");

    let client = reqwest::Client::new();
    let bytes = client
        .get(&download_url)
        .header("User-Agent", "openatrading-update")
        .send()
        .await?
        .bytes()
        .await?;

    // 替换当前二进制
    let current_exe = std::env::current_exe().context("无法获取当前二进制路径")?;
    let backup = current_exe.with_extension("bak");

    // Write to temp, then rename
    let tmp = current_exe.with_extension("tmp");
    std::fs::write(&tmp, &bytes)?;

    // Set executable on unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&tmp)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&tmp, perms)?;
    }

    // Backup current, replace with new
    if current_exe.exists() {
        std::fs::rename(&current_exe, &backup)?;
    }
    std::fs::rename(&tmp, &current_exe)?;

    println!("✅ 已更新到 {latest_tag}");
    println!("   旧版本备份: {}", backup.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_current_version_is_set() {
        assert!(!CURRENT_VERSION.is_empty());
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_asset_name_is_valid() {
        let name = asset_name().unwrap();
        assert!(name.starts_with("openatrading-"));
    }
}
