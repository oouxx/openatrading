use anyhow::Result;
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub default_role: Option<String>,
    pub roles: Vec<openatrading_core::roles::RoleConfig>,
    pub mcp_servers: Vec<openatrading_core::llm::McpServerConfig>,
    pub service: ServiceConfig,
    pub cron: Option<CronTomlConfig>,
    pub social: Option<SocialConfig>,
    pub llm: LlmTomlConfig,
    pub mcp_server: McpServerTomlConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct McpServerTomlConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_mcp_bind")]
    pub bind: String,
}

impl Default for McpServerTomlConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            bind: default_mcp_bind(),
        }
    }
}

fn default_mcp_bind() -> String {
    "0.0.0.0:8082".to_string()
}

#[derive(Debug, Clone, Deserialize)]
pub struct LlmTomlConfig {
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawConfig {
    default_role: Option<String>,
    #[serde(default)]
    roles: Vec<openatrading_core::roles::RoleConfig>,
    #[serde(default)]
    mcp_servers: Vec<openatrading_core::llm::McpServerConfig>,
    #[serde(default)]
    service: Option<ServiceConfig>,
    #[serde(default)]
    cron: Option<CronTomlConfig>,
    #[serde(default)]
    social: Option<SocialConfig>,
    #[serde(default)]
    llm: Option<LlmTomlConfig>,
    #[serde(default)]
    mcp_server: Option<McpServerTomlConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServiceConfig {
    #[serde(default = "default_db_path")]
    pub db_path: String,
    #[serde(default = "default_web_bind")]
    pub web_bind: String,
}

fn default_db_path() -> String {
    "data.db".to_string()
}

fn default_web_bind() -> String {
    "0.0.0.0:8080".to_string()
}

#[derive(Debug, Clone, Deserialize)]
pub struct CronTomlConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_collection_schedule")]
    pub collection_schedule: String,
    #[serde(default = "default_decision_schedule")]
    pub decision_schedule: String,
    #[serde(default = "default_news_schedule")]
    pub news_schedule: String,
}

fn default_true() -> bool {
    true
}

fn substitute_env(content: &str) -> String {
    shellexpand::full(content)
        .unwrap_or_else(|_| content.into())
        .into()
}

fn default_collection_schedule() -> String {
    "0 0 18 * * 1-5".to_string()
}
fn default_decision_schedule() -> String {
    "0 */30 9-15 * * 1-5".to_string()
}
fn default_news_schedule() -> String {
    "0 */5 9-15 * * 1-5".to_string()
}

#[derive(Debug, Clone, Deserialize)]
pub struct SocialConfig {
    #[serde(default)]
    pub notify_channel: Option<String>,
    #[serde(default)]
    pub telegram: Option<TelegramConfig>,
    #[serde(default)]
    pub discord: Option<DiscordConfig>,
    #[serde(default)]
    pub slack: Option<SlackConfig>,
    #[serde(default)]
    pub lark: Option<LarkConfig>,
    #[serde(default = "default_true")]
    pub notify_on_collect: bool,
    #[serde(default = "default_true")]
    pub notify_on_decision: bool,
    #[serde(default = "default_true")]
    pub notify_on_trade: bool,
    #[serde(default = "default_true")]
    pub notify_on_news: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TelegramConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub bot_polling: bool,
    pub bot_token: Option<String>,
    pub chat_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DiscordConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub bot_token: Option<String>,
    pub channel_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SlackConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub bot_token: Option<String>,
    pub channel: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LarkConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub app_id: Option<String>,
    pub app_secret: Option<String>,
    pub receive_id: Option<String>,
}

impl AppConfig {
    pub fn load() -> Result<Self> {
        let home = openatrading_core::atrading_home();
        openatrading_core::log::info!(?home, "Loading config from home");

        let mut merged = AppConfig {
            default_role: None,
            roles: Vec::new(),
            mcp_servers: Vec::new(),
            service: ServiceConfig {
                db_path: default_db_path(),
                web_bind: default_web_bind(),
            },
            cron: None,
            social: None,
            llm: LlmTomlConfig {
                provider: None,
                model: None,
            },
            mcp_server: McpServerTomlConfig::default(),
        };

        // Load config from {atrading_home}/config.toml
        if let Some(cfg) = Self::load_from(&home.join("config.toml")) {
            merged.merge(cfg);
        }

        // Load .mcp.json MCP server configs
        Self::load_mcp_json(&home.join(".mcp.json"), &mut merged.mcp_servers);

        Ok(merged)
    }

