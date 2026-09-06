//! 暴露给 agent 的自动化任务工具(对话中创建/查看/更新定时任务)。
//!
//! 工具通过 [`FactoryOptions.custom_tools`](crate::llm::FactoryOptions) 注入
//! 每个 agent 会话,执行时直接操作 [`AutomationStore`] 并触发调度器 reload。
//!
//! 为减少 LLM 传参错误,本模块做了四层防御:
//! 1. **扁平参数**——schedule/scope 不再是嵌套对象,而是顶层标量字段
//!    (`schedule_kind`/`hour`/`minute`/`minutes`/`days`/`cron_expr`/`scope`),
//!    消除 tagged union 二次编码问题(嵌套写法仍兼容,自动转换);
//! 2. **字段级 schema 说明 + 完整调用示例**(可直接照抄);
//! 3. **prompt_guidelines 规则清单**(注入系统提示);
//! 4. **执行前归一化**——数字字符串→数字、单个周序号→数组、缺省兜底,
//!    且解析失败时错误信息带正确格式指引。

use std::sync::Arc;

use pi_extension_api::{ToolCallOutput, ToolDefinition, ToolExecuteFn};
use serde_json::{json, Value};

use super::model::{AutomationPatch, NewAutomation, Schedule, Scope, SessionId};
use super::runner::AutomationHub;

/// 构建自动化任务工具集:`create_automation` + `list_automations` + `update_automation`。
///
/// 每个会话的工厂会克隆这些定义;执行闭包持有 [`AutomationHub`]。
pub fn automation_tools(hub: &Arc<AutomationHub>) -> Vec<ToolDefinition> {
    vec![
        create_automation_tool(hub),
        list_automations_tool(hub),
        update_automation_tool(hub),
    ]
}

fn output(content: serde_json::Value) -> ToolCallOutput {
    ToolCallOutput {
        content: vec![content],
        details: None,
        is_error: false,
        terminate: None,
    }
}

fn tool_error(message: impl std::fmt::Display) -> Box<dyn std::error::Error + Send + Sync> {
    Box::new(std::io::Error::other(message.to_string()))
}

const CREATE_SCHEMA: &str = r#"{
  "type": "object",
  "additionalProperties": false,
  "properties": {
    "name": { "type": "string", "description": "简短任务名称,如「每日提交简报」" },
    "prompt": { "type": "string", "description": "每次运行执行的指令,必须自包含:写清做什么、检查什么、什么算有发现、何时停止或求助" },
    "schedule_kind": {
      "type": "string",
      "enum": ["daily", "weekly", "every_minutes", "cron"],
      "description": "调度类型;daily=每天固定时间, weekly=每周固定时间, every_minutes=每 N 分钟, cron=自定义 6 字段 cron"
    },
    "hour": { "type": "integer", "minimum": 0, "maximum": 23, "description": "daily/weekly 必填:0-23" },
    "minute": { "type": "integer", "minimum": 0, "maximum": 59, "description": "daily/weekly 必填:0-59" },
    "days": { "type": "array", "items": { "type": "integer", "minimum": 1, "maximum": 7 }, "description": "仅 weekly 必填:ISO 周序号数组 1=周一 2=周二 … 7=周日,工作日=[1,2,3,4,5]" },
    "minutes": { "type": "integer", "minimum": 1, "maximum": 1440, "description": "仅 every_minutes 必填:间隔分钟数,建议 ≥15" },
    "cron_expr": { "type": "string", "description": "仅 cron 必填:6 字段「秒 分 时 日 月 周」,周字段 1=周日;工作日 18 点 = \"0 0 18 * * 2-6\"" },
    "scope": { "type": "string", "enum": ["new_thread", "existing_thread"], "description": "new_thread=每次全新运行(默认推荐);existing_thread=心跳唤醒同一会话,需提供 session_id" },
    "session_id": { "type": "string", "description": "仅 scope=existing_thread 必填:要唤醒的会话 ID" },
    "cwd": { "type": "string", "description": "new_thread 的工作目录(项目路径);省略则为当前目录" },
    "read_only": { "type": "boolean", "description": "只读模式:禁用 bash/edit/write 与交易决策;巡检/简报类建议 true" },
    "timeout_secs": { "type": "integer", "minimum": 1, "maximum": 86400, "description": "单次运行超时秒数,省略则默认 600" }
  },
  "required": ["name", "prompt", "schedule_kind", "scope"]
}"#;

