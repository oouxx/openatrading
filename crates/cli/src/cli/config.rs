use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use trending_agent_core::roles::{MergeMode, RoleConfig, BUILTIN_STRATEGIES};

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub default_role: Option<String>,
    pub roles: Vec<RoleEntry>,
    pub service: ServiceConfig,
    pub cron: Option<CronTomlConfig>,
    pub social: Option<SocialConfig>,
}

#[derive(Debug, Clone)]
pub struct RoleEntry {
    pub config: RoleConfig,
    /// resolved absolute path for `dir`
    pub strategy_base: PathBuf,
}

#[derive(Debug, Deserialize)]
struct RawConfig {
    default_role: Option<String>,
    #[serde(default)]
    roles: Vec<RoleConfig>,
    #[serde(default)]
    service: Option<ServiceConfig>,
    #[serde(default)]
    cron: Option<CronTomlConfig>,
    #[serde(default)]
    social: Option<SocialConfig>,
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
}

fn default_true() -> bool {
    true
}

/// Substitute `${VAR}` patterns with environment variable values
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
                }
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
    "*/30 9-15 * * 1-5".to_string()
}
fn default_decision_schedule() -> String {
    "0 0 * * 1-5".to_string()
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
    pub webhook_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SlackConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub webhook_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LarkConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub webhook_url: Option<String>,
}

const DEFAULT_CONFIG_TOML: &str = r##"# ── OpenATrading 配置 ──────────────────────────────────
# 优先级：项目根 config.toml > 项目根/.atrading/config.toml > ~/.atrading/config.toml
# 同名角色：高优先级完整覆盖低优先级

default_role = "zarili"

[[roles]]
name = "zarili"
display_name = "zarili量价派"
dir = "strategies/zarili"
merge_mode = "supplement"

[[roles]]
name = "trader"
display_name = "游资操盘手"
dir = "strategies/trader"
merge_mode = "base_only"

[[roles]]
name = "quant"
display_name = "量化分析师"
dir = "strategies/quant"
merge_mode = "supplement"

[[roles]]
name = "risk_officer"
display_name = "风控官"
dir = "strategies/risk_officer"
merge_mode = "supplement"

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

[social.telegram]
enabled = true
bot_polling = true
bot_token = "${TELEGRAM_BOT_TOKEN}"
chat_id = "${TELEGRAM_CHAT_ID}"

# [social.discord]
# enabled = true
# webhook_url = "${DISCORD_WEBHOOK_URL}"

# [social.slack]
# enabled = true
# webhook_url = "${SLACK_WEBHOOK_URL}"

# [social.lark]
# enabled = true
# webhook_url = "${LARK_WEBHOOK_URL}"
"##;

