use anyhow::Result;
pub use openatrading_core::config::{
    DiscordConfig, LarkConfig, SlackConfig, SocialConfig, TelegramConfig,
};
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
    #[serde(default = "default_news_schedule")]
    pub news_schedule: String,
}

const fn default_true() -> bool {
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
fn default_news_schedule() -> String {
    "0 */5 9-15 * * 1-5".to_string()
}

impl AppConfig {
    /// 三层作用域配置合并（类似 Claude 的 ~/.claude/ + .claude/ 模式）:
    /// 1. 内置默认值（struct defaults）
    /// 2. 用户配置: {atrading_home}/config.toml
    /// 3. 项目配置: cwd/.atrading/config.toml（覆盖用户配置，类似 Claude 的 .claude/）
    pub fn load() -> Result<Self> {
        let home = openatrading_core::atrading_home();
        openatrading_core::log::info!(?home, "Loading config from home");

        let mut merged = Self {
            default_role: None,
            roles: Vec::new(),
            mcp_servers: Vec::new(),
            service: ServiceConfig {
                db_path: default_db_path(),
                web_bind: default_web_bind(),
            },
            cron: None,
            social: None,
            mcp_server: McpServerTomlConfig::default(),
        };

        // Load config from {atrading_home}/config.toml
        let home_cfg = home.join("config.toml");
        if let Some(cfg) = Self::load_from(&home_cfg) {
            merged.merge(cfg);
        }

        // Layer 3: Project-level config from cwd/.atrading/config.toml
        if let Ok(cwd) = std::env::current_dir() {
            let project_cfg_path = cwd.join(".atrading").join("config.toml");
            // OPENATRADING_HOME 指向项目根时 home 配置与项目配置是同一个文件，
            // 跳过重复加载与重复的未知字段告警。
            if project_cfg_path.exists() && project_cfg_path != home_cfg {
                openatrading_core::log::info!(
                    "Loading project config from {}",
                    project_cfg_path.display()
                );
                if let Some(cfg) = Self::load_from(&project_cfg_path) {
                    merged.merge(cfg);
                }
                validate_config(&project_cfg_path);
            }
        }

        // Project-level .mcp.json (cwd/.atrading/.mcp.json)
        if let Ok(cwd) = std::env::current_dir() {
            let project_mcp = cwd.join(".atrading").join(".mcp.json");
            if project_mcp.exists() {
                openatrading_core::log::info!(
                    "Loading project MCP config from {}",
                    project_mcp.display()
                );
                Self::load_mcp_json(&project_mcp, &mut merged.mcp_servers);
            }
        }

        // Load .mcp.json MCP server configs
        Self::load_mcp_json(&home.join(".mcp.json"), &mut merged.mcp_servers);

        // 配置校验：检查未知字段
        validate_config(&home.join("config.toml"));

        Ok(merged)
    }

    fn load_from(config_path: &Path) -> Option<Self> {
        let content = match std::fs::read_to_string(config_path) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
            Err(e) => {
                eprintln!("跳过 {}: {e}", config_path.display());
                return None;
            }
        };
        let content = substitute_env(&content);
        let raw: RawConfig = match toml::from_str(&content) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("跳过 {} (解析错误): {e}", config_path.display());
                return None;
            }
        };
        Some(Self {
            default_role: raw.default_role,
            roles: raw.roles,
            mcp_servers: raw.mcp_servers,
            service: raw.service.unwrap_or_else(|| ServiceConfig {
                db_path: default_db_path(),
                web_bind: default_web_bind(),
            }),
            cron: raw.cron,
            social: raw.social,
            mcp_server: raw.mcp_server.unwrap_or_default(),
        })
    }

    fn merge(&mut self, other: Self) {
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
            env: ahash::AHashMap<String, String>,
        }
        fn default_transport_type() -> String {
            "streamable-http".to_string()
        }
        fn default_url() -> String {
            "http://localhost:8081/mcp".to_string()
        }
        const fn default_enabled() -> bool {
            true
        }

        let Ok(content) = std::fs::read_to_string(path) else {
            return;
        };
        let parsed: McpJson = match serde_json::from_str(&content) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("跳过 {} (解析错误): {e}", path.display());
                return;
            }
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
    pub fn social_notify_channel(&self) -> Option<&str> {
        self.social.as_ref()?.notify_channel.as_deref()
    }
    pub fn social_telegram(&self) -> Option<&TelegramConfig> {
        self.social.as_ref()?.telegram.as_ref()
    }
    pub fn social_discord(&self) -> Option<&DiscordConfig> {
        self.social.as_ref()?.discord.as_ref()
    }
    /// Write one key under `section` to config.toml.
    /// `section` is a dotted path, e.g. "llm" or "social.telegram".
    /// Uses `toml_edit` for round-trip preserving comments and section style.
    pub fn set_value(section: &str, key: &str, value: &str) -> Result<()> {
        let path = Self::config_path();
        let content = std::fs::read_to_string(&path).unwrap_or_default();
        let mut doc: toml_edit::DocumentMut = content.parse().unwrap_or_default();

        // Navigate/create sub-tables along the dotted section path
        let mut current = doc.as_table_mut();
        for part in section.split('.') {
            if !current.contains_table(part) {
                current.insert(part, toml_edit::table());
            }
            current = current
                .entry(part)
                .or_insert(toml_edit::table())
                .as_table_mut()
                .ok_or_else(|| anyhow::anyhow!("section {section} is not a table"))?;
        }

        current.insert(key, value.into());

        std::fs::write(&path, doc.to_string())?;
        Ok(())
    }
}

pub fn cmd_init() -> Result<()> {
    let created = openatrading_core::init::ensure_initialized()?;
    let atrading = openatrading_core::atrading_home();
    if created {
        eprintln!("✅ 已初始化 {}", atrading.display());
        eprintln!("   配置文件: {}", atrading.join("config.toml").display());
        eprintln!("   (可运行 `openatrading skills install` 安装 pi-rs 技能)");
    } else {
        eprintln!("{} 已存在，跳过初始化", atrading.display());
    }
    Ok(())
}

/// 校验配置文件中是否有未知字段。
pub fn validate_config(path: &Path) {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return,
    };

    let parsed: Result<toml::Value, _> = content.parse();
    let Ok(table) = parsed else { return };
    let Some(table) = table.as_table() else {
        return;
    };

    let known: std::collections::HashSet<&str> = [
        "default_role",
        "roles",
        "mcp_servers",
        "service",
        "cron",
        "social",
        "mcp_server",
    ]
    .into();

    for key in table.keys() {
        if !known.contains(key.as_str()) {
            eprintln!(
                "⚠ 配置文件中存在未知字段: {key}\n   {}\n   请检查拼写或删除该字段",
                path.display()
            );
        }
    }
}
