use anyhow::Result;
use openatrading_core::skills::{install_skill, list_skills, uninstall_skill, SkillRequest};

pub fn cmd_skills_list() -> Result<()> {
    let skills = list_skills();
    if skills.is_empty() {
        eprintln!("暂无已安装的技能");
        return Ok(());
    }
    println!("已安装的技能 ({}):\n", skills.len());
    for skill in skills {
        let scope = match openatrading_core::skills::skill_scope(&skill) {
            "project" => " [project]",
            "temporary" => " [temp]",
            _ => "",
        };
        println!("  {:<30} {}{}", skill.name, skill.description, scope);
    }
    Ok(())
}

/// Parse raw skill markdown into a `SkillRequest`, using `name` as override.
fn parse_raw(name: &str, raw: &str) -> Result<SkillRequest> {
    let (description, content) = match openatrading_core::utils::parse_yaml_frontmatter(raw) {
        Some((matter, body)) => {
            // best-effort description extraction from frontmatter
            let desc = matter
                .lines()
                .find_map(|l| l.strip_prefix("description:").map(|s| s.trim().to_string()))
                .unwrap_or_default();
            (desc, body.trim().to_string())
        }
        None => (String::new(), raw.trim().to_string()),
    };
    Ok(SkillRequest {
        name: name.to_string(),
        description,
        content,
        disable_model_invocation: false,
    })
}

/// Resolve a skill source argument. Tries:
/// 1. File path (if the path exists)
/// 2. Raw content (anything else)
pub fn resolve_skill_source(name: &str, source: &str) -> Result<SkillRequest> {
    let path = std::path::Path::new(source);
    if path.exists() && path.is_file() {
        let raw = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("读取文件失败 {source}: {e}"))?;
        return parse_raw(name, &raw);
    }
    parse_raw(name, source)
}

pub fn cmd_skills_install(name: &str, source: &str) -> Result<()> {
    let req = resolve_skill_source(name, source)?;
    let path = install_skill(&req)?;
    eprintln!("✅ 技能已安装: {} ({})", req.name, path.display());
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
    let req = parse_raw(name, &raw)?;
    let path = install_skill(&req)?;
    eprintln!("✅ 技能已安装: {} ({})", req.name, path.display());
    Ok(())
}

pub fn cmd_skills_uninstall(name: &str) -> Result<()> {
    uninstall_skill(name).map_err(|e| anyhow::anyhow!("技能未安装或删除失败: {e}"))?;
    eprintln!("✅ 技能已卸载: {name}");
    Ok(())
}