    fn load_from(config_path: &Path) -> Option<AppConfig> {
        let content = match std::fs::read_to_string(config_path) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
            Err(e) => {
                eprintln!("跳过 {}: {e}", config_path.display());
                return None;
            },
        };
        let content = substitute_env(&content);
        let raw: RawConfig = match toml::from_str(&content) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("跳过 {} (解析错误): {e}", config_path.display());
                return None;
            },
        };
        Some(AppConfig {
            default_role: raw.default_role,
            roles: raw.roles,
            mcp_servers: raw.mcp_servers,
            service: raw.service.unwrap_or(ServiceConfig {
                db_path: default_db_path(),
                web_bind: default_web_bind(),
            }),
            cron: raw.cron,
            social: raw.social,
            llm: raw.llm.unwrap_or(LlmTomlConfig {
                provider: None,
                model: None,
            }),
            mcp_server: raw.mcp_server.unwrap_or_default(),
        })
    }

    fn merge(&mut self, other: AppConfig) {
        if other.default_role.is_some() {
            self.default_role = other.default_role;
        }
        self.service = other.service;
        if other.cron.is_some() {
            self.cron = other.cron;
        }
        if other.social.is_some() {
            self.social = other.social;
        }
        if other.llm.provider.is_some() {
            self.llm.provider = other.llm.provider;
        }
        if other.llm.model.is_some() {
            self.llm.model = other.llm.model;
        }
        for entry in other.roles {
            self.roles.retain(|e| e.name != entry.name);
            self.roles.push(entry);
        }
        if !other.mcp_servers.is_empty() {
            self.mcp_servers = other.mcp_servers;
        }
        self.mcp_server = other.mcp_server;
    }

    /// Load MCP servers from a `.mcp.json` file.
    ///
    /// Supports both Claude Code format and streamable-http format:
    /// ```json
    /// { "mcpServers": { "name": { "type": "stdio", "command": "npx", "args": ["..."] } } }
    /// { "mcpServers": { "name": { "url": "http://...", "enabled": true } } }
    /// ```
    fn load_mcp_json(path: &Path, servers: &mut Vec<openatrading_core::llm::McpServerConfig>) {
        use openatrading_core::llm::{McpServerConfig, McpTransportConfig};

        #[derive(serde::Deserialize)]
        struct McpJson {
            #[serde(rename = "mcpServers")]
            mcp_servers: std::collections::HashMap<String, McpJsonEntry>,
        }
        #[derive(serde::Deserialize)]
        struct McpJsonEntry {
            #[serde(default = "default_enabled")]
            enabled: bool,
            /// Transport type: "streamable-http", "sse", or "stdio"
            #[serde(rename = "type", default = "default_transport_type")]
            transport_type: String,
            /// Server URL (for streamable-http / sse).
            #[serde(default = "default_url")]
            url: String,
            /// Optional POST endpoint for SSE mode.
            #[serde(skip_serializing_if = "Option::is_none")]
            post_url: Option<String>,
            /// Executable command (for stdio).
            #[serde(default)]
            command: String,
            /// Command arguments (for stdio).
            #[serde(default)]
            args: Vec<String>,
            /// Environment variables (for stdio).
            #[serde(default)]
            env: std::collections::HashMap<String, String>,
        }
        fn default_transport_type() -> String {
            "streamable-http".to_string()
        }
        fn default_url() -> String {
            "http://localhost:8081/mcp".to_string()
        }
        fn default_enabled() -> bool {
            true
        }

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return,
        };
        let parsed: McpJson = match serde_json::from_str(&content) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("跳过 {} (解析错误): {e}", path.display());
                return;
            },
        };

        for (name, entry) in parsed.mcp_servers {
            if servers.iter().any(|s| s.name == name) {
                continue;
            }
            let transport = match entry.transport_type.as_str() {
                "sse" => McpTransportConfig::Sse {
                    url: entry.url,
                    post_url: entry.post_url,
                },
                "stdio" => McpTransportConfig::Stdio {
                    command: entry.command,
                    args: entry.args,
                    env: entry.env,
                },
                _ => McpTransportConfig::StreamableHttp {
                    url: entry.url,
                    auth_token: None,
                },
            };
            servers.push(McpServerConfig {
                name,
                transport,
                enabled: entry.enabled,
            });
        }
    }

    pub fn config_path() -> PathBuf {
        openatrading_core::atrading_home().join("config.toml")
    }

    /// Read LLM provider from config.
    pub fn llm_provider(&self) -> Option<&str> {
        self.llm.provider.as_deref()
    }

    /// Read LLM model from config.
    pub fn llm_model(&self) -> Option<&str> {
        self.llm.model.as_deref()
    }

    /// Read LLM provider from config.
    pub fn social_notify_channel(&self) -> Option<&str> {
        self.social.as_ref()?.notify_channel.as_deref()
    }
    pub fn social_telegram(&self) -> Option<&TelegramConfig> {
        self.social.as_ref()?.telegram.as_ref()
    }
    pub fn social_discord(&self) -> Option<&DiscordConfig> {
        self.social.as_ref()?.discord.as_ref()
    }
    pub fn social_slack(&self) -> Option<&SlackConfig> {
        self.social.as_ref()?.slack.as_ref()
    }
    pub fn social_lark(&self) -> Option<&LarkConfig> {
        self.social.as_ref()?.lark.as_ref()
    }

    /// Write one key under `section` to config.toml.
    /// `section` is a dotted path, e.g. "llm" or "social.telegram".
    /// Uses serde + toml for round-trip — safe and correct.
    pub fn set_value(section: &str, key: &str, value: &str) -> Result<()> {
        let path = Self::config_path();
        let content = std::fs::read_to_string(&path).unwrap_or_default();
        let mut doc: toml::Value = content
            .parse()
            .unwrap_or(toml::Value::Table(toml::value::Table::new()));

        // Navigate/create sub-tables along the dotted section path
        let mut current = &mut doc;
        for part in section.split('.') {
            current = current
                .as_table_mut()
                .ok_or_else(|| anyhow::anyhow!("section {section} is not a table"))?
                .entry(part)
                .or_insert_with(|| toml::Value::Table(toml::value::Table::new()));
        }

        // Insert the key-value pair
        // Insert the key-value pair
        let table = current
            .as_table_mut()
            .ok_or_else(|| anyhow::anyhow!("section {section} is not a table"))?;
        let entry_line = format!("{key} = {value:?}");
        match entry_line.parse::<toml::Value>() {
            Ok(toml::Value::Table(mut kv)) => {
                if let Some((_, v)) = kv.iter_mut().next() {
                    table.insert(key.to_string(), v.clone());
                }
            },
            _ => {
                table.insert(key.to_string(), toml::Value::String(value.to_string()));
            },
        }

        std::fs::write(&path, doc.to_string())?;
        Ok(())
    }
}

