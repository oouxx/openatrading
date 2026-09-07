//! 暴露给 agent 的自动化任务工具(对话中创建/查看/更新定时任务)。
//!
//! 工具通过 [`FactoryOptions.custom_tools`](crate::llm::FactoryOptions) 注入
//! 每个 agent 会话,执行时直接操作 [`AutomationStore`] 并触发调度器 reload。
//!
//! 参数结构由 pi-rs 的受限采样保证(`ToolDefinition.constrained_sampling`,
//! strict 模式下 provider 在 API 侧强制输出符合 schema 的参数),因此本模块
//! 不做输入纠偏:字段按 schema 类型严格读取,非法传参返回带字段指引的
//! 错误供模型自纠正。不支持 strict 采样的 provider 上,模型同样会收到
//! 这些字段级错误并自行重试。

use std::sync::Arc;

use pi_ai::types::{ConstrainedSamplingConfig, JsonSchemaStrict};
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

/// 把结构化 JSON 包成 pi 的 text ContentBlock。
///
/// pi 在 agent_session.rs 中把 `ToolCallOutput.content` 逐项
/// `serde_json::from_value::<ContentBlock>` 并 `.ok()` 丢弃不匹配项:
/// 裸 JSON 对象(无 `type` 字段)会被静默丢成空 content,模型只会看到
/// 空工具结果。因此必须包成 `{"type":"text","text":...}`;
/// 结构化原文放 `details` 供 UI 使用。
fn output(content: serde_json::Value) -> ToolCallOutput {
    ToolCallOutput {
        content: vec![json!({
            "type": "text",
            "text": serde_json::to_string(&content).unwrap_or_default()
        })],
        details: Some(content),
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

/// 从扁平参数构造调度计划;缺配套字段时给出面向模型的明确指引。
fn build_schedule(kind: &str, args: &Value) -> Result<Schedule, String> {
    let get_i64 = |key: &str| -> Option<i64> { args.get(key).and_then(Value::as_i64) };
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
                    .filter_map(|v| v.as_i64().and_then(|n| u8::try_from(n).ok()))
                    .collect(),
                _ => {
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
    let timeout_secs = args.get("timeout_secs").and_then(Value::as_u64);
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
    let mut patch = AutomationPatch {
        name: args.get("name").and_then(Value::as_str).map(str::to_string),
        prompt: args
            .get("prompt")
            .and_then(Value::as_str)
            .map(str::to_string),
        enabled: args.get("enabled").and_then(Value::as_bool),
        ..AutomationPatch::default()
    };
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
            let draft = build_draft(&args).map_err(tool_error)?;
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
        // 受限采样:provider 支持 strict 工具时,参数结构由 API 侧保证,
        // 彻底消除二次编码/格式错误;不支持的 provider 经 Prefer 优雅降级
        // (归一化兜底继续生效)。
        constrained_sampling: Some(ConstrainedSamplingConfig::JsonSchema {
            strict: JsonSchemaStrict::Prefer,
        }),
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
        constrained_sampling: Some(ConstrainedSamplingConfig::JsonSchema {
            strict: JsonSchemaStrict::Prefer,
        }),
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
            let patch = build_patch(&args).map_err(tool_error)?;
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
        constrained_sampling: Some(ConstrainedSamplingConfig::JsonSchema {
            strict: JsonSchemaStrict::Prefer,
        }),
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
        let out = fut.await.unwrap_or_else(|e| panic!("execute: {e}"));
        assert_tool_output_survives_pi_roundtrip(&out);
        out
    }

    /// 取第一个 text ContentBlock 的正文(模拟模型实际读到的文本)。
    fn first_text_block(out: &ToolCallOutput) -> String {
        out.content
            .first()
            .and_then(|v| v.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or_else(|| panic!("first content block has no text: {:?}", out.content))
            .to_string()
    }

    /// 回归防杜:pi 在 agent_session.rs 里把 content 逐项
    /// `from_value::<ContentBlock>(v).ok()` 丢弃不匹配项。所有工具输出必须
    /// 能无损反序列化为 ContentBlock,否则模型看到的是空结果。
    fn assert_tool_output_survives_pi_roundtrip(out: &ToolCallOutput) {
        use pi_ai::types::ContentBlock;
        for (i, v) in out.content.iter().enumerate() {
            let block: ContentBlock = serde_json::from_value(v.clone()).unwrap_or_else(|e| {
                panic!("content[{i}] not a valid ContentBlock: {e}; value={v}")
            });
            assert!(
                matches!(block, ContentBlock::Text { .. }),
                "content[{i}] should be a text block, got {block:?}"
            );
        }
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

    // ── 扁平参数严格契约(结构由 pi-rs 受限采样保证)──────────────────────

    #[tokio::test]
    async fn flat_args_weekly_single_day_and_string_numbers() {
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

        // 严格契约:字符串数字被拒,错误点名字段
        let result = execute(
            "test".into(),
            json!({
                "name": "严格类型",
                "prompt": "p",
                "schedule_kind": "weekly",
                "days": [1, 2, 3, 4, 5],
                "hour": "9",
                "minute": 0
            }),
            None,
        )
        .await;
        let err = result.err().unwrap_or_else(|| panic!("expected error"));
        assert!(
            err.to_string().contains("hour"),
            "error names the field: {err}"
        );

        // 合法类型 → 成功
        let out = call_tool(
            create,
            json!({
                "name": "合法周报",
                "prompt": "p",
                "schedule_kind": "weekly",
                "days": [1, 2, 3, 4, 5],
                "hour": 9,
                "minute": 0
            }),
        )
        .await;
        assert!(!out.is_error);
        let text = first_text_block(&out);
        let payload: serde_json::Value =
            serde_json::from_str(&text).unwrap_or_else(|e| panic!("text block is valid JSON: {e}"));
        assert_eq!(payload["created"], json!(true));

        let list = hub.store().list().unwrap_or_else(|e| panic!("list: {e}"));
        assert_eq!(list[0].cwd, "."); // cwd 缺省兜底保留(属于 API 语义)
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
