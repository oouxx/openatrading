use crate::context::AppContext;
use crate::social::current_model_display;
use crate::RoleEntry;
use ahash::AHashMap as HashMap;
use anyhow::Result;
use openatrading_channels::event::{MessageEvent, Platform};
use openatrading_channels::Channel;
use std::sync::Arc;
use tokio::sync::mpsc;

/// 消息处理运行时——接收适配器事件，调用 Agent，回复。
///
/// 仅处理以下命令，其余消息忽略：
/// - `/model` — 查看当前默认模型
/// - `/skill:<名称> [问题]` — 选择技能并向其提问
/// - `/new` — 清空上下文，开始新对话
pub async fn run_adapter_handler(
    mut rx: mpsc::Receiver<MessageEvent>,
    adapters: Vec<Arc<dyn Channel>>,
    roles: Vec<RoleEntry>,
    ctx: AppContext,
) -> Result<()> {
    let adapter_map: HashMap<&'static str, Arc<dyn Channel>> =
        adapters.into_iter().map(|a| (a.name(), a)).collect();

    while let Some(event) = rx.recv().await {
        let platform_name = match event.platform {
            Platform::Telegram => "telegram",
            Platform::Discord => "discord",
            Platform::Slack => "slack",
            Platform::Lark => "lark",
        };

        let Some(adapter) = adapter_map.get(platform_name) else {
            crate::log::warn!("[adapter-handler] no adapter for {platform_name}");
            continue;
        };

        let text = event.text.trim().to_string();

        // 处理命令
        if let Some(cmd) = text.strip_prefix('/') {
            // 归一化：`/skill:<名称>` → `skill <名称>`
            let body: String = if let Some(rest) = cmd.strip_prefix("skill:") {
                let rest = rest.trim();
                if rest.is_empty() {
                    "skill".to_string()
                } else {
                    format!("skill {rest}")
                }
            } else {
                cmd.to_string()
            };

            let (command, args) = body.split_once(' ').unwrap_or((&body, ""));

            match command {
                "skill" => {
                    handle_skill(adapter, &event, args.trim(), &roles, &ctx).await;
                }
                "new" => {
                    adapter.reply(&event, "已清空上下文，开始新对话").await.ok();
                }
                "model" => {
                    let msg = format!("当前模型: {}", current_model_display());
                    adapter.reply(&event, &msg).await.ok();
                }
                _ => {
                    adapter
                        .reply(
                            &event,
                            "未知命令，可用命令:\n/skill:<名称> [问题] — 选择技能提问\n/new — 开始新对话\n/model — 查看当前模型",
                        )
                        .await
                        .ok();
                }
            }
        }
    }

    Ok(())
}

async fn handle_skill(
    adapter: &Arc<dyn Channel>,
    event: &MessageEvent,
    args: &str,
    roles: &[RoleEntry],
    ctx: &AppContext,
) {
    // args 为空时列出可用技能
    if args.is_empty() {
        let msg = if roles.is_empty() {
            "暂无可用技能".to_string()
        } else {
            let names: Vec<_> = roles
                .iter()
                .map(|r| format!("/skill:{} — {}", r.name, r.display_name))
                .collect();
            format!(
                "用法: /skill:<名称> [问题]\n\n可用技能:\n{}",
                names.join("\n")
            )
        };
        adapter.reply(event, &msg).await.ok();
        return;
    }

    let (skill_name, question) = match args.split_once(' ') {
        Some((name, q)) => (name.trim(), Some(q.trim())),
        None => (args.trim(), None),
    };

    let Some(role) = roles.iter().find(|r| r.name == skill_name) else {
        let names: Vec<_> = roles.iter().map(|r| r.name.as_str()).collect();
        adapter
            .reply(
                event,
                &format!("未找到技能 '{skill_name}'，可用技能: {}", names.join(", ")),
            )
            .await
            .ok();
        return;
    };

    let Some(question) = question.filter(|q| !q.is_empty()) else {
        adapter
            .reply(
                event,
                &format!("已选择技能「{}」，请发送问题", role.display_name),
            )
            .await
            .ok();
        return;
    };

    let _ = adapter
        .reply(event, &format!("🤔 {} 正在思考…", role.display_name))
        .await;

    let cwd = std::env::current_dir()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    match ctx.pool.prompt_once(&cwd, question).await {
        Ok(response) => {
            let _ = adapter
                .reply(event, &format!("**{}**: {}", role.display_name, response))
                .await;
        }
        Err(e) => {
            let _ = adapter.reply(event, &format!("❌ 处理失败: {e}")).await;
        }
    }
}
