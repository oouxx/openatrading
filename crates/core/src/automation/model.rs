//! 自动化任务领域类型。
//!
//! 调度计划统一解析为 tokio-cron-scheduler 的 6 字段 cron 表达式
//! (`秒 分 时 日 月 周`,cron crate 的 Quartz 语义:**1 = 周日**)。
//! 与现有 `cron` 模块写法一致(如 `0 0 18 * * 1-5`)。

use serde::{Deserialize, Serialize};

use crate::log;

/// 自动化任务唯一标识。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AutomationId(pub String);

impl AutomationId {
    pub fn generate() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}

impl std::fmt::Display for AutomationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// 会话标识(newtype)。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionId(pub String);

/// 运行范围——对应 Codex 的两种自动化:
/// - `NewThread`:每次全新会话,结果独立入收件箱(standalone automation)
/// - `ExistingThread`:心跳式唤醒同一会话,保留上下文(thread automation)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Scope {
    NewThread,
    ExistingThread { session_id: SessionId },
}

/// 调度计划。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Schedule {
    /// 原生 cron 表达式(5/6/7 字段;5 字段自动补秒位 `0`)。
    Cron { expr: String },
    /// 每 N 分钟一次(1..=1440)。
    EveryMinutes { minutes: u32 },
    /// 每天 `HH:MM`。
    Daily { hour: u8, minute: u8 },
    /// 每周选定日期的 `HH:MM` 运行。`days` 为 ISO 周序号(1=周一 … 7=周日)。
    Weekly { days: Vec<u8>, hour: u8, minute: u8 },
}

/// 计划转 cron 失败。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScheduleError {
    CronFieldCount(usize),
    InvalidInterval(u32),
    InvalidDailyTime { hour: u8, minute: u8 },
    WeeklyNoDays,
    WeeklyDayRange,
}

impl std::fmt::Display for ScheduleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScheduleError::CronFieldCount(n) => {
                write!(f, "cron expression must have 5-7 fields, got {n}")
            }
            ScheduleError::InvalidInterval(n) => {
                write!(f, "every_minutes must be in 1..=1440, got {n}")
            }
            ScheduleError::InvalidDailyTime { hour, minute } => {
                write!(
                    f,
                    "daily time must be HH:MM in 0-23:0-59, got {hour:02}:{minute:02}"
                )
            }
            ScheduleError::WeeklyNoDays => {
                write!(
                    f,
                    "weekly schedule requires at least one day (1=Mon..7=Sun)"
                )
            }
            ScheduleError::WeeklyDayRange => {
                write!(f, "weekly days must be ISO weekday numbers 1..=7")
            }
        }
    }
}

impl std::error::Error for ScheduleError {}

impl Schedule {
    /// 解析为 6 字段 cron 表达式(秒 分 时 日 月 周)。
    ///
    /// # Errors
    /// 字段数非法、间隔越界或每日时间越界时返回 [`ScheduleError`]。
    pub fn to_cron(&self) -> Result<String, ScheduleError> {
        match self {
            Schedule::Cron { expr } => {
                let fields = expr.split_whitespace().count();
                match fields {
                    5 => Ok(format!("0 {expr}")),
                    6 | 7 => Ok(expr.clone()),
                    _ => Err(ScheduleError::CronFieldCount(fields)),
                }
            }
            Schedule::EveryMinutes { minutes: n } => {
                if *n == 0 || *n > 1440 {
                    return Err(ScheduleError::InvalidInterval(*n));
                }
                Ok(format!("0 */{n} * * * *"))
            }
            Schedule::Daily { hour, minute } => {
                if *hour > 23 || *minute > 59 {
                    return Err(ScheduleError::InvalidDailyTime {
                        hour: *hour,
                        minute: *minute,
                    });
                }
                Ok(format!("0 {minute} {hour} * * *"))
            }
            Schedule::Weekly { days, hour, minute } => {
                Self::validate_weekly_days(days)?;
                if *hour > 23 || *minute > 59 {
                    return Err(ScheduleError::InvalidDailyTime {
                        hour: *hour,
                        minute: *minute,
                    });
                }
                // ISO 周序号(1=周一..7=周日)→ Quartz 周序号(1=周日..7=周六)。
                let quartz_days: Vec<u8> = days.iter().map(|d| d % 7 + 1).collect();
                let day_list = quartz_days
                    .iter()
                    .map(std::string::ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(",");
                Ok(format!("0 {minute} {hour} * * {day_list}"))
            }
        }
    }

    /// Weekly days 非空且为合法 ISO 周序号(1..=7)。
    fn validate_weekly_days(days: &[u8]) -> Result<(), ScheduleError> {
        if days.is_empty() {
            return Err(ScheduleError::WeeklyNoDays);
        }
        if days.iter().any(|d| *d == 0 || *d > 7) {
            return Err(ScheduleError::WeeklyDayRange);
        }
        Ok(())
    }
}

/// 自动化任务定义。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Automation {
    pub id: AutomationId,
    pub name: String,
    /// 每次运行注入给 agent 的提示词。
    pub prompt: String,
    pub schedule: Schedule,
    pub scope: Scope,
    /// NewThread 运行的工作目录(项目目录)。
    pub cwd: String,
    pub enabled: bool,
    /// 只读模式:NewThread 会话禁用 bash/edit/write。
    /// ExistingThread 复用既有会话,工具集不受本开关约束。
    pub read_only: bool,
    /// 单次运行超时秒数;`None` = 默认(600s)。
    pub timeout_secs: Option<u64>,
    pub created_at: String,
    pub updated_at: String,
}

