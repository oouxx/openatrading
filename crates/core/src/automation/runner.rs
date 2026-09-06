//! 自动化调度运行时:单个 `JobScheduler` 同时驱动内置任务(采集/推送)
//! 与用户自定义自动化。

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use anyhow::Result;
use chrono_tz::Asia::Shanghai;
use tokio::sync::Mutex;
use tokio_cron_scheduler::{Job, JobScheduler};
use uuid::Uuid;

use crate::context::AppContext;
use crate::llm::{find_session_file_by_id, SessionHandle, SessionPool};
use pi_coding_agent::core::agent_session::PromptOptions;

use super::builtin::{register_builtin, CronConfig};
use super::model::{
    failure_text, notification_text, Automation, AutomationId, NewRun, RunStatus, Scope,
    DEFAULT_RUN_TIMEOUT_SECS,
};
use super::store::AutomationStore;

/// 注册一个 cron job;计划表达式非法时记日志并跳过(不 panic)。
macro_rules! add_job {
    ($scheduler:expr, $schedule:expr, $name:expr, $task:expr) => {
        match Job::new_async_tz($schedule.as_str(), Shanghai, $task) {
            Ok(j) => {
                $scheduler.add(j).await?;
            }
            Err(e) => {
                crate::log::error!(
                    "[automation] {}: invalid schedule '{}': {e}",
                    $name,
                    $schedule
                );
            }
        }
    };
}
pub(crate) use add_job;

/// 自动化调度器中枢。
///
/// 生命周期:`new`(store)→ `attach`(AppContext)→ `start`(幂等)。
/// CRUD 后调用 [`AutomationHub::reload`] 热更新任务;未启动时 reload
/// 只更新内部映射,下次 `start` 生效。
pub struct AutomationHub {
    store: Arc<AutomationStore>,
    ctx: std::sync::OnceLock<Arc<AppContext>>,
    scheduler: Mutex<Option<JobScheduler>>,
    /// 用户自动化 job 映射:automation_id → scheduler job id。
    jobs: Mutex<HashMap<AutomationId, Uuid>>,
    /// 最近一次 reload 时的任务表指纹(reconcile 用)。
    fingerprint: Mutex<Option<String>>,
    /// 正在运行的 automation 集合(并发保护:同一任务同时只有一个 run)。
    running: Mutex<HashSet<AutomationId>>,
}

impl AutomationHub {
    pub fn new(store: Arc<AutomationStore>) -> Arc<Self> {
        Arc::new(Self {
            store,
            ctx: std::sync::OnceLock::new(),
            scheduler: Mutex::new(None),
            jobs: Mutex::new(HashMap::new()),
            fingerprint: Mutex::new(None),
            running: Mutex::new(HashSet::new()),
        })
    }

    /// 注入运行上下文;重复调用时首次生效(进程内只允许一个调度器实例)。
    pub fn attach(&self, ctx: Arc<AppContext>) {
        let _ = self.ctx.set(ctx);
    }

