//! 暴露给 agent 的自动化任务工具(对话中创建/查看定时任务)。
//!
//! 工具通过 [`FactoryOptions.custom_tools`](crate::llm::FactoryOptions) 注入
//! 每个 agent 会话,执行时直接操作 [`AutomationStore`] 并触发调度器 reload。

use std::sync::Arc;

use pi_extension_api::{ToolCallOutput, ToolDefinition, ToolExecuteFn};
use serde_json::json;

use super::model::{NewAutomation, Schedule};
use super::runner::AutomationHub;

const CREATE_SCHEMA: &str = r#"{
  "type": "object",
  "properties": {
    "name": { "type": "string", "description": "任务名称,例如「每日项目简报」" },
    "prompt": { "type": "string", "description": "每次运行注入给 agent 的提示词,应包含如何判断是否有值得报告的内容" },
    "schedule": {
      "oneOf": [
        { "type": "object", "properties": { "kind": { "const": "daily" }, "hour": { "type": "integer", "minimum": 0, "maximum": 23 }, "minute": { "type": "integer", "minimum": 0, "maximum": 59 } }, "required": ["kind", "hour", "minute"] },
        { "type": "object", "properties": { "kind": { "const": "every_minutes" }, "minutes": { "type": "integer", "minimum": 1, "maximum": 1440 } }, "required": ["kind", "minutes"] },
        { "type": "object", "properties": { "kind": { "const": "cron" }, "expr": { "type": "string", "description": "6 字段 cron:秒 分 时 日 月 周(1=周日)" } }, "required": ["kind", "expr"] }
      ]
    },
    "scope": {
      "oneOf": [
        { "type": "object", "properties": { "type": { "const": "new_thread" } }, "required": ["type"] },
        { "type": "object", "properties": { "type": { "const": "existing_thread" }, "session_id": { "type": "string" } }, "required": ["type", "session_id"] }
      ]
    },
    "cwd": { "type": "string", "description": "独立会话运行的工作目录" }
  },
  "required": ["name", "prompt", "schedule", "scope", "cwd"]
}"#;

const LIST_SCHEMA: &str = r#"{"type":"object","properties":{}}"#;

const UPDATE_SCHEMA: &str = r#"{
  "type": "object",
  "properties": {
    "id": { "type": "string", "description": "要更新的任务 ID(list_automations 可查)" },
    "name": { "type": "string" },
    "prompt": { "type": "string" },
    "schedule": {
      "oneOf": [
        { "type": "object", "properties": { "kind": { "const": "daily" }, "hour": { "type": "integer", "minimum": 0, "maximum": 23 }, "minute": { "type": "integer", "minimum": 0, "maximum": 59 } }, "required": ["kind", "hour", "minute"] },
        { "type": "object", "properties": { "kind": { "const": "every_minutes" }, "minutes": { "type": "integer", "minimum": 1, "maximum": 1440 } }, "required": ["kind", "minutes"] },
        { "type": "object", "properties": { "kind": { "const": "weekly" }, "days": { "type": "array", "items": { "type": "integer", "minimum": 1, "maximum": 7 }, "minItems": 1, "description": "ISO 周序号:1=周一..7=周日" }, "hour": { "type": "integer" }, "minute": { "type": "integer" } }, "required": ["kind", "days", "hour", "minute"] },
        { "type": "object", "properties": { "kind": { "const": "cron" }, "expr": { "type": "string" } }, "required": ["kind", "expr"] }
      ]
    },
    "enabled": { "type": "boolean" }
  },
  "required": ["id"]
}"#;

/// 构建自动化任务工具集:`create_automation` + `list_automations`。
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

