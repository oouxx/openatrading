use anyhow::Result;
use openatrading_core::skills::{SkillEntry, SkillRegistry};

pub fn cmd_skills_list() -> Result<()> {
    let reg = SkillRegistry::from_disk();
    let skills = reg.skills();
    if skills.is_empty() {
        eprintln!("暂无已安装的技能");
        return Ok(());
    }
    println!("已安装的技能 ({}):\n", skills.len());
    for skill in skills {
        let scope = match skill.scope {
            openatrading_core::skills::SkillScope::Project => " [project]",
            openatrading_core::skills::SkillScope::User => "",
        };
        println!("  {:<30} {}{}", skill.name, skill.description, scope);
        if let Some(compat) = &skill.compatibility {
            println!("  {:>30} 兼容性: {}", "", compat);
        }
        if !skill.list_resources().is_empty() {
            println!("  {:>30} 资源: {}", "", skill.list_resources().join(", "));
        }
    }
    Ok(())
}

/// Resolve a skill source argument. Tries:
/// 1. File path (if the path exists)
/// 2. Raw content (anything else)
pub fn resolve_skill_source(name: &str, source: &str) -> Result<SkillEntry> {
    // Try file path
    let path = std::path::Path::new(source);
    if path.exists() && path.is_file() {
        let raw = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("读取文件失败 {}: {e}", source))?;
        return SkillRegistry::parse_skill_from_md(&raw)
            .map(|mut s| {
                s.name = name.to_string();
                s
            })
            .ok_or_else(|| anyhow::anyhow!("文件内容为空: {source}"));
    }

    // Treat as raw content
    SkillRegistry::parse_skill_from_md(source)
        .map(|mut s| {
            s.name = name.to_string();
            s
        })
        .ok_or_else(|| anyhow::anyhow!("内容为空"))
}

pub fn cmd_skills_install(name: &str, source: &str) -> Result<()> {
    let skill = resolve_skill_source(name, source)?;
    let path = SkillRegistry::install_skill(&skill)?;
    eprintln!("✅ 技能已安装: {} ({})", skill.name, path.display());
    Ok(())
}

/// Install a skill from a URL (downloads the content).
pub async fn cmd_skills_install_url(name: &str, url: &str) -> Result<()> {
    let raw = reqwest::get(url)
        .await
        .map_err(|e| anyhow::anyhow!("下载失败: {e}"))?
        .text()
        .await
        .map_err(|e| anyhow::anyhow!("读取响应失败: {e}"))?;

    let skill = SkillRegistry::parse_skill_from_md(&raw)
        .map(|mut s| {
            s.name = name.to_string();
            s
        })
        .ok_or_else(|| anyhow::anyhow!("URL 内容为空: {url}"))?;

    let path = SkillRegistry::install_skill(&skill)?;
    eprintln!("✅ 技能已安装: {} ({})", skill.name, path.display());
    Ok(())
}

pub fn cmd_skills_uninstall(name: &str) -> Result<()> {
    let dir = SkillRegistry::install_dir().join(name);
    if !dir.exists() {
        anyhow::bail!("技能未安装: {name}");
    }
    std::fs::remove_dir_all(&dir)?;
    eprintln!("✅ 技能已卸载: {name}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_resolve_from_file() {
        let dir = SkillRegistry::install_dir();
        fs::create_dir_all(&dir).ok();
        let file_path = dir.join("_test_resolve_skill.md");
        let raw = "# 首板策略\n\n只做首板";
        fs::write(&file_path, raw).unwrap();

        let skill = resolve_skill_source("test-skill", &file_path.to_string_lossy())
            .expect("should resolve from file");
        assert_eq!(skill.name, "test-skill");
        assert!(skill.content.contains("首板策略"));

        let _ = fs::remove_file(&file_path);
    }

    #[test]
    fn test_resolve_from_raw() {
        let raw = "# 我的策略\n\n买入条件：均线多头排列";
        let skill =
            resolve_skill_source("my-strategy", raw).expect("should resolve from raw content");
        assert_eq!(skill.name, "my-strategy");
        assert!(skill.content.contains("均线多头排列"));
    }

    #[test]
    fn test_resolve_empty() {
        assert!(resolve_skill_source("empty", "").is_err());
        assert!(resolve_skill_source("empty", "   \n  ").is_err());
    }
}