    pub(super) fn ctx(&self) -> Result<Arc<AppContext>> {
        self.ctx
            .get()
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("automation hub not attached to an AppContext"))
    }

    /// 任务存储(CRUD / 收件箱查询)。
    pub fn store(&self) -> &AutomationStore {
        &self.store
    }

    /// 供 builtin 模块注册任务用的调度器句柄。
    pub(super) async fn scheduler(&self) -> Result<JobScheduler> {
        self.scheduler
            .lock()
            .await
            .clone()
            .ok_or_else(|| anyhow::anyhow!("automation scheduler not started"))
    }

    /// 启动调度器(幂等,重复调用为 no-op)。
    ///
    /// `builtin` 为 `Some` 时同时注册内置采集/推送任务(仅 gateway 进程)。
    ///
    /// # Errors
    /// 调度器创建/启动失败或任务注册失败。
    pub async fn start(self: &Arc<Self>, builtin: Option<CronConfig>) -> Result<()> {
        {
            let mut guard = self.scheduler.lock().await;
            if guard.is_none() {
                let scheduler = JobScheduler::new().await?;
                scheduler.start().await?;
                *guard = Some(scheduler);
                crate::log::info!("[automation] scheduler started (tz=Asia/Shanghai)");
            }
        }
        if let Some(cfg) = builtin.filter(|c| c.enabled) {
            register_builtin(self, cfg).await?;
        }
        self.reload().await?;

        // Reconcile:每分钟比对任务表指纹,感知跨进程(tauri)的 CRUD。
        let watcher = Arc::clone(self);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                interval.tick().await;
                if let Err(e) = watcher.store.reap_stale_runs(86_400) {
                    crate::log::warn!("[automation] reap stale runs: {e}");
                }
                if watcher.reconcile_tick().await {
                    if let Err(e) = watcher.reload().await {
                        crate::log::warn!("[automation] reconcile reload failed: {e}");
                    }
                }
            }
        });
        Ok(())
    }

    /// 比对任务表指纹;有变化返回 true(需要 reload)。
    async fn reconcile_tick(&self) -> bool {
        let Ok(fp) = self.store.fingerprint() else {
            return false;
        };
        let current = self.fingerprint.lock().await.clone();
        current.as_deref() != Some(fp.as_str())
    }

    /// 重新加载用户自动化(增删改后调用),热更新 job 列表。
    ///
    /// 单个任务的表达式非法只跳过该任务并记日志,不影响其余任务。
    ///
    /// # Errors
    /// 读取任务列表或调度器操作失败。
    pub async fn reload(self: &Arc<Self>) -> Result<()> {
        let scheduler = self.scheduler.lock().await.clone();
        let Some(scheduler) = scheduler else {
            // 未启动:仅依赖 store 的持久化,启动时会重新加载。
            return Ok(());
        };

        // 先移除全部旧 job,再按当前 store 状态重建。
        let old_jobs: Vec<Uuid> = {
            let mut jobs = self.jobs.lock().await;
            jobs.drain().map(|(_, job_id)| job_id).collect()
        };
        for job_id in old_jobs {
            if let Err(e) = scheduler.remove(&job_id).await {
                crate::log::warn!("[automation] remove stale job {job_id}: {e}");
            }
        }

        *self.fingerprint.lock().await = self.store.fingerprint().ok();

        let mut jobs = self.jobs.lock().await;
        for automation in self.store.list()?.into_iter().filter(|a| a.enabled) {
            let cron_expr = match automation.schedule.to_cron() {
                Ok(expr) => expr,
                Err(e) => {
                    crate::log::error!(
                        "[automation] {}: invalid schedule, skipped: {e}",
                        automation.name
                    );
                    continue;
                }
            };
            let hub = Arc::clone(self);
            let automation_for_job = automation.clone();
            let task = move |_uuid: Uuid, _lock: JobScheduler| {
                let hub = Arc::clone(&hub);
                let automation = automation_for_job.clone();
                Box::pin(async move {
                    hub.run_once(automation).await;
                })
                    as std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>
            };
            match Job::new_async_tz(&cron_expr, Shanghai, task) {
                Ok(job) => {
                    let job_id = scheduler.add(job).await?;
                    crate::log::info!(
                        "[automation] scheduled '{}' ({}) as {cron_expr}",
                        automation.name,
                        automation.id
                    );
                    jobs.insert(automation.id.clone(), job_id);
                }
                Err(e) => {
                    crate::log::error!(
                        "[automation] {}: invalid schedule '{cron_expr}': {e}",
                        automation.name
                    );
                }
            }
        }
        Ok(())
    }

    /// 手动立即触发一次(异步执行,不等待完成)。
    ///
    /// # Errors
    /// 任务不存在,或该任务已有一次运行正在进行。
    pub async fn trigger_now(self: &Arc<Self>, id: &AutomationId) -> Result<()> {
        let automation = self
            .store
            .get(id)?
            .ok_or_else(|| anyhow::anyhow!("automation {id} not found"))?;
        if !self.try_claim(&automation.id).await {
            anyhow::bail!("automation {} is already running", automation.name);
        }
        // run_once 内部负责释放占位;这里先把占位传出去。
        let hub = Arc::clone(self);
        tokio::spawn(async move {
            hub.run_once(automation).await;
        });
        Ok(())
    }

    /// 并发占位:同一任务同时只允许一个 run。
    async fn try_claim(&self, id: &AutomationId) -> bool {
        self.running.lock().await.insert(id.clone())
    }

    async fn release(&self, id: &AutomationId) {
        self.running.lock().await.remove(id);
    }

    /// 执行一次运行并落库(调度触发与手动触发共用)。
    /// 调用前须已 [`AutomationHub::try_claim`];结束时负责释放。
    pub(super) async fn run_once(self: &Arc<Self>, automation: Automation) {
        let ctx = match self.ctx() {
            Ok(ctx) => ctx,
            Err(e) => {
                crate::log::error!("[automation] {}: {e}", automation.name);
                self.release(&automation.id).await;
                return;
            }
        };

        let run_id = match self.store.start_run(&automation.id) {
            Ok(id) => id,
            Err(e) => {
                crate::log::error!("[automation] {}: failed to start run: {e}", automation.name);
                self.release(&automation.id).await;
                return;
            }
        };

        let timeout_secs = automation.timeout_secs.unwrap_or(DEFAULT_RUN_TIMEOUT_SECS);
        let execution = execute(&automation, &ctx.pool, timeout_secs).await;
        let (status, output, error) = match execution.result {
            Ok(text) => (RunStatus::Success, Some(text), None),
            Err(e) => {
                crate::log::warn!("[automation] {}: run failed: {e}", automation.name);
                (RunStatus::Failed, None, Some(e))
            }
        };

        let run = NewRun {
            automation_id: automation.id.clone(),
            status,
            output: output.clone(),
            error: error.clone(),
        };
        if let Err(e) = self.store.finish_run(run_id, &run) {
            crate::log::error!(
                "[automation] {}: failed to finish run #{run_id}: {e}",
                automation.name
            );
        }

        // 通知语义:有产出(发现)+ 失败都要提醒;空产出静默归档。
        let message = match (&status, output.as_deref(), error.as_deref()) {
            (RunStatus::Success, Some(findings), _) if !findings.trim().is_empty() => {
                Some(notification_text(&automation, findings))
            }
            (RunStatus::Failed, _, Some(err)) => Some(failure_text(&automation, err)),
            _ => None,
        };
        if let Some(message) = message {
            if let Err(e) = ctx.social.send_text(&message).await {
                crate::log::warn!("[automation] {}: notify failed: {e}", automation.name);
            }
        }

        // NewThread 会话是即用即弃的,清掉磁盘 JSONL,避免污染会话列表。
        if let Some(session_id) = execution.session_id {
            if let Err(e) = self.discard_session(&ctx.session_dir, &session_id).await {
                crate::log::warn!(
                    "[automation] {}: discard session file: {e}",
                    automation.name
                );
            }
        }

        self.release(&automation.id).await;
        crate::log::info!("[automation] {} run #{run_id} {status:?}", automation.name);
    }

    /// 删除一次性会话的磁盘文件(仅记日志,失败不影响运行结果)。
    async fn discard_session(&self, session_dir: &str, session_id: &str) -> Result<()> {
        let Some(path) = find_session_file_by_id(session_dir, session_id) else {
            return Ok(());
        };
        std::fs::remove_file(&path).map_err(|e| anyhow::anyhow!("remove {path}: {e}"))
    }

    /// 各任务的下次运行时间(本地时区 HH:MM),未调度/未启动的任务不出现。
    pub async fn next_run_times(&self) -> HashMap<AutomationId, String> {
        let mut result = HashMap::new();
        let scheduler = self.scheduler.lock().await.clone();
        let Some(scheduler) = scheduler else {
            return result;
        };
        let jobs = self.jobs.lock().await;
        for (automation_id, job_id) in jobs.iter() {
            let mut s = scheduler.clone();
            if let Ok(Some(dt)) = s.next_tick_for_job(*job_id).await {
                let local = dt.with_timezone(&chrono::Local);
                result.insert(
                    automation_id.clone(),
                    local.format("%m-%d %H:%M").to_string(),
                );
            }
        }
        result
    }
}