const CREATE_EXAMPLES: &str = r#"完整调用示例(字段名、取值必须一字不差;数字不要加引号):

① 每天 9 点简报(最常用):
{"name":"每日提交简报","prompt":"查看本仓库最近 24 小时的提交,按主题分组写一份简短简报;若无新提交,回复「过去 24 小时无新提交」即可。","schedule_kind":"daily","hour":9,"minute":0,"scope":"new_thread","cwd":"/path/to/repo","read_only":true}

② 每 30 分钟盯一次 PR:
{"name":"PR 状态巡检","prompt":"检查 PR #42 的 CI 状态与评论;有新变化才输出详情,否则不输出任何内容。","schedule_kind":"every_minutes","minutes":30,"scope":"new_thread","read_only":true}

③ 每周一 09:00 周报:
{"name":"每周简报","prompt":"汇总最近 7 天的提交与 PR,按工作流分组输出周报。","schedule_kind":"weekly","days":[1],"hour":9,"minute":0,"scope":"new_thread","cwd":"/path/to/repo","read_only":true}

④ 工作日 18 点(用 cron):
{"name":"收盘总结","prompt":"汇总今天涨停/炸板与情绪数据,输出收盘总结。","schedule_kind":"cron","cron_expr":"0 0 18 * * 2-6","scope":"new_thread","read_only":true}"#;

const LIST_SCHEMA: &str = r#"{"type":"object","additionalProperties":false,"properties":{}}"#;

const UPDATE_SCHEMA: &str = r#"{
  "type": "object",
  "additionalProperties": false,
  "properties": {
    "id": { "type": "string", "description": "任务 ID(先 list_automations 查询)" },
    "name": { "type": "string", "description": "可选;只传需要修改的字段" },
    "prompt": { "type": "string", "description": "可选" },
    "schedule_kind": { "type": "string", "enum": ["daily", "weekly", "every_minutes", "cron"], "description": "可选;修改计划时必填,配套字段同 create_automation" },
    "hour": { "type": "integer", "minimum": 0, "maximum": 23 },
    "minute": { "type": "integer", "minimum": 0, "maximum": 59 },
    "days": { "type": "array", "items": { "type": "integer", "minimum": 1, "maximum": 7 } },
    "minutes": { "type": "integer", "minimum": 1, "maximum": 1440 },
    "cron_expr": { "type": "string" },
    "enabled": { "type": "boolean", "description": "false=暂停,true=恢复" }
  },
  "required": ["id"]
}"#;

const UPDATE_EXAMPLES: &str = r#"示例——把任务改为每天 10 点并暂停:
{"id":"<list_automations 查到的 id>","schedule_kind":"daily","hour":10,"minute":0,"enabled":false}"#;

// ── 参数解析(扁平 schema → 领域模型)─────────────────────────────────

/// schedule_kind 的合法取值(错误信息用)。
const SCHEDULE_KINDS: &str = "daily|weekly|every_minutes|cron";

