//! 暴露给 agent 的自动化任务工具(对话中创建/查看/更新定时任务)。
//!
//! 工具通过 [`FactoryOptions.custom_tools`](crate::llm::FactoryOptions) 注入
//! 每个 agent 会话,执行时直接操作 [`AutomationStore`] 并触发调度器 reload。
//!
//! 为减少 LLM 传参错误,三个工具都提供:字段级 schema 说明、完整调用示例、
//! `prompt_guidelines` 规则清单(注入系统提示),以及执行前的参数归一化
//! (数字字符串→数字、单个周序号→数组、缺省字段兜底)。

use std::sync::Arc;

use pi_extension_api::{ToolCallOutput, ToolDefinition, ToolExecuteFn};
use serde_json::{json, Value};

use super::model::{NewAutomation, Schedule};
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
    "schedule": {
      "description": "调度计划对象;kind 决定其余必填字段(hour/minute/minutes/days 均为数字)",
      "oneOf": [
        { "type": "object", "additionalProperties": false, "properties": { "kind": { "const": "daily" }, "hour": { "type": "integer", "minimum": 0, "maximum": 23, "description": "0-23" }, "minute": { "type": "integer", "minimum": 0, "maximum": 59, "description": "0-59" } }, "required": ["kind", "hour", "minute"] },
        { "type": "object", "additionalProperties": false, "properties": { "kind": { "const": "weekly" }, "days": { "type": "array", "items": { "type": "integer", "minimum": 1, "maximum": 7 }, "minItems": 1, "description": "ISO 周序号数组:1=周一 2=周二 … 7=周日,如 [1,3,5]" }, "hour": { "type": "integer", "minimum": 0, "maximum": 23 }, "minute": { "type": "integer", "minimum": 0, "maximum": 59 } }, "required": ["kind", "days", "hour", "minute"] },
        { "type": "object", "additionalProperties": false, "properties": { "kind": { "const": "every_minutes" }, "minutes": { "type": "integer", "minimum": 1, "maximum": 1440, "description": "1-1440,建议 ≥15" } }, "required": ["kind", "minutes"] },
        { "type": "object", "additionalProperties": false, "properties": { "kind": { "const": "cron" }, "expr": { "type": "string", "description": "6 字段 cron:秒 分 时 日 月 周(周 1=周日),如「工作日 18 点」= \"0 0 18 * * 2-6\"" } }, "required": ["kind", "expr"] }
      ]
    },
    "scope": {
      "description": "运行范围;不确定 session_id 时用 new_thread",
      "oneOf": [
        { "type": "object", "additionalProperties": false, "properties": { "type": { "const": "new_thread" } }, "required": ["type"] },
        { "type": "object", "additionalProperties": false, "properties": { "type": { "const": "existing_thread" }, "session_id": { "type": "string", "description": "要心跳唤醒的既有会话 ID" } }, "required": ["type", "session_id"] }
      ]
    },
    "cwd": { "type": "string", "description": "NewThread 的工作目录(项目路径);existing_thread 可省略" },
    "read_only": { "type": "boolean", "description": "只读模式:禁用 bash/edit/write 与交易决策;巡检/简报类建议 true" },
    "timeout_secs": { "type": "integer", "minimum": 1, "maximum": 86400, "description": "单次运行超时秒数,省略则默认 600" }
  },
  "required": ["name", "prompt", "schedule", "scope"]
}"#;

const CREATE_EXAMPLES: &str = r#"完整调用示例(字段名、kind 名必须一字不差):

① 每天 9 点简报(最常用):
{"name":"每日提交简报","prompt":"查看本仓库最近 24 小时的提交,按主题分组写一份简短简报;若无新提交,回复「过去 24 小时无新提交」即可。","schedule":{"kind":"daily","hour":9,"minute":0},"scope":{"type":"new_thread"},"cwd":"/path/to/repo","read_only":true}

② 每 30 分钟盯一次 PR:
{"name":"PR 状态巡检","prompt":"检查 PR #42 的 CI 状态与评论;有新变化才输出详情,否则不输出任何内容。","schedule":{"kind":"every_minutes","minutes":30},"scope":{"type":"new_thread"},"read_only":true}

