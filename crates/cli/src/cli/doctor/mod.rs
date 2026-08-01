use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Result;
use async_trait::async_trait;
use owo_colors::OwoColorize;
use serde::Serialize;

use crate::cli::config::AppConfig;

// ── Sub-modules ─────────────────────────────────────────

mod check_config;
mod check_cron;
mod check_env;
mod check_home;
mod check_llm;
mod check_mcp;
mod check_port;
mod check_skills;
mod check_sqlite;
mod check_telegram;

// ── Types ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub enum CheckStatus {
    Ok,
    #[serde(rename = "warn")]
    Warn(String),
    #[serde(rename = "error")]
    Error(String),
}

#[derive(Debug, Clone, Serialize)]
pub struct CheckResult {
    pub status: CheckStatus,
    pub summary: String,
    pub details: Vec<String>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Copy)]
pub enum FixRisk {
    Safe,
    Impossible,
}

#[derive(Debug)]
pub enum FixStatus {
    Fixed,
    Skipped,
}

#[derive(Debug)]
pub struct FixResult {
    pub status: FixStatus,
    pub message: String,
}

pub struct CheckContext {
    pub atrading_home: PathBuf,
    pub app_config: Option<AppConfig>,
}

// ── Check trait ──────────────────────────────────────────

#[async_trait]
pub trait Check: Send + Sync {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn has_fix(&self) -> bool;
    fn fix_risk(&self) -> FixRisk;
    fn fix_label(&self) -> Option<String>;

    async fn check(&self, ctx: &CheckContext) -> CheckResult;
    async fn fix(&self, ctx: &CheckContext) -> Result<FixResult>;
}

// ── Flat result for JSON output ──────────────────────────

#[derive(Debug, Serialize)]
pub struct FlatCheckResult {
    pub id: &'static str,
    pub name: &'static str,
    pub status: CheckStatus,
    pub summary: String,
    pub details: Vec<String>,
    pub duration_ms: u64,
}

// ── Helper functions ─────────────────────────────────────

fn status_summary(results: &[CheckResult]) -> (usize, usize, usize) {
    let ok = results
        .iter()
        .filter(|r| matches!(r.status, CheckStatus::Ok))
        .count();
    let warn = results
        .iter()
        .filter(|r| matches!(r.status, CheckStatus::Warn(_)))
        .count();
    let err = results
        .iter()
        .filter(|r| matches!(r.status, CheckStatus::Error(_)))
        .count();
    (ok, warn, err)
}

fn format_check_line(check: &dyn Check, result: &CheckResult) -> String {
    let icon = match &result.status {
        CheckStatus::Ok => format!("{}", "✓".green().bold()),
        CheckStatus::Warn(_) => format!("{}", "⚠".yellow().bold()),
        CheckStatus::Error(_) => format!("{}", "✗".red().bold()),
    };
    let detail = match &result.status {
        CheckStatus::Warn(msg) => format!(": {msg}"),
        CheckStatus::Error(msg) => format!(": {msg}"),
        _ => String::new(),
    };
    format!("  {icon} {:<20} {}{detail}", check.name(), result.summary)
}

// ── All checks registry ──────────────────────────────────

fn all_checks() -> Vec<Box<dyn Check>> {
    vec![
        Box::new(check_home::HomeCheck),
        Box::new(check_config::ConfigCheck),
        Box::new(check_env::EnvCheck),
        Box::new(check_sqlite::SqliteCheck),
        Box::new(check_cron::CronCheck),
        Box::new(check_telegram::TelegramCheck),
        Box::new(check_llm::LlmCheck),
        Box::new(check_mcp::McpCheck),
        Box::new(check_skills::SkillsCheck),
        Box::new(check_port::PortCheck),
    ]
}

// ── Runner ───────────────────────────────────────────────

/// 并行执行所有检查，返回完整的 `CheckResult` 列表。
async fn run_checks_parallel(checks: &[Box<dyn Check>], ctx: &CheckContext) -> Vec<CheckResult> {
    use futures::future::join_all;

    let tasks: Vec<_> = checks
        .iter()
        .map(|check| {
            let ctx = CheckContext {
                atrading_home: ctx.atrading_home.clone(),
                app_config: ctx.app_config.clone(),
            };
            async move {
                let start = std::time::Instant::now();
                let result = check.check(&ctx).await;
                let elapsed = start.elapsed();
                let mut r = result;
                r.duration_ms = elapsed.as_millis() as u64;
                (check.id(), r)
            }
        })
        .collect();

    let results = join_all(tasks).await;
    // 按 checks 原始顺序排序
    let mut ordered: Vec<CheckResult> = Vec::with_capacity(checks.len());
    for check in checks {
        if let Some((_, r)) = results.iter().find(|(id, _)| *id == check.id()) {
            ordered.push(r.clone());
        }
    }
    ordered
}

// ── Reporter (Text) ──────────────────────────────────────

fn print_report_header(home: &Path) {
    println!("{}", "═".repeat(50));
    println!("  OpenATrading Doctor  v{}", env!("GIT_VERSION"));
    println!("  {}", home.display());
    println!("{}", "═".repeat(50));
    println!();
}

fn print_report_footer(results: &[CheckResult]) {
    let (ok, warn, err) = status_summary(results);
    println!();
    println!("{}", "─".repeat(50));
    println!(
        "  总计: {}  ✓, {}  ⚠, {}  ✗  ({} 项检查)",
        ok.green().bold(),
        warn.yellow().bold(),
        err.red().bold(),
        results.len()
    );
    println!("{}", "─".repeat(50));
}