impl AppConfig {
    /// 按三层优先级加载并合并配置
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
        };

        // Layer 1 (lowest): ~/.atrading/config.toml
        if let Some(cfg) = Self::load_from(&home_atrading.join("config.toml"), &home_atrading) {
            merged.merge(cfg);
        }

        // Layer 2 (medium): <project>/.atrading/config.toml
        if let Some(cfg) = Self::load_from(&project_atrading.join("config.toml"), &project_atrading)
        {
            merged.merge(cfg);
        }

        // Layer 3 (highest): <project>/config.toml
        if let Some(cfg) = Self::load_from(&project_root.join("config.toml"), &project_atrading) {
            merged.merge(cfg);
        }

        Ok(merged)
    }

    fn load_from(config_path: &Path, atrading_dir: &Path) -> Option<AppConfig> {
        let content = match std::fs::read_to_string(config_path) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
            Err(e) => {
                eprintln!("跳过 {}: {e}", config_path.display());
                return None;
            }
        };
        // Substitute ${VAR} with environment variables
        let content = substitute_env(&content);
        let raw: RawConfig = match toml::from_str(&content) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("跳过 {} (解析错误): {e}", config_path.display());
                return None;
            }
        };
        let roles = raw
            .roles
            .into_iter()
            .map(|r| {
                let strategy_base = atrading_dir.join(&r.dir);
                RoleEntry {
                    config: r,
                    strategy_base,
                }
            })
            .collect();
        Some(AppConfig {
            default_role: raw.default_role,
            roles,
            service: raw.service.unwrap_or(ServiceConfig {
                db_path: default_db_path(),
            }),
            cron: raw.cron,
            social: raw.social,
        })
    }

    fn merge(&mut self, other: AppConfig) {
        if other.default_role.is_some() {
            self.default_role = other.default_role;
        }
        // Merge service (higher priority overwrites non-default values)
        self.service = other.service;
        // Merge cron
        if other.cron.is_some() {
            self.cron = other.cron;
        }
        // Merge social
        if other.social.is_some() {
            self.social = other.social;
        }
        for entry in other.roles {
            self.roles.retain(|e| e.config.name != entry.config.name);
            self.roles.push(entry);
        }
    }

    pub fn find_role(&self, name: &str) -> Option<&RoleEntry> {
        self.roles.iter().find(|e| e.config.name == name)
    }

    /// 按 merge_mode 从 strategy_base/base.md 和 supplement.md 组装策略文本
    pub fn load_strategy(role: &RoleEntry) -> Result<String> {
        let base_path = role.strategy_base.join("base.md");
        let supplement_path = role.strategy_base.join("supplement.md");

        match role.config.merge_mode {
            MergeMode::BaseOnly => {
                let base = std::fs::read_to_string(&base_path)
                    .with_context(|| format!("无法读取 base.md: {}", base_path.display()))?;
                Ok(base)
            }
            MergeMode::Supplement => {
                let base = std::fs::read_to_string(&base_path)
                    .with_context(|| format!("无法读取 base.md: {}", base_path.display()))?;
                let supplement = std::fs::read_to_string(&supplement_path).unwrap_or_default();
                let trimmed = supplement.trim();
                if trimmed.is_empty() {
                    Ok(base)
                } else {
                    Ok(format!("{base}\n\n{supplement}"))
                }
            }
            MergeMode::Override => {
                let supplement = std::fs::read_to_string(&supplement_path).with_context(|| {
                    format!("无法读取 supplement.md: {}", supplement_path.display())
                })?;
                Ok(supplement)
            }
        }
    }

    /// 列出所有可用角色
    pub fn list_roles(&self) {
        println!("{:<20} {:<20} {:<30}", "name", "display_name", "dir");
        println!("{}", "-".repeat(72));
        for entry in &self.roles {
            println!(
                "{:<20} {:<20} {:<30}",
                entry.config.name, entry.config.display_name, entry.config.dir
            );
        }
        if let Some(ref default) = self.default_role {
            println!("\ndefault_role: {default}");
        }
    }
}

/// `trending-agent init` — 在 HOME 目录创建 ~/.atrading/
pub fn cmd_init() -> Result<()> {
    let home = dirs::home_dir().context("无法获取 HOME 目录")?;
    let atrading = home.join(".atrading");

    if atrading.exists() {
        eprintln!("~/.atrading/ 已存在，跳过初始化");
        return Ok(());
    }

    // 写入 config.toml
    std::fs::create_dir_all(&atrading)?;
    std::fs::write(atrading.join("config.toml"), DEFAULT_CONFIG_TOML)?;

    // 写入内置策略 base.md 和空 supplement.md
    for (name, _display, content) in BUILTIN_STRATEGIES {
        let role_dir = atrading.join("strategies").join(name);
        std::fs::create_dir_all(&role_dir)?;
        std::fs::write(role_dir.join("base.md"), content)?;
        std::fs::write(
            role_dir.join("supplement.md"),
            "# 在此文件补充或覆盖 base.md 中的策略\n",
        )?;
    }

    eprintln!("✅ 已初始化 ~/.atrading/");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use trending_agent_core::roles::MergeMode;

    #[test]
    fn test_merge_role_override() {
        let role_a = RoleConfig {
            name: "trader".into(),
            display_name: "A".into(),
            dir: "strategies/a".into(),
            merge_mode: MergeMode::BaseOnly,
        };
        let role_b = RoleConfig {
            name: "trader".into(),
            display_name: "B".into(),
            dir: "strategies/b".into(),
            merge_mode: MergeMode::Supplement,
        };

        let mut cfg = AppConfig {
            default_role: None,
            roles: vec![RoleEntry {
                config: role_a,
                strategy_base: PathBuf::from("/tmp/a"),
            }],
            service: ServiceConfig {
                db_path: default_db_path(),
            },
            cron: None,
            social: None,
        };
        cfg.merge(AppConfig {
            default_role: Some("trader".into()),
            roles: vec![RoleEntry {
                config: role_b,
                strategy_base: PathBuf::from("/tmp/b"),
            }],
            service: ServiceConfig {
                db_path: default_db_path(),
            },
            cron: None,
            social: None,
        });

        assert_eq!(cfg.default_role.as_deref(), Some("trader"));
        assert_eq!(cfg.roles.len(), 1);
        assert_eq!(cfg.roles[0].config.display_name, "B");
    }
}
