use anyhow::Result;
use openatrading_core::skills::SkillRegistry;

pub fn cmd_skills_list() -> Result<()> {
    let skills = SkillRegistry::builtin_skills();
    if skills.is_empty() {
        eprintln!("无可用技能");
        return Ok(());
    }
    println!("可用技能 ({}):\n", skills.len());
    for skill in &skills {
        let installed = SkillRegistry::install_dir()
            .join(&skill.name)
            .join("SKILL.md");
        let status = if installed.exists() {
            "已安装"
        } else {
            "未安装"
        };
        println!(
            "  {:<30} {}\n  {:>30} {}\n",
            skill.name, skill.description, "", status
        );
    }
    Ok(())
}

pub fn cmd_skills_install(name: &str) -> Result<()> {
    let skills = SkillRegistry::builtin_skills();
    let skill = skills
        .iter()
        .find(|s| s.name == name)
        .ok_or_else(|| anyhow::anyhow!("未找到技能: {name}"))?;

    let path = SkillRegistry::install_skill(skill)?;
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