/// Check ids that require real network I/O. Skipped under `--skip-network`.
const NETWORK_CHECK_IDS: &[&str] = &["telegram", "llm", "mcp"];

// ── Entry point ─────────────────────────────────────────

pub async fn cmd_doctor(json: bool, fix: bool, skip_network: bool) -> Result<()> {
    #[derive(Serialize)]
    struct JsonOutput {
        version: &'static str,
        items: Vec<FlatCheckResult>,
    }

    let atrading_home = openatrading_core::atrading_home();
    let app_config = AppConfig::load().ok();

    let ctx = CheckContext {
        atrading_home,
        app_config,
    };

    let mut checks = all_checks();
    if skip_network {
        let mut skipped: Vec<&str> = Vec::new();
        checks.retain(|c| {
            let is_network = NETWORK_CHECK_IDS.contains(&c.id());
            if is_network {
                skipped.push(c.id());
            }
            !is_network
        });
        if !skipped.is_empty() && !json {
            println!("  {} 已跳过联网检查: {}", "⤚".dimmed(), skipped.join(", "));
        }
    }

    if json {
        let results = run_checks_parallel(&checks, &ctx).await;
        let items: Vec<FlatCheckResult> = checks
            .iter()
            .zip(results.iter())
            .map(|(check, result)| FlatCheckResult {
                id: check.id(),
                name: check.name(),
                status: result.status.clone(),
                summary: result.summary.clone(),
                details: result.details.clone(),
                duration_ms: result.duration_ms,
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&JsonOutput {
                version: env!("GIT_VERSION"),
                items,
            })?
        );
    } else {
        print_report_header(&ctx.atrading_home);
        let results = run_checks_parallel(&checks, &ctx).await;
        for (check, result) in checks.iter().zip(results.iter()) {
            println!("{}", format_check_line(check.as_ref(), result));
        }
        print_report_footer(&results);

        if fix {
            interactive_fix(&checks, &ctx, &results).await;
        }
    }

    Ok(())
}

// ── Interactive Fix ─────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FixDecision {
    Proceed,
    Skip,
}

fn confirm_fix(risk: FixRisk, _id: &str, input: &str) -> FixDecision {
    if matches!(risk, FixRisk::Impossible) {
        return FixDecision::Skip;
    }
    let trimmed = input.trim().to_lowercase();
    if trimmed == "y" || trimmed == "yes" {
        FixDecision::Proceed
    } else {
        FixDecision::Skip
    }
}

async fn interactive_fix(checks: &[Box<dyn Check>], ctx: &CheckContext, results: &[CheckResult]) {
    println!();
    println!("{}", "═".repeat(50));
    println!("  交互式修复");
    println!("{}", "═".repeat(50));

    for (check, result) in checks.iter().zip(results.iter()) {
        if matches!(result.status, CheckStatus::Ok) {
            continue;
        }
        if !check.has_fix() {
            continue;
        }

        let risk_label = match check.fix_risk() {
            FixRisk::Safe => "安全",
            FixRisk::Impossible => "不可修复",
        };

        println!();
        println!("  {} ({risk_label})", check.name().bold());
        println!("    问题: {}", result.summary);
        if let Some(label) = check.fix_label() {
            print!("  修复: {label} [y/N] ");
        } else {
            print!("  是否尝试修复? [y/N] ");
        }
        std::io::stdout().flush().ok();
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).ok();

        if confirm_fix(check.fix_risk(), check.id(), &input) == FixDecision::Skip {
            println!("  {} 跳过", "☐".dimmed());
            continue;
        }

        match check.fix(ctx).await {
            Ok(fix_result) => match fix_result.status {
                FixStatus::Fixed => {
                    println!("  {} {}", "✓".green().bold(), fix_result.message);
                    let new_result = check.check(ctx).await;
                    let icon = match &new_result.status {
                        CheckStatus::Ok => format!("{}", "✓".green().bold()),
                        CheckStatus::Warn(_) => format!("{}", "⚠".yellow().bold()),
                        CheckStatus::Error(_) => format!("{}", "✗".red().bold()),
                    };
                    println!("    复查: {} {}", icon, new_result.summary);
                }
                FixStatus::Skipped => {
                    println!("  {} {}", "☐".dimmed(), fix_result.message);
                }
            },
            Err(e) => {
                println!("  {} 修复失败: {e:#}", "✗".red().bold());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_fix_proceeds_on_yes() {
        assert_eq!(
            confirm_fix(FixRisk::Safe, "port", "y"),
            FixDecision::Proceed
        );
        assert_eq!(
            confirm_fix(FixRisk::Safe, "port", "YES"),
            FixDecision::Proceed
        );
    }

    #[test]
    fn safe_fix_skips_on_anything_else() {
        assert_eq!(confirm_fix(FixRisk::Safe, "port", "n"), FixDecision::Skip);
        assert_eq!(confirm_fix(FixRisk::Safe, "port", ""), FixDecision::Skip);
    }

    #[test]
    fn impossible_fix_always_skips() {
        assert_eq!(
            confirm_fix(FixRisk::Impossible, "mcp", "mcp"),
            FixDecision::Skip
        );
    }
}