/// 兼容层:把三种历史写法归一成扁平参数再解析——
/// 1. schedule/scope 二次编码 JSON 字符串 → 解包对象
/// 2. 嵌套 schedule/scope 对象 → 平铺为顶层字段
/// 3. 数字字符串 → 数字、单个周序号 → 数组
/// 扁平 schema 的字段(若同时出现)优先于兼容写法。
fn normalize_compat(args: Value) -> Value {
    let mut v = args;
    let Some(obj) = v.as_object_mut() else {
        return v;
    };

    // 1. 二次编码字符串解包(schedule/scope 是高频翻车点)
    for key in ["schedule", "scope"] {
        if let Some(Value::String(s)) = obj.get(key) {
            if let Ok(parsed) = serde_json::from_str::<Value>(s) {
                if parsed.is_object() {
                    obj.insert(key.to_string(), parsed);
                }
            }
        }
    }

    // 2. 嵌套 schedule 对象 → 平铺
    if let Some(schedule) = obj.get("schedule").cloned() {
        if let Some(m) = schedule.as_object() {
            if obj.get("schedule_kind").is_none() {
                if let Some(kind) = m.get("kind").and_then(Value::as_str) {
                    obj.insert("schedule_kind".into(), json!(kind));
                }
            }
            for key in ["hour", "minute", "minutes", "days"] {
                if let Some(x) = m.get(key) {
                    obj.entry(key.to_string()).or_insert_with(|| x.clone());
                }
            }
            if let Some(Value::String(expr)) = m.get("expr") {
                obj.entry("cron_expr".to_string()).or_insert(json!(expr));
            }
        }
    }

    // 3. 嵌套 scope 对象 → 平铺
    if let Some(scope) = obj.get("scope").cloned() {
        if let Some(m) = scope.as_object() {
            if let Some(t) = m.get("type").and_then(Value::as_str) {
                if !matches!(obj.get("scope"), Some(Value::String(_))) {
                    obj.insert("scope".into(), json!(t));
                }
            }
            if let Some(sid) = m.get("session_id") {
                obj.entry("session_id".to_string())
                    .or_insert_with(|| sid.clone());
            }
        }
    }

    // 4. 数字字符串纠偏 + days 单值/字符串数组转数组
    for key in ["hour", "minute", "minutes", "timeout_secs"] {
        if let Some(v) = obj.get_mut(key) {
            if let Some(s) = v.as_str() {
                if let Ok(n) = s.trim().parse::<i64>() {
                    *v = json!(n);
                }
            }
        }
    }
    if let Some(days) = obj.get("days").cloned() {
        match days {
            Value::Number(_) => {
                let n = days.as_i64().unwrap_or(0);
                obj.insert("days".into(), json!([n]));
            }
            Value::String(s) => {
                if let Ok(Value::Array(items)) = serde_json::from_str::<Value>(&s) {
                    obj.insert("days".into(), Value::Array(items));
                } else if let Ok(n) = s.trim().parse::<i64>() {
                    obj.insert("days".into(), json!([n]));
                }
            }
            _ => {}
        }
    }

    v
}

/// 从扁平参数构造调度计划;缺配套字段时给出面向模型的明确指引。
fn build_schedule(kind: &str, args: &Value) -> Result<Schedule, String> {
    let get_i64 = |key: &str| -> Option<i64> {
        args.get(key).and_then(|v| {
            v.as_i64()
                .or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))
        })
    };
    let hour = || -> Result<u8, String> {
        get_i64("hour")
            .map(|h| u8::try_from(h).unwrap_or(255))
            .ok_or_else(|| format!("kind={kind} 需要 hour(0-23);示例 \"hour\": 9"))
    };
    let minute = || -> Result<u8, String> {
        get_i64("minute")
            .map(|m| u8::try_from(m).unwrap_or(255))
            .ok_or_else(|| format!("kind={kind} 需要 minute(0-59);示例 \"minute\": 0"))
    };

    match kind {
        "daily" => Ok(Schedule::Daily {
            hour: hour()?,
            minute: minute()?,
        }),
        "weekly" => {
            let days: Vec<u8> = match args.get("days") {
                Some(Value::Array(items)) => items
                    .iter()
                    .filter_map(|v| {
                        v.as_i64()
                            .or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))
                            .and_then(|n| u8::try_from(n).ok())
                    })
                    .collect(),
                Some(v) => v
                    .as_i64()
                    .map(|n| vec![u8::try_from(n).unwrap_or(0)])
                    .ok_or_else(|| {
                        "kind=weekly 需要 days 数组(1=周一..7=周日);示例 \"days\": [1,2,3,4,5]"
                            .to_string()
                    })?,
                None => {
                    return Err(
                        "kind=weekly 需要 days 数组(1=周一..7=周日);示例 \"days\": [1,2,3,4,5]"
                            .into(),
                    )
                }
            };
            Ok(Schedule::Weekly {
                days,
                hour: hour()?,
                minute: minute()?,
            })
        }
        "every_minutes" => {
            let minutes = get_i64("minutes").ok_or_else(|| {
                "kind=every_minutes 需要 minutes(1-1440);示例 \"minutes\": 30".to_string()
            })?;
            Ok(Schedule::EveryMinutes {
                minutes: u32::try_from(minutes).unwrap_or(0),
            })
        }
        "cron" => {
            let expr = args
                .get("cron_expr")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    "kind=cron 需要 cron_expr(6 字段「秒 分 时 日 月 周」);示例 \"cron_expr\": \"0 0 18 * * 2-6\""
                        .to_string()
                })?;
            Ok(Schedule::Cron {
                expr: expr.to_string(),
            })
        }
        _ => Err(format!(
            "未知的 schedule_kind「{kind}」;合法取值:{SCHEDULE_KINDS}"
        )),
    }
}