③ 每周一 09:00 周报:
{"name":"每周简报","prompt":"汇总最近 7 天的提交与 PR,按工作流分组输出周报。","schedule":{"kind":"weekly","days":[1],"hour":9,"minute":0},"scope":{"type":"new_thread"},"cwd":"/path/to/repo","read_only":true}"#;

const LIST_SCHEMA: &str = r#"{"type":"object","additionalProperties":false,"properties":{}}"#;

const UPDATE_SCHEMA: &str = r#"{
  "type": "object",
  "additionalProperties": false,
  "properties": {
    "id": { "type": "string", "description": "任务 ID(先 list_automations 查询)" },
    "name": { "type": "string", "description": "可选;只传需要修改的字段" },
    "prompt": { "type": "string", "description": "可选" },
    "schedule": {
      "description": "可选;结构与 create_automation 相同",
      "oneOf": [
        { "type": "object", "additionalProperties": false, "properties": { "kind": { "const": "daily" }, "hour": { "type": "integer", "minimum": 0, "maximum": 23 }, "minute": { "type": "integer", "minimum": 0, "maximum": 59 } }, "required": ["kind", "hour", "minute"] },
        { "type": "object", "additionalProperties": false, "properties": { "kind": { "const": "weekly" }, "days": { "type": "array", "items": { "type": "integer", "minimum": 1, "maximum": 7 }, "minItems": 1 }, "hour": { "type": "integer", "minimum": 0, "maximum": 23 }, "minute": { "type": "integer", "minimum": 0, "maximum": 59 } }, "required": ["kind", "days", "hour", "minute"] },
        { "type": "object", "additionalProperties": false, "properties": { "kind": { "const": "every_minutes" }, "minutes": { "type": "integer", "minimum": 1, "maximum": 1440 } }, "required": ["kind", "minutes"] },
        { "type": "object", "additionalProperties": false, "properties": { "kind": { "const": "cron" }, "expr": { "type": "string" } }, "required": ["kind", "expr"] }
      ]
    },
    "enabled": { "type": "boolean", "description": "暂停/恢复任务" }
  },
  "required": ["id"]
}"#;

const UPDATE_EXAMPLES: &str = r#"示例——把任务改为每天 10 点并暂停:
{"id":"<list_automations 查到的 id>","schedule":{"kind":"daily","hour":10,"minute":0},"enabled":false}"#;

/// 执行前归一化常见传参错误(降低重试):
/// - schedule/scope 传成 JSON 字符串(二次编码)→ 解析回对象
/// - 数字字段传成字符串("9" → 9)、浮点(9.0 → 9)
/// - weekly.days 传单个数字 → 数组
/// - scope 缺省 → new_thread;scope 有 session_id 缺 type → existing_thread
/// - cwd 缺省 → "."
fn normalize_create_args(args: Value) -> Value {
    let mut v = args;
    if let Some(obj) = v.as_object_mut() {
        // 二次编码的 JSON 字符串 → 解析回对象(schedule/scope 是高频翻车点)。
        for key in ["schedule", "scope"] {
            if let Some(Value::String(s)) = obj.get(key) {
                if let Ok(parsed) = serde_json::from_str::<Value>(s) {
                    if parsed.is_object() {
                        obj.insert(key.to_string(), parsed);
                    }
                }
            }
        }
    }
    normalize_object_args(v)
}