/// 一次执行的产出:最终回复(或错误)+ NewThread 会话 id(用于即用即弃清理)。
struct Execution {
    result: std::result::Result<String, String>,
    /// 仅 NewThread:本次新建的会话,结束后应删除其磁盘文件。
    session_id: Option<String>,
}

/// 只读模式排除的会话工具(bash/edit/write + 交易决策提交)。
const READ_ONLY_EXCLUDED_TOOLS: &[&str] = &["bash", "edit", "write", "submit_trading_decision"];

/// 执行一次自动化 prompt,受 `timeout_secs` 约束。
async fn execute(automation: &Automation, pool: &SessionPool, timeout_secs: u64) -> Execution {
    let fut = async {
        match &automation.scope {
            Scope::NewThread => {
                let session_id = if automation.read_only {
                    pool.create_session_with_tools(&automation.cwd, READ_ONLY_EXCLUDED_TOOLS)
                        .await?
                } else {
                    pool.create_session(&automation.cwd).await?
                };
                let handle = pool
                    .get_session(&session_id)
                    .await
                    .ok_or_else(|| "session not found after creation".to_string())?;
                let result = run_prompt(&handle, &automation.prompt).await;
                // 无论成败都关掉会话,避免占用池位。
                pool.close_session(&session_id).await;
                Ok::<_, String>((result, Some(session_id)))
            }
            Scope::ExistingThread { session_id } => {
                let handle = pool.get_or_resume(&session_id.0).await?;
                let result = run_prompt(&handle, &automation.prompt).await;
                Ok((result, None))
            }
        }
    };

    let (result, session_id) =
        match tokio::time::timeout(std::time::Duration::from_secs(timeout_secs), fut).await {
            Ok(Ok(inner)) => inner,
            Ok(Err(e)) => (Err(e), None),
            Err(_) => (Err(format!("run timed out after {timeout_secs}s")), None),
        };
    Execution { result, session_id }
}

async fn run_prompt(handle: &SessionHandle, prompt: &str) -> std::result::Result<String, String> {
    handle.prompt(prompt, PromptOptions::default()).await?;
    handle
        .last_assistant_text()
        .await
        .ok_or_else(|| "no assistant response".to_string())
}
