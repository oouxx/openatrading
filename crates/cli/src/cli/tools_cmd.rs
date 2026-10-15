use std::fmt::Write;

/// 单个工具的展示信息（来自扩展注册表或远端 MCP server）。
struct ToolView {
    name: String,
    description: String,
    params: String,
}

pub async fn cmd_tools_doc(endpoint: &str) -> anyhow::Result<()> {
    // ── 收集工具：扩展注册表（行情/交易）+ 远端 MCP server ────────────────
    let mut views: Vec<ToolView> = Vec::new();
    // 工具名去重集合：本地工具先入，远端 MCP 工具按名去重，
    // 避免远端就是本服务自身（默认 endpoint 指向 gateway）时重复列出。
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    let registry = openatrading_core::llm::create_registry(
        openatrading_core::llm::RegistryOptions::market_only(),
    );
    for rt in registry.tools() {
        seen.insert(rt.definition.name.clone());
        views.push(ToolView {
            name: rt.definition.name.clone(),
            description: rt.definition.description.clone(),
            params: format_params(rt.definition.parameters.as_ref()),
        });
    }

    // 远端 MCP server 工具（失败不阻断本地工具文档生成）。
    // 连接与工具元数据来自 pi-rs SDK 的 MCP 客户端。
    let spec = openatrading_core::llm::McpServerSpec {
        name: "endpoint".to_string(),
        transport: openatrading_core::llm::McpTransportSpec::Http {
            url: endpoint.to_string(),
            headers: std::collections::BTreeMap::new(),
        },
    };
    if let Ok(connection) = openatrading_core::llm::McpConnection::connect_spec(&spec).await {
        for tool in connection.tool_definitions() {
            // 与本地/已收录工具同名则跳过，保证每个工具名只出现一次。
            if !seen.insert(tool.name.clone()) {
                continue;
            }
            views.push(ToolView {
                name: tool.name,
                description: tool.description,
                params: format_params(tool.parameters.as_ref()),
            });
        }
    }

    // ── 按已安装技能分组 ───────────────────────────────────────────────
    let skills = openatrading_core::skills::list_skill_entries();

    let mut out = String::new();
    out.push_str("# 工具参考文档\n\n本文档列出 LLM 可用的所有工具，按已安装技能分组。\n\n---\n\n");

    let mut used_names: std::collections::HashSet<String> = std::collections::HashSet::new();

    for skill in &skills {
        let _ = write!(out, "## {}\n\n  \n\n", skill.name);
        out.push_str("| 工具 | 说明 | 参数 |\n|------|------|------|\n");

        let mut count = 0;
        for tool in &views {
            if skill.content.contains(&tool.name) {
                let _ = writeln!(
                    out,
                    "| `{}` | {} | {} |",
                    tool.name, tool.description, tool.params
                );
                used_names.insert(tool.name.clone());
                count += 1;
            }
        }
        if count == 0 {
            out.push_str("| — | 无关联工具 | — |\n");
        }
        out.push('\n');
    }

    // ── 未分组的工具 ───────────────────────────────────────────────────
    let ungrouped: Vec<&ToolView> = views
        .iter()
        .filter(|t| !used_names.contains(&t.name))
        .collect();

    if !ungrouped.is_empty() {
        out.push_str("## 其他工具\n\n| 工具 | 说明 | 参数 |\n|------|------|------|\n");
        for tool in &ungrouped {
            let _ = std::fmt::Write::write_fmt(
                &mut out,
                format_args!(
                    "| `{}` | {} | {} |\n",
                    tool.name, tool.description, tool.params
                ),
            );
        }
        out.push('\n');
    }

    out.push_str("---\n\n*文档由 `openatrading tools doc` 自动生成*\n");
    print!("{out}");
    Ok(())
}

fn format_params(schema: Option<&serde_json::Value>) -> String {
    let Some(props) = schema
        .and_then(|s| s.get("properties"))
        .and_then(|p| p.as_object())
    else {
        return "无".to_string();
    };
    if props.is_empty() {
        return "无".to_string();
    }
    let required: std::collections::HashSet<&str> = schema
        .and_then(|s| s.get("required"))
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
