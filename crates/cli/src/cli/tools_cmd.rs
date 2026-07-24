use std::sync::Arc;

use openatrading_core::llm::DynTool;

pub async fn cmd_tools_doc(endpoint: &str) -> anyhow::Result<()> {
    // ── Collect all tools ─────────────────────────────
    let mut all_tools: Vec<Arc<DynTool>> =
        openatrading_core::llm::tools::ToolRegistry::new().all_tools();

    let mcp = openatrading_core::llm::McpRegistry::single(endpoint);
    all_tools.extend(mcp.discover().await.0);

    // ── Group by installed skills ───────────────────────
    let reg = openatrading_core::skills::SkillRegistry::from_disk();
    let skills = reg.skills().to_vec();

    let mut out = String::new();
    out.push_str("# 工具参考文档\n\n本文档列出 LLM 可用的所有工具，按已安装技能分组。\n\n---\n\n");

    let mut used_names: std::collections::HashSet<String> = std::collections::HashSet::new();

    for skill in &skills {
        out.push_str(&format!("## {}\n\n  \n\n", skill.name));
        out.push_str("| 工具 | 说明 | 参数 |\n|------|------|------|\n");

        let mut count = 0;
        for tool in &all_tools {
            if skill.content.contains(&tool.name) {
                out.push_str(&format!(
                    "| `{}` | {} | {} |\n",
                    tool.name,
                    tool.description,
                    format_params(&tool.parameters_schema)
                ));
                used_names.insert(tool.name.clone());
                count += 1;
            }
        }
        if count == 0 {
            out.push_str("| — | 无关联工具 | — |\n");
        }
        out.push('\n');
    }

    // ── 未分组的工具 ───────────────────────────────────
    let ungrouped: Vec<&Arc<DynTool>> = all_tools
        .iter()
        .filter(|t| !used_names.contains(&t.name))
        .collect();

    if !ungrouped.is_empty() {
        out.push_str("## 其他工具\n\n| 工具 | 说明 | 参数 |\n|------|------|------|\n");
        for tool in &ungrouped {
            out.push_str(&format!(
                "| `{}` | {} | {} |\n",
                tool.name,
                tool.description,
                format_params(&tool.parameters_schema)
            ));
        }
        out.push('\n');
    }

    out.push_str("---\n\n*文档由 `openatrading tools doc` 自动生成*\n");
    print!("{out}");
    Ok(())
}

fn format_params(schema: &serde_json::Value) -> String {
    let props = match schema.get("properties").and_then(|p| p.as_object()) {
        Some(p) => p,
        None => return "无".to_string(),
    };
    if props.is_empty() {
        return "无".to_string();
    }
    let required: std::collections::HashSet<&str> = schema
        .get("required")
        .and_then(|r| r.as_array())
        .map(|r| r.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();

    props
        .iter()
        .map(|(name, info)| {
            let desc = info
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let req = if required.contains(name.as_str()) {
                "必填"
            } else {
                "可选"
            };
            let ptype = info
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("string");
            format!("`{name}` ({ptype}, {req}, {desc})")
        })
        .collect::<Vec<_>>()
        .join("<br>")
}