fn create_automation_tool(hub: &Arc<AutomationHub>) -> ToolDefinition {
    let hub = Arc::clone(hub);
    let execute: ToolExecuteFn = Arc::new(move |_call_id, args, _cancel| {
        let hub = Arc::clone(&hub);
        Box::pin(async move {
            let draft: NewAutomation = serde_json::from_value(args).map_err(
                |e| -> Box<dyn std::error::Error + Send + Sync> {
                    Box::new(std::io::Error::other(format!("invalid arguments: {e}")))
                },
            )?;
            let created = hub.store().create(&draft).map_err(
                |e| -> Box<dyn std::error::Error + Send + Sync> {
                    Box::new(std::io::Error::other(e.to_string()))
                },
            )?;
            // 已启动的调度器立即热更新;未启动(桌面端)时为 no-op,由 reconcile 兜底。
            if let Err(e) = hub.reload().await {
                crate::log::warn!("[automation] tool reload failed: {e}");
            }
            Ok(output(json!({
                "created": true,
                "id": created.id.0,
                "name": created.name,
                "schedule": format_cron_hint(&created.schedule),
            })))
        })
    });

    ToolDefinition {
        name: "create_automation".into(),
        label: Some("创建自动化".into()),
        description: "创建定时自动化任务:按计划(cron/间隔/每日)在后台重复运行一段提示词,\
             有发现的结果会进入收件箱并通知,无产出则自动归档。\
             每次运行应彼此独立时用 new_thread 范围;需要延续当前对话上下文时用 existing_thread。"
            .into(),
        prompt_snippet: Some(
            "用户说「每天/每小时/每 N 分钟…帮我做某事」时,先测试性执行一次任务描述,\
             再调用 create_automation 挂成定时任务。"
                .into(),
        ),
        parameters: Some(serde_json::from_str(CREATE_SCHEMA).unwrap_or_default()),
        prompt_guidelines: None,
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
            let body = args;
            let id = body
                .get("id")
                .and_then(serde_json::Value::as_str)
                .map(std::string::ToString::to_string)
                .ok_or_else(|| -> Box<dyn std::error::Error + Send + Sync> {
                    Box::new(std::io::Error::other("missing id"))
                })?;
            let patch = serde_json::from_value(body.clone()).map_err(
                |e| -> Box<dyn std::error::Error + Send + Sync> {
                    Box::new(std::io::Error::other(format!("invalid arguments: {e}")))
                },
            )?;
            let updated = hub
                .store()
                .update(&crate::automation::AutomationId(id), &patch)
                .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                    Box::new(std::io::Error::other(e.to_string()))
                })?;
            if let Err(e) = hub.reload().await {
                crate::log::warn!("[automation] tool reload failed: {e}");
            }
            Ok(output(json!({
                "updated": true,
                "id": updated.id.0,
                "name": updated.name,
                "enabled": updated.enabled,
            })))
        })
    });

    ToolDefinition {
        name: "update_automation".into(),
        label: Some("更新自动化".into()),
        description: "更新已有定时自动化任务的名称/提示词/调度计划/启停。只传需要修改的字段。"
            .into(),
        prompt_snippet: None,
        parameters: Some(serde_json::from_str(UPDATE_SCHEMA).unwrap_or_default()),
        prompt_guidelines: None,
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
            let list =
                hub.store()
                    .list()
                    .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                        Box::new(std::io::Error::other(e.to_string()))
                    })?;
            let summary: Vec<serde_json::Value> = list
                .iter()
                .map(|a| {
                    json!({
                        "id": a.id.0,
                        "name": a.name,
                        "enabled": a.enabled,
                        "schedule": format_cron_hint(&a.schedule),
                    })
                })
                .collect();
            Ok(output(json!({ "automations": summary })))
        })
    });

    ToolDefinition {
        name: "list_automations".into(),
        label: Some("查看自动化".into()),
        description: "列出当前全部定时自动化任务及其调度计划。".into(),
        prompt_snippet: None,
        parameters: Some(serde_json::from_str(LIST_SCHEMA).unwrap_or_default()),
        prompt_guidelines: None,
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
    use super::super::model::Scope;
    use super::*;

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
}