/// 从扁平参数构造 NewAutomation;缺省兜底 + 归一化都在这里完成。
fn build_draft(args: &Value) -> Result<NewAutomation, String> {
    let name = args
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| "缺少 name".to_string())?;
    let prompt = args
        .get("prompt")
        .and_then(Value::as_str)
        .ok_or_else(|| "缺少 prompt".to_string())?;
    let kind = args
        .get("schedule_kind")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("缺少 schedule_kind(合法取值:{SCHEDULE_KINDS})"))?;
    let schedule = build_schedule(kind, args)?;
    let scope = match args
        .get("scope")
        .and_then(Value::as_str)
        .unwrap_or("new_thread")
    {
        "existing_thread" => Scope::ExistingThread {
            session_id: SessionId(
                args.get("session_id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
            ),
        },
        _ => Scope::NewThread,
    };
    let timeout_secs = args.get("timeout_secs").and_then(|v| {
        v.as_u64()
            .or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))
    });
    Ok(NewAutomation {
        name: name.to_string(),
        prompt: prompt.to_string(),
        schedule,
        scope,
        cwd: args
            .get("cwd")
            .and_then(Value::as_str)
            .unwrap_or(".")
            .to_string(),
        read_only: args
            .get("read_only")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        timeout_secs,
    })
}

/// 从扁平参数构造更新补丁;没有任何 schedule 字段时 schedule 保持不变。
fn build_patch(args: &Value) -> Result<AutomationPatch, String> {
    let mut patch = AutomationPatch::default();
    patch.name = args.get("name").and_then(Value::as_str).map(str::to_string);
    patch.prompt = args
        .get("prompt")
        .and_then(Value::as_str)
        .map(str::to_string);
    patch.enabled = args.get("enabled").and_then(Value::as_bool);
    if let Some(kind) = args.get("schedule_kind").and_then(Value::as_str) {
        patch.schedule = Some(build_schedule(kind, args)?);
    }
    Ok(patch)
}

fn create_automation_tool(hub: &Arc<AutomationHub>) -> ToolDefinition {
    let hub = Arc::clone(hub);
    let execute: ToolExecuteFn = Arc::new(move |_call_id, args, _cancel| {
        let hub = Arc::clone(&hub);
        Box::pin(async move {
            let draft = build_draft(&normalize_compat(args)).map_err(tool_error)?;
            let created = hub.store().create(&draft).map_err(tool_error)?;
            // 已启动的调度器立即热更新;未启动(桌面端)时为 no-op,由 reconcile 兜底。
            if let Err(e) = hub.reload().await {
                crate::log::warn!("[automation] tool reload failed: {e}");
            }
            Ok(output(json!({
                "created": true,
                "id": created.id.0,
                "name": created.name,
                "schedule_cron": format_cron_hint(&created.schedule),
                "hint": "已生效。可在「自动化」面板查看/编辑;下次运行时间见 next_runs。",
            })))
        })
    });

    ToolDefinition {
        name: "create_automation".into(),
        label: Some("创建自动化".into()),
        description: format!(
            "创建定时自动化任务:按计划在后台重复运行一段提示词。\
             有发现的运行进入收件箱并通知用户;无产出自动归档,不打扰。\n\n\
             参数全是顶层字段:schedule_kind 决定哪个字段必填——\n\
             - daily → hour + minute\n\
             - weekly → days + hour + minute(days 是 ISO 周序号 1=周一…7=周日)\n\
             - every_minutes → minutes\n\
             - cron → cron_expr(6 字段「秒 分 时 日 月 周」,周字段 1=周日)\n\
             scope 为 \"new_thread\"(默认,推荐)或 \"existing_thread\"(需 session_id)。\n\n\
             {CREATE_EXAMPLES}"
        ),
        prompt_snippet: Some(
            "用户说「每天/每周/每 N 分钟…帮我做某事」时:先在当前对话手动执行一次任务验证效果,\
             再调用 create_automation 挂成定时任务,并复述返回的计划给用户。"
                .into(),
        ),
        parameters: Some(serde_json::from_str(CREATE_SCHEMA).unwrap_or_default()),
        prompt_guidelines: Some(vec![
            "schedule_kind 精确取值: daily|weekly|every_minutes|cron;scope 取值: new_thread|existing_thread,不要发明其他值。".into(),
            "所有数字字段(hour/minute/minutes/days)直接写数字,不要加引号。".into(),
            "weekly 的 days 是 ISO 周序号:1=周一、2=周二、…、7=周日,「工作日」=[1,2,3,4,5]。".into(),
            "cron_expr 是 6 字段(秒 分 时 日 月 周),周字段 1=周日。".into(),
            "创建前先调用 list_automations 确认没有同名的既有任务,避免重复。".into(),
            "巡检/简报/盯盘类只读任务一律 read_only=true;需要真正修改文件的任务才用 false 并向用户确认。".into(),
            "用户未指定时间时,先询问具体时间点,不要擅自猜测;timeout_secs 一般省略(默认 600 秒)。".into(),
            "调用成功后,向用户复述:任务名、调度计划(cron 形式)、下次运行时间。".into(),
        ]),
        render_shell: None,
        execution_mode: None,
        execute: Some(execute),
        source_info: None,
    }
}

