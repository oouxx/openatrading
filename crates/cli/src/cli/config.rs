use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub default_role: Option<String>,
    pub roles: Vec<openatrading_core::roles::RoleConfig>,
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

fn substitute_env(mut content: &str) -> std::borrow::Cow<'_, str> {
    let mut result = String::new();
    while let Some(start) = content.find("${") {
        result.push_str(&content[..start]);
        let rest = &content[start + 2..];
        if let Some(end) = rest.find('}') {
            let var = &rest[..end];
            match std::env::var(var) {
                Ok(val) => result.push_str(&val),
                Err(_) => {
                    result.push_str("${");
                    result.push_str(var);
                    result.push('}');
                },
            }
            content = &rest[end + 1..];
        } else {
            result.push_str("${");
            content = rest;
        }
    }
    result.push_str(content);
    std::borrow::Cow::Owned(result)
}

fn default_collection_schedule() -> String {
    "0 */30 9-15 * * 1-5".to_string()
}
fn default_decision_schedule() -> String {
    "0 0 0 * * 1-5".to_string()
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
# 未设置时降级使用环境变量 LLM_API_KEY / LLM_MODEL
#[llm]
# provider = "openrouter"
# model = "openrouter/free"

# ── 后台服务 ─────────────────────────────────────────────
[service]
db_path = "data.db"

# ── Cron 调度 ────────────────────────────────────────────
[cron]
enabled = true
collection_schedule = "0 */30 9-15 * * 1-5 *"
decision_schedule = "0 0 0 * * 1-5 *"

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
    pub fn load(project_root: &Path) -> Result<Self> {
        let home = dirs::home_dir().context("无法获取 HOME 目录")?;
        let home_atrading = home.join(".atrading");
        let project_atrading = project_root.join(".atrading");

        let mut merged = AppConfig {
            default_role: None,
            roles: Vec::new(),
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

        // Layer 1 (lowest): ~/.atrading/config.toml
        if let Some(cfg) = Self::load_from(&home_atrading.join("config.toml")) {
            merged.merge(cfg);
        }

        // Layer 2 (medium): <project>/.atrading/config.toml
        if let Some(cfg) = Self::load_from(&project_atrading.join("config.toml")) {
            merged.merge(cfg);
        }

        // Layer 3 (highest): <project>/config.toml
        if let Some(cfg) = Self::load_from(&project_root.join("config.toml")) {
            merged.merge(cfg);
        }

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
    let home = dirs::home_dir().context("无法获取 HOME 目录")?;
    let atrading = home.join(".atrading");

    if atrading.exists() {
        eprintln!("~/.atrading/ 已存在，跳过初始化");
        return Ok(());
    }

    std::fs::create_dir_all(&atrading)?;
    std::fs::write(atrading.join("config.toml"), DEFAULT_CONFIG_TOML)?;

    // Create skills dir with default zarili strategy
    let skills_dir = atrading.join("skills").join("zarili");
    std::fs::create_dir_all(&skills_dir)?;
    std::fs::write(
        skills_dir.join("SKILL.md"),
        "---\nname: zarili\ndescription: zarili量价派\n---\n\n# zarili 量价派策略\n\n你是一个A股量价低吸派实战交易者。\n\n## 择时体系\n- 市场温度 > 60 积极做多\n- 市场温度 30-60 谨慎\n- 市场温度 < 30 空仓或极小仓位\n\n## 选股体系\n- 选股池：涨停股、强势股\n- 首板、二板晋级\n- 量价配合良好\n\n## 买卖体系\n- 低吸为主，不追高\n- 分时均线附近建仓\n- 跌破止损位立即离场\n\n## 风控体系\n- 单票仓位不超过 30%\n- 总仓位根据市场温度动态调整\n- 连板晋级率 < 30% 不开新仓",

    )?;

    eprintln!("✅ 已初始化 ~/.atrading/");
    eprintln!("   配置文件: ~/.atrading/config.toml");
    eprintln!("   策略文件: ~/.atrading/skills/zarili/SKILL.md");
    eprintln!("   (可运行 `openatrading skills list` 查看更多可安装技能)");
    Ok(())
}