/// 对象级归一化:缺省兜底 + 字段类型纠偏。
fn normalize_object_args(args: Value) -> Value {
    let mut v = args;
    let Some(obj) = v.as_object_mut() else {
        return v;
    };

    // scope 缺省/补 type
    let scope_missing_type = match obj.get("scope") {
        Some(Value::Object(m)) => !m.contains_key("type"),
        Some(Value::Null) | None => true,
        _ => false,
    };
    if scope_missing_type {
        let new_scope = match obj.get("scope") {
            Some(Value::Object(m)) => {
                let mut m = m.clone();
                m.insert("type".into(), json!("existing_thread"));
                Value::Object(m)
            }
            _ => json!({"type": "new_thread"}),
        };
        obj.insert("scope".into(), new_scope);
    }

    // cwd 缺省
    if !obj.contains_key("cwd") {
        obj.insert("cwd".into(), json!("."));
    }

    // schedule 归一化
    if let Some(schedule) = obj.get_mut("schedule") {
        coerce_number(schedule, &["hour", "minute", "minutes"]);
        if let Some(days) = schedule.get_mut("days") {
            match days {
                Value::Number(_) => {
                    let n = days.as_i64().unwrap_or(0);
                    *days = json!([n]);
                }
                Value::Array(items) => {
                    for item in items.iter_mut() {
                        if let Some(n) = item.as_str().and_then(|s| s.trim().parse::<i64>().ok()) {
                            *item = json!(n);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    // timeout_secs 字符串→数字
    if let Some(t) = obj.get_mut("timeout_secs") {
        if let Some(s) = t.as_str() {
            if let Ok(n) = s.trim().parse::<i64>() {
                *t = json!(n);
            }
        }
    }

    v
}

/// 对象内的指定键做字符串→整数转换(忽略无法解析的值,交给校验层报错)。
fn coerce_number(value: &mut Value, keys: &[&str]) {
    let Some(obj) = value.as_object_mut() else {
        return;
    };
    for key in keys {
        if let Some(v) = obj.get_mut(*key) {
            if let Some(s) = v.as_str() {
                if let Ok(n) = s.trim().parse::<i64>() {
                    *v = json!(n);
                }
            }
        }
    }
}

fn create_automation_tool(hub: &Arc<AutomationHub>) -> ToolDefinition {
    let hub = Arc::clone(hub);
    let execute: ToolExecuteFn = Arc::new(move |_call_id, args, _cancel| {
        let hub = Arc::clone(&hub);
        Box::pin(async move {
            let normalized = normalize_create_args(args);
            let draft: NewAutomation = serde_json::from_value(normalized).map_err(|e| {
                tool_error(format!(
                    "参数解析失败:{e}\n正确格式见工具 description 中的完整示例;\
                         schedule/scope 必须是对象且 kind/type 名字精确。"
                ))
            })?;
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
             schedule 必须是对象,kind 取 daily|weekly|every_minutes|cron,各自必填字段:\n\
             - {{kind:\"daily\", hour:0-23, minute:0-59}}\n\
             - {{kind:\"weekly\", days:[1-7], hour, minute}}  days 为 ISO 周序号 1=周一…7=周日\n\
             - {{kind:\"every_minutes\", minutes:1-1440}}\n\
             - {{kind:\"cron\", expr:\"秒 分 时 日 月 周\"}}  周字段 1=周日\n\n\
             scope:{{type:\"new_thread\"}} 每次全新运行;{{type:\"existing_thread\", session_id}} 心跳唤醒同一会话。\n\n\
             {CREATE_EXAMPLES}"
        ),
        prompt_snippet: Some(
            "用户说「每天/每周/每 N 分钟…帮我做某事」时:先在当前对话手动执行一次任务验证效果,\
             再调用 create_automation 挂成定时任务,并复述返回的计划给用户。"
                .into(),
        ),
        parameters: Some(serde_json::from_str(CREATE_SCHEMA).unwrap_or_default()),
        prompt_guidelines: Some(vec![
            "schedule 与 scope 必须传完整对象;kind/type 的取值精确为 daily|weekly|every_minutes|cron 与 new_thread|existing_thread,不要发明其他值。".into(),
            "hour、minute、minutes、days 必须是数字或数字数组,不要传字符串。".into(),
            "weekly 的 days 是 ISO 周序号:1=周一、2=周二、…、7=周日,「工作日」=[1,2,3,4,5]。".into(),
            "cron 的 expr 是 6 字段(秒 分 时 日 月 周),周字段 1=周日;5 字段会自动补秒,但优先写全 6 字段。".into(),
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
            let body = normalize_create_args(args);
            let id = body
                .get("id")
                .and_then(Value::as_str)
                .map(std::string::ToString::to_string)
                .ok_or_else(|| tool_error("缺少 id;先用 list_automations 查询任务 ID"))?;
            let patch = serde_json::from_value(body.clone()).map_err(|e| {
                tool_error(format!(
                    "参数解析失败:{e}\nupdate 只传需要修改的字段,示例:\
                     {{\"id\":\"<任务id>\",\"schedule\":{{\"kind\":\"daily\",\"hour\":10,\"minute\":0}},\"enabled\":false}}"
                ))
            })?;
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
            "更新已有定时自动化任务:只传 id 与需要修改的字段(name/prompt/schedule/enabled),\
             未提及的字段保持不变。schedule 结构与 create_automation 相同。\n\n\
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
                "schedule": {"kind": "daily", "hour": 8, "minute": 30},
                "scope": {"type": "new_thread"},
                "cwd": "/tmp"
            }),
        )
        .await;

        assert!(!out.is_error);
        let store = hub.store();
        let list = store.list().unwrap_or_else(|e| panic!("list: {e}"));
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "工具创建的任务");
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
            json!({ "name": "", "prompt": "p", "schedule": {"kind":"daily","hour":9,"minute":0}, "scope": {"type":"new_thread"}, "cwd": "." }),
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

    // ── 本轮:参数归一化 ────────────────────────────────────────────────

    #[tokio::test]
    async fn normalize_coerces_string_numbers_and_single_day() {
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
                "schedule": {"kind": "daily", "hour": "9", "minute": "0"},
                "scope": {"type": "new_thread"},
                "timeout_secs": "120"
            }),
        )
        .await;
        assert!(!out.is_error, "string numbers must be coerced");

        let store = hub.store();
        let list = store.list().unwrap_or_else(|e| panic!("list: {e}"));
        assert_eq!(list[0].timeout_secs, Some(120));
        assert_eq!(list[0].cwd, ".");
    }

    #[tokio::test]
    async fn normalize_single_day_becomes_array() {
        let (_dir, hub) = temp_hub();
        let tools = automation_tools(&hub);
        let create = tools
            .iter()
            .find(|t| t.name == "create_automation")
            .unwrap_or_else(|| panic!("missing create_automation"));

        let out = call_tool(
            create,
            json!({
                "name": "周报",
                "prompt": "p",
                "schedule": {"kind": "weekly", "days": 1, "hour": 9, "minute": 0},
                "scope": {"type": "new_thread"}
            }),
        )
        .await;
        assert!(!out.is_error, "single day number must coerce to array");

        let list = hub.store().list().unwrap_or_else(|e| panic!("list: {e}"));
        assert_eq!(
            list[0].schedule,
            Schedule::Weekly {
                days: vec![1],
                hour: 9,
                minute: 0
            }
        );
    }

    #[tokio::test]
    async fn parse_failure_error_contains_hint() {
        let (_dir, hub) = temp_hub();
        let tools = automation_tools(&hub);
        let create = tools
            .iter()
            .find(|t| t.name == "create_automation")
            .unwrap_or_else(|| panic!("missing create_automation"));
        let execute = create
            .execute
            .clone()
            .unwrap_or_else(|| panic!("no execute"));

        // schedule 传成字符串 → 解析失败,错误信息必须包含示例指引
        let result = execute(
            "test".into(),
            json!({
                "name": "n",
                "prompt": "p",
                "schedule": "每天九点",
                "scope": {"type": "new_thread"}
            }),
            None,
        )
        .await;
        let err = result.err().unwrap_or_else(|| panic!("expected error"));
        assert!(
            err.to_string().contains("schedule/scope 必须是对象"),
            "error must guide the model to retry with correct shape, got: {err}"
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
        assert!(create.description.contains(r#""kind":"daily""#));
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
        assert!(update.description.contains(r#""kind":"daily""#));
    }
    // ── 线上报错回归:二次编码的 JSON 字符串 ──────────────────────────────

    #[tokio::test]
    async fn normalize_unwraps_double_encoded_json_string() {
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
            "scope": {"type": "new_thread"}
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
    async fn normalize_unwraps_scope_as_json_string() {
        let (_dir, hub) = temp_hub();
        let tools = automation_tools(&hub);
        let create = tools
            .iter()
            .find(|t| t.name == "create_automation")
            .unwrap_or_else(|| panic!("missing create_automation"));

        let out = call_tool(
            create,
            json!({
                "name": "n",
                "prompt": "p",
                "schedule": {"kind": "daily", "hour": 9, "minute": 0},
                "scope": "{\"type\": \"new_thread\"}"
            }),
        )
        .await;
        assert!(!out.is_error, "double-encoded scope must be unwrapped");

        let list = hub.store().list().unwrap_or_else(|e| panic!("list: {e}"));
        assert_eq!(list[0].scope, Scope::NewThread);
    }
}
