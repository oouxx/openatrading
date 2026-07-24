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
            .map_err(|e| anyhow::anyhow!("读取文件失败 {source}: {e}"))?;
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
