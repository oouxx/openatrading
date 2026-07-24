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

/// 每项检查完成后的回调类型。
///
/// 在文本模式下用于流式输出单行结果；JSON 模式下传 `None`，runner 仍会
/// 收集完整 `Vec<CheckResult>` 以便序列化为单个 JSON 数组。
type CheckCallback<'a> = &'a mut dyn FnMut(&dyn Check, &CheckResult);

/// 顺序执行所有检查，返回完整的 `CheckResult` 列表。
///
/// `on_result` 若提供，则在每项检查**完成后立即**回调一次（已填充
/// `duration_ms`），用于文本模式的流式输出。无论是否回调，结果都会被收集
/// 返回——JSON 模式与 `interactive_fix` 都依赖这份完整结果。
async fn run_checks(
    checks: &[Box<dyn Check>],
    ctx: &CheckContext,
    mut on_result: Option<CheckCallback<'_>>,
) -> Vec<CheckResult> {
    let mut results = Vec::with_capacity(checks.len());
    for check in checks {
        let start = std::time::Instant::now();
        let result = check.check(ctx).await;
        let elapsed = start.elapsed();
        let mut r = result;
        r.duration_ms = elapsed.as_millis() as u64;
        if let Some(cb) = on_result.as_deref_mut() {
            cb(check.as_ref(), &r);
        }
        results.push(r);
    }
    results
}

// ── Reporter (Text) ──────────────────────────────────────

/// 检查名称在单行输出中的固定列宽，用于对齐各检查项。
const CHECK_NAME_COL_WIDTH: usize = 12;

/// 渲染单条检查结果的行为单行字符串（图标 + 名称 + 摘要 + 详情）。
///
/// 文本模式的流式输出与（如保留的）批量打印共用此函数，确保格式一致、不发散。
fn format_check_line(check: &dyn Check, result: &CheckResult) -> String {
    let icon = match &result.status {
        CheckStatus::Ok => format!("{}", "✓".green().bold()),
        CheckStatus::Warn(_) => format!("{}", "⚠".yellow().bold()),
        CheckStatus::Error(_) => format!("{}", "✗".red().bold()),
    };
    let name = format!("{}", check.name().bold());
    let details = if result.details.is_empty() {
        String::new()
    } else {
        result.details.join("; ")
    };
    format!(
        " {}  {:<width$}  {}  {}",
        icon,
        name,
        result.summary,
        details.dimmed(),
        width = CHECK_NAME_COL_WIDTH,
    )
}

/// `打印报告头部（标题、版本、平台、AI_HOME`）。
///
/// 在文本模式下于检查开始前先打印，使用户能立即看到运行环境；检查结果随后
/// 流式追加。
fn print_report_header(atrading_home: &std::path::Path) {
    println!();
    println!("{}", " OpenATrading 诊断报告 ".bold());
    println!("{}", "════════════════════════════════════".bold());
    println!("  版本: {}", env!("GIT_VERSION").bold());
    println!(
        "  平台: {}/{}",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    println!("  AI_HOME: {}", atrading_home.display());
    println!();
}

/// 打印报告尾部（分隔线 + 汇总行）。
fn print_report_footer(results: &[CheckResult]) {
    let (ok, warn, err) = status_summary(results);
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
    // 保留原始检查序号（1 基），使修复提示的编号与报告一致，而非过滤后的
    // 连续编号。
    let fixable: Vec<_> = checks
        .iter()
        .enumerate()
        .zip(results.iter())
        .filter(|((_, check), result)| {
            check.has_fix() && matches!(result.status, CheckStatus::Warn(_) | CheckStatus::Error(_))
        })
        .map(|((i, check), result)| (i, check, result))
        .collect();

    if fixable.is_empty() {
        println!("{}", "没有需要修复的项目".green());
        return;
    }

    for (original_index, check, _result) in &fixable {
        let fix_label = check.fix_label();
        let label = fix_label.as_deref().unwrap_or("执行修复");

        print!(
            "是否修复 #{} ({})？[{}] [y/N]: ",
            original_index + 1,
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

    // JSON 模式：收集全部结果后一次性输出单个 JSON 数组，不流式。
    // 文本模式：先打印头部，再以流式回调逐项输出单行结果，最后打印尾部汇总。
    let results = if json {
        let results = run_checks(&checks, &ctx, None).await;
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
        results
    } else {
        print_report_header(&ctx.atrading_home);
        // 流式回调：每项检查完成后立即打印单行结果。`cb` 必须与 `run_checks`
        // 同一作用域声明，使其借用覆盖整个 await。
        let mut cb = |check: &dyn Check, result: &CheckResult| {
            println!("{}", format_check_line(check, result));
        };
        let results = run_checks(&checks, &ctx, Some(&mut cb as CheckCallback<'_>)).await;
        print_report_footer(&results);
        results
    };

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