/// 单次运行超时默认值(秒)。
pub const DEFAULT_RUN_TIMEOUT_SECS: u64 = 600;

/// 超时上限(24h)。
const MAX_RUN_TIMEOUT_SECS: u64 = 86_400;

/// 新建任务的草稿(id 与时间戳由 store 生成)。
#[derive(Debug, Clone, Deserialize)]
pub struct NewAutomation {
    pub name: String,
    pub prompt: String,
    pub schedule: Schedule,
    pub scope: Scope,
    pub cwd: String,
    #[serde(default)]
    pub read_only: bool,
    #[serde(default)]
    pub timeout_secs: Option<u64>,
}

/// 更新任务的补丁(未提及的字段保持不变)。
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct AutomationPatch {
    pub name: Option<String>,
    pub prompt: Option<String>,
    pub schedule: Option<Schedule>,
    pub scope: Option<Scope>,
    pub cwd: Option<String>,
    pub enabled: Option<bool>,
    pub read_only: Option<bool>,
    pub timeout_secs: Option<u64>,
}

/// 运行状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    /// 运行中(start_run 落库,finish_run 收尾;进程中断由 reap 兜底)。
    Running,
    Success,
    Failed,
}

impl RunStatus {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            RunStatus::Running => "running",
            RunStatus::Success => "success",
            RunStatus::Failed => "failed",
        }
    }

    pub(super) fn from_str(s: &str) -> Option<Self> {
        match s {
            "running" => Some(RunStatus::Running),
            "success" => Some(RunStatus::Success),
            "failed" => Some(RunStatus::Failed),
            _ => None,
        }
    }
}

/// 一次待落库的运行结果。
#[derive(Debug, Clone)]
pub struct NewRun {
    pub automation_id: AutomationId,
    pub status: RunStatus,
    /// 最终助手回复;`None` 或空白视为"无可报告内容"。
    pub output: Option<String>,
    pub error: Option<String>,
}

/// 校验新建/更新时的业务约束,返回错误信息(面向 API 调用方)。
pub fn validate(
    name: &str,
    prompt: &str,
    schedule: &Schedule,
    scope: &Scope,
    timeout_secs: Option<u64>,
) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("name must not be empty".into());
    }
    if prompt.trim().is_empty() {
        return Err("prompt must not be empty".into());
    }
    if let Err(e) = schedule.to_cron() {
        return Err(e.to_string());
    }
    if matches!(scope, Scope::ExistingThread { session_id } if session_id.0.trim().is_empty()) {
        return Err("existing_thread scope requires a non-empty session_id".into());
    }
    if let Some(t) = timeout_secs {
        if t == 0 || t > MAX_RUN_TIMEOUT_SECS {
            return Err(format!(
                "timeout_secs must be in 1..={MAX_RUN_TIMEOUT_SECS}, got {t}"
            ));
        }
    }
    Ok(())
}

/// 通知正文截断上限,避免长回复刷屏。
const NOTIFY_OUTPUT_LIMIT: usize = 500;

/// 组装通知文本(有产出时)。
pub fn notification_text(automation: &Automation, output: &str) -> String {
    let trimmed = output.trim();
    let body: String = if trimmed.chars().count() > NOTIFY_OUTPUT_LIMIT {
        let head: String = trimmed.chars().take(NOTIFY_OUTPUT_LIMIT).collect();
        log::debug!("[automation] notification output truncated to {NOTIFY_OUTPUT_LIMIT} chars");
        format!("{head} …")
    } else {
        trimmed.to_string()
    };
    format!("🔔 [自动化] {}\n{body}", automation.name)
}