pub fn cmd_init() -> Result<()> {
    let atrading = openatrading_core::atrading_home();

    if atrading.exists() {
        eprintln!("{} 已存在，跳过初始化", atrading.display());
        return Ok(());
    }

    std::fs::create_dir_all(&atrading)?;
    std::fs::write(
        atrading.join("config.toml"),
        include_str!("../../../../config.example.toml"),
    )?;

    // Create skills dir with default zarili strategy
    let skills_dir = atrading.join("skills");
    std::fs::create_dir_all(&skills_dir)?;

    eprintln!("✅ 已初始化 {}", atrading.display());
    eprintln!("   配置文件: {}", atrading.join("config.toml").display());

    eprintln!("   (可运行 `openatrading skills install` 查看更多可安装技能)");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcp_server_defaults() {
        let cfg = McpServerTomlConfig::default();
        assert!(cfg.enabled);
        assert_eq!(cfg.bind, "0.0.0.0:8082");
    }

    #[test]
    fn config_loads_without_mcp_server_section() -> Result<()> {
        let toml_text = r#"
            [llm]
            provider = "anthropic"
            model = "claude-sonnet-4-6"

            [service]
            db_path = "data.db"
        "#;
        let raw: RawConfig = toml::from_str(toml_text)?;
        assert!(raw.mcp_server.is_none());
        Ok(())
    }
}
