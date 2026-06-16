use crate::commands::{CommandSpec, CommandType, PipelineSpec};

/// Registry of installed pipelines loaded from `~/.atrading/pipelines/`.
pub struct PipelineRegistry {
    pipelines: Vec<PipelineSpec>,
}

impl PipelineRegistry {
    pub fn new() -> Self {
        Self {
            pipelines: Vec::new(),
        }
    }

    pub fn from_disk() -> Self {
        let dir = crate::atrading_home().join("pipelines");
        if !dir.exists() {
            return Self::new();
        }

        let mut reg = Self::new();
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => return Self::new(),
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let pipeline_path = path.join("PIPELINE.md");
            if !pipeline_path.exists() {
                continue;
            }
            let content = match std::fs::read_to_string(&pipeline_path) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();

            let (tools, description, body) = Self::parse_frontmatter(&content);
            if body.is_empty() {
                continue;
            }
            reg.pipelines.push(PipelineSpec {
                name,
                description,
                tools,
                prompt: body,
            });
        }

        reg
    }

    pub fn is_empty(&self) -> bool {
        self.pipelines.is_empty()
    }

    pub fn pipelines(&self) -> &[PipelineSpec] {
        &self.pipelines
    }

    pub fn commands(&self) -> Vec<CommandSpec> {
        self.pipelines
            .iter()
            .map(|p| CommandSpec {
                name: p.name.clone(),
                description: p.description.clone(),
                command_type: CommandType::Pipeline,
            })
            .collect()
    }

    pub fn find(&self, name: &str) -> Option<&PipelineSpec> {
        self.pipelines.iter().find(|p| p.name == name)
    }

    /// Minimal frontmatter parser: reads lines between `---` delimiters,
    /// extracts `description:` and `tools:` list. Everything after the
    /// closing `---` is the prompt body.
    fn parse_frontmatter(raw: &str) -> (Vec<String>, String, String) {
        let mut tools = Vec::new();
        let mut description = String::new();
        let mut in_frontmatter = false;
        let mut body_start = 0;

        for (i, line) in raw.lines().enumerate() {
            if line.trim() == "---" {
                if !in_frontmatter {
                    in_frontmatter = true;
                    body_start = i + 1;
                    continue;
                } else {
                    // closing ---
                    body_start = i + 1;
                    break;
                }
            }
            if in_frontmatter {
                if let Some(rest) = line.trim().strip_prefix("description:") {
                    description = rest.trim().to_string();
                }
                if let Some(rest) = line.trim().strip_prefix("- ") {
                    tools.push(rest.trim().to_string());
                }
            }
        }

        let body: String = raw.lines().skip(body_start).collect::<Vec<_>>().join("\n");
        let body = body.trim().to_string();

        (tools, description, body)
    }
}

impl Default for PipelineRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_frontmatter_full() {
        let raw = "---\ndescription: 盘前检查\ntools:\n  - get_market_overview\n  - get_limit_up\n---\n\n# 盘前检查\n\n分析市场总体情绪。";
        let (tools, desc, body) = PipelineRegistry::parse_frontmatter(raw);
        assert_eq!(desc, "盘前检查");
        assert_eq!(tools, vec!["get_market_overview", "get_limit_up"]);
        assert!(body.contains("盘前检查"));
    }

    #[test]
    fn test_parse_frontmatter_no_frontmatter() {
        let raw = "# Just a title\n\nSome content";
        let (tools, desc, body) = PipelineRegistry::parse_frontmatter(raw);
        assert!(tools.is_empty());
        assert!(desc.is_empty());
        assert_eq!(body.trim(), raw.trim());
    }

    #[test]
    fn test_parse_frontmatter_empty_body() {
        let raw = "---\ndescription: test\n---";
        let (_, _, body) = PipelineRegistry::parse_frontmatter(raw);
        assert!(body.is_empty());
    }

    #[test]
    fn test_pipeline_registry_from_disk_empty() {
        let reg = PipelineRegistry::from_disk();
        assert!(reg.is_empty());
    }
}
