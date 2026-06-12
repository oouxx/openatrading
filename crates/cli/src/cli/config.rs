use anyhow::Result;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub default_role: Option<String>,
    pub roles: Vec<openatrading_core::roles::RoleConfig>,
    pub mcp_servers: Vec<openatrading_core::llm::McpServerConfig>,
    pub service: ServiceConfig,
    pub cron: Option<CronTomlConfig>,
    pub social: Option<SocialConfig>,
    pub llm: LlmTomlConfig,
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
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServiceConfig {
    #[serde(default = "default_db_path")]
    pub db_path: String,
}

fn default_db_path() -> String {
    "data.db".to_string()
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

const DEFAULT_CONFIG_TOML: &str = r##"# ── OpenATrading 配置 ──────────────────────────────────

# ── LLM 配置 ─────────────────────────────────────────────
[llm]
provider = "openrouter"
model = "openrouter/free"

# ── MCP 服务器 ────────────────────────────────────────────
# akshare 数据源（Docker 环境下使用服务名，本地改为 localhost）
[[mcp_servers]]
name = "akshare"
transport = { type = "streamable-http", url = "http://akshare-mcp:8081/mcp" }
enabled = true

# ── 后台服务 ─────────────────────────────────────────────
[service]
db_path = "data.db"

# ── Cron 调度 ────────────────────────────────────────────
[cron]
enabled = true
collection_schedule = "0 0 18 * * 1-5 *"
decision_schedule = "0 */30 9-15 * * 1-5 *"

# ── 社交平台 ─────────────────────────────────────────────
[social]
notify_channel = "telegram"
notify_on_collect = true
notify_on_decision = true
notify_on_trade = true
notify_on_news = true

[social.telegram]
enabled = true
bot_polling = true
bot_token = "${TELEGRAM_BOT_TOKEN}"
chat_id = "${TELEGRAM_CHAT_ID}"

# [social.discord]
# enabled = true
# bot_token = "${DISCORD_BOT_TOKEN}"
# channel_id = "${DISCORD_CHANNEL_ID}"

# [social.slack]
# enabled = true
# bot_token = "${SLACK_BOT_TOKEN}"
# channel = "${SLACK_CHANNEL}"

# [social.lark]
# enabled = true
# app_id = "${LARK_APP_ID}"
# app_secret = "${LARK_APP_SECRET}"
# receive_id = "${LARK_RECEIVE_ID}"
"##;

impl AppConfig {
    pub fn load() -> Result<Self> {
        let home = openatrading_core::atrading_home();

        let mut merged = AppConfig {
            default_role: None,
            roles: Vec::new(),
            mcp_servers: Vec::new(),
            service: ServiceConfig {
                db_path: default_db_path(),
            },
            cron: None,
            social: None,
            llm: LlmTomlConfig {
                provider: None,
                model: None,
            },
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
            }),
            cron: raw.cron,
            social: raw.social,
            llm: raw.llm.unwrap_or(LlmTomlConfig {
                provider: None,
                model: None,
            }),
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
    }

    /// Load MCP servers from a `.mcp.json` file.
    ///
    /// Format expected:
    /// ```json
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
            /// Transport type: "streamable-http" or "sse"
            #[serde(rename = "type", default = "default_transport_type")]
            transport_type: String,
            /// Server URL.
            #[serde(default = "default_url")]
            url: String,
            /// Optional POST endpoint for SSE mode.
            #[serde(skip_serializing_if = "Option::is_none")]
            post_url: Option<String>,
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

    pub fn list_roles(&self) {
        println!("{:<20} {:<10}", "name", "readonly");
        println!("{}", "-".repeat(30));
        for entry in &self.roles {
            println!("{:<20} {}", entry.name, entry.readonly);
        }
        if let Some(ref default) = self.default_role {
            println!("\ndefault_role: {default}");
        }
    }
}

pub fn cmd_init() -> Result<()> {
    let atrading = openatrading_core::atrading_home();

    if atrading.exists() {
        eprintln!("{} 已存在，跳过初始化", atrading.display());
        return Ok(());
    }

    std::fs::create_dir_all(&atrading)?;
    std::fs::write(atrading.join("config.toml"), DEFAULT_CONFIG_TOML)?;

    // Create skills dir with default zarili strategy
    let skills_dir = atrading.join("skills");
    std::fs::create_dir_all(&skills_dir)?;

    eprintln!("✅ 已初始化 {}", atrading.display());
    eprintln!("   配置文件: {}", atrading.join("config.toml").display());

    eprintln!("   (可运行 `openatrading skills install` 查看更多可安装技能)");
    Ok(())
}
