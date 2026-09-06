//! 定时自动化任务(Codex 桌面端 Scheduled Tasks / Automations 的复刻)。
//!
//! - [`model`] — 领域类型:调度计划、运行范围(独立会话 / 复用会话)
//! - [`store`] — SQLite 持久化:任务定义 + 运行记录(收件箱语义)
//! - [`runner`] — 调度运行时:注册 cron job、触发执行、结果入收件箱
//!
//! 收件箱语义与 Codex 对齐:运行有产出 → 未读入收件箱并通知;
//! 无产出 → 自动归档、不打扰。

mod builtin;
mod model;
mod runner;
mod store;
pub mod tool;

pub use builtin::CronConfig;
pub use model::{
    validate, Automation, AutomationId, AutomationPatch, NewAutomation, NewRun, RunStatus,
    Schedule, Scope, SessionId,
};
pub use runner::AutomationHub;
pub use store::{AutomationRun, AutomationStore, RunListQuery};
pub use tool::automation_tools;
