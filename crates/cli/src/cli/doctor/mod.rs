use std::io::Write;
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

#[derive(Debug)]
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

async fn run_checks(checks: &[Box<dyn Check>], ctx: &CheckContext) -> Vec<CheckResult> {
    let mut results = Vec::with_capacity(checks.len());
    for check in checks {
        let start = std::time::Instant::now();
        let result = check.check(ctx).await;
        let elapsed = start.elapsed();
        let mut r = result;
        r.duration_ms = elapsed.as_millis() as u64;
        results.push(r);
    }
    results
}

// ── Reporter (Text) ──────────────────────────────────────

fn print_report(checks: &[Box<dyn Check>], results: &[CheckResult]) {
    let (ok, warn, err) = status_summary(results);
    println!();
    println!("{}", " OpenATrading 诊断报告 ".bold());
    println!("{}", "════════════════════════════════════".bold());
    println!("  版本: {}", env!("GIT_VERSION").bold());
    println!(
        "  平台: {}/{}",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    println!();

    for (check, r) in checks.iter().zip(results.iter()) {
        let icon = match &r.status {
            CheckStatus::Ok => format!("{}", "✓".green().bold()),
            CheckStatus::Warn(_) => format!("{}", "⚠".yellow().bold()),
            CheckStatus::Error(_) => format!("{}", "✗".red().bold()),
        };
        let name = format!("{}", check.name().bold());
        let details = if r.details.is_empty() {
            String::new()
        } else {
            r.details.join("; ")
        };
        println!(
            " {}  {:12}  {}  {}",
            icon,
            name,
            r.summary,
            details.dimmed()
        );
    }

    println!();
    println!("{}", "════════════════════════════════════".bold());

    if err == 0 && warn == 0 {
        println!("{}", "✓ 一切正常".green().bold());
    } else if err == 0 {
        println!(
            "{}",
            format!("⚠ {ok} 项正常，{warn} 项警告").yellow().bold()
        );
        println!("  建议运行 `openatrading doctor --fix` 修复警告项");
    } else {
        println!(
            "{}",
            format!("✗ {ok} 项正常，{warn} 项警告，{err} 项错误")
                .red()
                .bold()
        );
        println!("  建议运行 `openatrading doctor --fix` 尝试自动修复");
    }
    println!();
}

// ── Reporter (JSON) ─────────────────────────────────────

// JSON output is produced inline in `cmd_doctor` via the local `JsonOutput`
// struct, which has access to the check id/name from the registry.

// ── Interactive fix ──────────────────────────────────────

/// Decision returned by [`confirm_fix`] for whether to run a fix.
///
/// Extracted as a pure function so the risk-gating contract is unit-testable
/// without driving stdin.
#[derive(Debug, PartialEq, Eq)]
enum FixDecision {
    /// Proceed and run the fix.
    Proceed,
    /// User declined; skip this check.
    Skip,
}

/// Decide whether to run a fix based on its risk level and the user's input.
///
/// - `Safe`: proceed only on an affirmative `y`/`yes` (case-insensitive).
/// - `Impossible` is filtered out by `has_fix()` before reaching here.
fn confirm_fix(risk: FixRisk, _id: &str, input: &str) -> FixDecision {
    match risk {
        FixRisk::Safe => {
            if input.trim().eq_ignore_ascii_case("y") || input.trim().eq_ignore_ascii_case("yes") {
                FixDecision::Proceed
            } else {
                FixDecision::Skip
            }
        },
        FixRisk::Impossible => FixDecision::Skip,
    }
}

async fn interactive_fix(checks: &[Box<dyn Check>], ctx: &CheckContext, results: &[CheckResult]) {
    let fixable: Vec<_> = checks
        .iter()
        .zip(results.iter())
        .filter(|(check, result)| {
            check.has_fix() && matches!(result.status, CheckStatus::Warn(_) | CheckStatus::Error(_))
        })
        .collect();

    if fixable.is_empty() {
        println!("{}", "没有需要修复的项目".green());
        return;
    }

    for (i, (check, _result)) in fixable.iter().enumerate() {
        let fix_label = check.fix_label();
        let label = fix_label.as_deref().unwrap_or("执行修复");

        print!(
            "是否修复 #{} ({})？[{}] [y/N]: ",
            i + 1,
            check.name(),
            label
        );
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
                    // Re-check
                    let new_result = check.check(ctx).await;
                    let icon = match &new_result.status {
                        CheckStatus::Ok => format!("{}", "✓".green().bold()),
                        CheckStatus::Warn(_) => format!("{}", "⚠".yellow().bold()),
                        CheckStatus::Error(_) => format!("{}", "✗".red().bold()),
                    };
                    println!("    复查: {} {}", icon, new_result.summary);
                },
                FixStatus::Skipped => {
                    println!("  {} {}", "☐".dimmed(), fix_result.message);
                },
            },
            // A fix that fails (expected or unexpected) surfaces here as an
            // error message; `FixStatus` only distinguishes success vs
            // no-op since failures propagate via `Result`.
            Err(e) => {
                println!("  {} 修复失败: {e:#}", "✗".red().bold());
            },
        }
    }
}

/// Check ids that require real network I/O. Skipped under `--skip-network`.
const NETWORK_CHECK_IDS: &[&str] = &["telegram", "llm", "mcp"];

// ── Entry point ─────────────────────────────────────────

pub async fn cmd_doctor(json: bool, fix: bool, skip_network: bool) -> Result<()> {
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
    let results = run_checks(&checks, &ctx).await;

    if json {
        #[derive(Serialize)]
        struct JsonOutput {
            version: &'static str,
            items: Vec<FlatCheckResult>,
        }
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
        print_report(&checks, &results);
    }

    if fix {
        interactive_fix(&checks, &ctx, &results).await;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── confirm_fix risk gating ──────────────────────────

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