fn list_automations_tool(hub: &Arc<AutomationHub>) -> ToolDefinition {
    let hub = Arc::clone(hub);
    let execute: ToolExecuteFn = Arc::new(move |_call_id, _args, _cancel| {
        let hub = Arc::clone(&hub);
        Box::pin(async move {
            let list = hub.store().list().map_err(tool_error)?;
            let summary: Vec<serde_json::Value> = list
                .iter()
                .map(|a| {
                    json!({
                        "id": a.id.0,
                        "name": a.name,
                        "enabled": a.enabled,
                        "read_only": a.read_only,
                        "schedule_cron": format_cron_hint(&a.schedule),
                    })
                })
                .collect();
            Ok(output(json!({ "automations": summary })))
        })
    });

    ToolDefinition {
        name: "list_automations".into(),
        label: Some("查看自动化".into()),
        description: "列出当前全部定时自动化任务(id/名称/启用状态/只读/cron 计划)。\
             无需参数,传 {} 即可;在创建同名任务前先调用它去重。"
            .into(),
        prompt_snippet: None,
        parameters: Some(serde_json::from_str(LIST_SCHEMA).unwrap_or_default()),
        prompt_guidelines: Some(vec!["调用时传空对象 {} 或省略参数。".into()]),
        render_shell: None,
        execution_mode: None,
        execute: Some(execute),
        source_info: None,
    }
}

fn update_automation_tool(hub: &Arc<AutomationHub>) -> ToolDefinition {
    let hub = Arc::clone(hub);
    let execute: ToolExecuteFn = Arc::new(move |_call_id, args, _cancel| {
        let hub = Arc::clone(&hub);
        Box::pin(async move {
            let id = args
                .get("id")
                .and_then(Value::as_str)
                .map(std::string::ToString::to_string)
                .ok_or_else(|| tool_error("缺少 id;先用 list_automations 查询任务 ID"))?;
            let patch = build_patch(&normalize_compat(args)).map_err(tool_error)?;
            let updated = hub
                .store()
                .update(&crate::automation::AutomationId(id), &patch)
                .map_err(tool_error)?;
            if let Err(e) = hub.reload().await {
                crate::log::warn!("[automation] tool reload failed: {e}");
            }
            Ok(output(json!({
                "updated": true,
                "id": updated.id.0,
                "name": updated.name,
                "enabled": updated.enabled,
                "schedule_cron": format_cron_hint(&updated.schedule),
            })))
        })
    });

    ToolDefinition {
        name: "update_automation".into(),
        label: Some("更新自动化".into()),
        description: format!(
            "更新已有定时自动化任务:只传 id 与需要修改的字段(name/prompt/schedule_kind+配套字段/enabled),\
             未提及的字段保持不变。\n\n\
             {UPDATE_EXAMPLES}"
        ),
        prompt_snippet: None,
        parameters: Some(serde_json::from_str(UPDATE_SCHEMA).unwrap_or_default()),
        prompt_guidelines: Some(vec![
            "id 必填,来自 list_automations;不要凭记忆猜测。".into(),
            "只传要修改的字段;enabled=false 表示暂停(不删除)。".into(),
        ]),
        render_shell: None,
        execution_mode: None,
        execute: Some(execute),
        source_info: None,
    }
}