/// 组装失败通知文本。
pub fn failure_text(automation: &Automation, error: &str) -> String {
    format!(
        "❌ [自动化] {} 运行失败\n{}",
        automation.name,
        truncate_for_notify(error.trim())
    )
}

fn truncate_for_notify(text: &str) -> String {
    if text.chars().count() > NOTIFY_OUTPUT_LIMIT {
        let head: String = text.chars().take(NOTIFY_OUTPUT_LIMIT).collect();
        format!("{head} …")
    } else {
        text.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_minutes_to_cron() {
        assert_eq!(
            Schedule::EveryMinutes { minutes: 5 }
                .to_cron()
                .map_err(|e| e.to_string()),
            Ok("0 */5 * * * *".to_string())
        );
    }

    #[test]
    fn daily_to_cron() {
        assert_eq!(
            Schedule::Daily {
                hour: 9,
                minute: 30
            }
            .to_cron()
            .map_err(|e| e.to_string()),
            Ok("0 30 9 * * *".to_string())
        );
    }

    #[test]
    fn cron_passthrough_six_fields() {
        let s = Schedule::Cron {
            expr: "0 0 18 * * 1-5".to_string(),
        };
        assert_eq!(
            s.to_cron().map_err(|e| e.to_string()),
            Ok("0 0 18 * * 1-5".to_string())
        );
    }

    #[test]
    fn cron_five_fields_gets_seconds_prefix() {
        let s = Schedule::Cron {
            expr: "30 9 * * *".to_string(),
        };
        assert_eq!(
            s.to_cron().map_err(|e| e.to_string()),
            Ok("0 30 9 * * *".to_string())
        );
    }

    #[test]
    fn invalid_cron_field_count_rejected() {
        let s = Schedule::Cron {
            expr: "* * *".to_string(),
        };
        assert!(s.to_cron().is_err());
    }

    #[test]
    fn invalid_interval_rejected() {
        assert!(Schedule::EveryMinutes { minutes: 0 }.to_cron().is_err());
        assert!(Schedule::EveryMinutes { minutes: 1441 }.to_cron().is_err());
    }

    #[test]
    fn invalid_daily_time_rejected() {
        assert!(Schedule::Daily {
            hour: 24,
            minute: 0
        }
        .to_cron()
        .is_err());
        assert!(Schedule::Daily {
            hour: 8,
            minute: 60
        }
        .to_cron()
        .is_err());
    }

    #[test]
    fn existing_thread_requires_session_id() {
        let err = validate(
            "n",
            "p",
            &Schedule::EveryMinutes { minutes: 5 },
            &Scope::NewThread,
            None,
        );
        assert!(err.is_ok());

        let err = validate(
            "n",
            "p",
            &Schedule::EveryMinutes { minutes: 5 },
            &Scope::ExistingThread {
                session_id: SessionId("  ".to_string()),
            },
            None,
        );
        assert!(err.is_err());

        let err = validate(
            "",
            "p",
            &Schedule::EveryMinutes { minutes: 5 },
            &Scope::NewThread,
            None,
        );
        assert!(err.is_err());

        let err = validate(
            "n",
            " ",
            &Schedule::EveryMinutes { minutes: 5 },
            &Scope::NewThread,
            None,
        );
        assert!(err.is_err());
    }

    #[test]
    fn scope_serde_roundtrip() {
        let scope = Scope::ExistingThread {
            session_id: SessionId("abc".to_string()),
        };
        let json = serde_json::to_string(&scope).unwrap_or_default();
        let back: Scope = serde_json::from_str(&json).unwrap_or_else(|e| panic!("roundtrip: {e}"));
        assert_eq!(back, scope);
    }

    #[test]
    fn notification_text_truncates_long_output() {
        let automation = Automation {
            id: AutomationId("a".into()),
            name: "每日简报".into(),
            read_only: false,
            timeout_secs: None,
            prompt: "p".into(),
            schedule: Schedule::Daily { hour: 9, minute: 0 },
            scope: Scope::NewThread,
            cwd: "/tmp".into(),
            enabled: true,
            created_at: String::new(),
            updated_at: String::new(),
        };
        let long = "x".repeat(2000);
        let text = notification_text(&automation, &long);
        assert!(text.contains("…"));
        assert!(text.chars().count() < 600);
    }
}