fn format_cron_hint(schedule: &Schedule) -> String {
    match schedule.to_cron() {
        Ok(expr) => expr,
        Err(e) => format!("invalid schedule: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automation::model::Scope;
    

    fn temp_hub() -> (tempfile::TempDir, Arc<AutomationHub>) {
        let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
        let store = Arc::new(
            crate::automation::AutomationStore::open(&dir.path().join("data.db"))
                .unwrap_or_else(|e| panic!("open: {e}")),
        );
        (dir, AutomationHub::new(store))
    }

    /// 模拟 agent 工具调用:走 ToolExecuteFn 的完整路径。
    async fn call_tool(tool: &ToolDefinition, args: serde_json::Value) -> ToolCallOutput {
        let execute = tool.execute.clone().unwrap_or_else(|| panic!("no execute"));
        let fut = execute("test-call".into(), args, None);
        fut.await.unwrap_or_else(|e| panic!("execute: {e}"))
    }

    #[tokio::test]
    async fn create_automation_tool_creates_and_persists() {
        let (_dir, hub) = temp_hub();
        let tools = automation_tools(&hub);
        let create = tools
            .iter()
            .find(|t| t.name == "create_automation")
            .unwrap_or_else(|| panic!("missing create_automation"));

        let out = call_tool(
            create,
            json!({
                "name": "工具创建的任务",
                "prompt": "巡检依赖",
                "schedule_kind": "daily",
                "hour": 8,
                "minute": 30,
                "scope": "new_thread",
                "cwd": "/tmp"
            }),
        )
        .await;

        assert!(!out.is_error);
        let store = hub.store();
        let list = store.list().unwrap_or_else(|e| panic!("list: {e}"));
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "工具创建的任务");
        assert_eq!(
            list[0].schedule,
            Schedule::Daily {
                hour: 8,
                minute: 30
            }
        );
    }

    #[tokio::test]
    async fn create_automation_tool_reports_validation_errors() {
        let (_dir, hub) = temp_hub();
        let tools = automation_tools(&hub);
        let create = tools
            .iter()
            .find(|t| t.name == "create_automation")
            .unwrap_or_else(|| panic!("missing"));
        let execute = create
            .execute
            .clone()
            .unwrap_or_else(|| panic!("no execute"));

        // 参数合法但业务校验失败 → Err(框架转成工具错误输出)
        let result = execute(
            "test".into(),
            json!({
                "name": "",
                "prompt": "p",
                "schedule_kind": "daily",
                "hour": 9,
                "minute": 0,
                "scope": "new_thread"
            }),
            None,
        )
        .await;
        assert!(result.is_err(), "empty name must surface as tool error");
    }

    #[tokio::test]
    async fn list_automations_tool_lists() {
        let (_dir, hub) = temp_hub();
        hub.store()
            .create(&NewAutomation {
                name: "已有任务".into(),
                prompt: "p".into(),
                schedule: Schedule::Daily { hour: 9, minute: 0 },
                scope: Scope::NewThread,
                cwd: "/tmp".into(),
                read_only: false,
                timeout_secs: None,
            })
            .unwrap_or_else(|e| panic!("create: {e}"));

        let tools = automation_tools(&hub);
        let list_tool = tools
            .iter()
            .find(|t| t.name == "list_automations")
            .unwrap_or_else(|| panic!("missing list_automations"));
        let out = call_tool(list_tool, json!({})).await;
        let text = out
            .content
            .first()
            .cloned()
            .unwrap_or_else(|| panic!("empty content"))
            .to_string();
        assert!(text.contains("已有任务"), "list output contains task name");
    }

    // ── 扁平参数与错误引导 ────────────────────────────────────────────────

    #[tokio::test]
    async fn flat_args_weekly_single_day_and_string_numbers() {
        let (_dir, hub) = temp_hub();
        let tools = automation_tools(&hub);
        let create = tools
            .iter()
            .find(|t| t.name == "create_automation")
            .unwrap_or_else(|| panic!("missing create_automation"));

        // 全部故意写错类型:字符串数字、单个 days、缺 cwd/scope
        let out = call_tool(
            create,
            json!({
                "name": "归一化任务",
                "prompt": "p",
                "schedule_kind": "weekly",
                "days": "1",
                "hour": "9",
                "minute": "0",
                "timeout_secs": "120"
            }),
        )
        .await;
        assert!(!out.is_error, "string numbers and single day must coerce");

        let list = hub.store().list().unwrap_or_else(|e| panic!("list: {e}"));
        assert_eq!(
            list[0].schedule,
            Schedule::Weekly {
                days: vec![1],
                hour: 9,
                minute: 0
            }
        );
        assert_eq!(list[0].timeout_secs, Some(120));
        assert_eq!(list[0].cwd, ".");
    }

    #[tokio::test]
    async fn missing_required_kind_field_gives_field_hint() {
        let (_dir, hub) = temp_hub();
        let tools = automation_tools(&hub);
        let create = tools
            .iter()
            .find(|t| t.name == "create_automation")
            .unwrap_or_else(|| panic!("missing"));
        let execute = create
            .execute
            .clone()
            .unwrap_or_else(|| panic!("no execute"));

        // kind=daily 但漏了 minute → 错误信息必须指出缺哪个字段 + 示例
        let result = execute(
            "test".into(),
            json!({
                "name": "n",
                "prompt": "p",
                "schedule_kind": "daily",
                "hour": 9,
                "scope": "new_thread"
            }),
            None,
        )
        .await;
        let err = result.err().unwrap_or_else(|| panic!("expected error"));
        assert!(
            err.to_string().contains("minute"),
            "error must name the missing field, got: {err}"
        );

        // 未知 kind → 列出合法取值
        let result = execute(
            "test".into(),
            json!({
                "name": "n",
                "prompt": "p",
                "schedule_kind": "every_hour",
                "scope": "new_thread"
            }),
            None,
        )
        .await;
        let err = result.err().unwrap_or_else(|| panic!("expected error"));
        assert!(
            err.to_string().contains("daily|weekly|every_minutes|cron"),
            "error must list valid kinds, got: {err}"
        );
    }

    // ── 兼容层:嵌套/二次编码写法仍被接受 ────────────────────────────────

    #[tokio::test]
    async fn nested_schedule_object_still_accepted() {
        let (_dir, hub) = temp_hub();
        let tools = automation_tools(&hub);
        let create = tools
            .iter()
            .find(|t| t.name == "create_automation")
            .unwrap_or_else(|| panic!("missing create_automation"));

        let out = call_tool(
            create,
            json!({
                "name": "嵌套写法",
                "prompt": "p",
                "schedule": {"kind": "daily", "hour": 7, "minute": 15},
                "scope": {"type": "new_thread"}
            }),
        )
        .await;
        assert!(
            !out.is_error,
            "nested tagged-object form must stay supported"
        );

        let list = hub.store().list().unwrap_or_else(|e| panic!("list: {e}"));
        assert_eq!(
            list[0].schedule,
            Schedule::Daily {
                hour: 7,
                minute: 15
            }
        );
    }

    #[tokio::test]
    async fn double_encoded_json_string_still_accepted() {
        let (_dir, hub) = temp_hub();
        let tools = automation_tools(&hub);
        let create = tools
            .iter()
            .find(|t| t.name == "create_automation")
            .unwrap_or_else(|| panic!("missing create_automation"));

        // 真实案例:schedule 被模型整体传成 JSON 字符串(内容完全正确)
        let out = call_tool(
            create,
            json!({
                "name": "工作日简报",
                "prompt": "p",
                "schedule": "{\"days\": [1, 2, 3, 4, 5], \"hour\": 8, \"kind\": \"weekly\", \"minute\": 30}",
                "scope": "new_thread"
            }),
        )
        .await;
        assert!(!out.is_error, "double-encoded schedule must be unwrapped");

        let list = hub.store().list().unwrap_or_else(|e| panic!("list: {e}"));
        assert_eq!(
            list[0].schedule,
            Schedule::Weekly {
                days: vec![1, 2, 3, 4, 5],
                hour: 8,
                minute: 30
            }
        );
    }

    #[tokio::test]
    async fn tool_definitions_carry_guidelines_and_examples() {
        let (_dir, hub) = temp_hub();
        let tools = automation_tools(&hub);

        let create = tools
            .iter()
            .find(|t| t.name == "create_automation")
            .unwrap_or_else(|| panic!("missing"));
        // 描述含完整示例(模型可直接照抄的 JSON)
        assert!(create.description.contains(r#""schedule_kind":"daily""#));
        assert!(create.description.contains(r#""read_only":true"#));
        // guidelines 注入系统提示
        let guidelines = create.prompt_guidelines.clone().unwrap_or_default();
        assert!(
            guidelines.iter().any(|g| g.contains("数字")),
            "guidelines should warn about number types"
        );
        assert!(create.prompt_snippet.is_some());

        let update = tools
            .iter()
            .find(|t| t.name == "update_automation")
            .unwrap_or_else(|| panic!("missing"));
        assert!(update.description.contains(r#""schedule_kind":"daily""#));
    }
}
