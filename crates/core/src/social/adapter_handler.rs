use crate::context::AppContext;
use crate::RoleEntry;
use anyhow::Result;
use openatrading_channels::adapter::MessageAdapter;
use openatrading_channels::event::{MessageEvent, Platform};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

/// 消息处理运行时——接收适配器事件，调用 Agent，回复。
pub async fn run_adapter_handler(
    mut rx: mpsc::Receiver<MessageEvent>,
    adapters: Vec<Arc<dyn MessageAdapter>>,
    roles: Vec<RoleEntry>,
    ctx: AppContext,
) -> Result<()> {
    let adapter_map: HashMap<&'static str, Arc<dyn MessageAdapter>> = adapters
        .into_iter()
        .map(|a| (a.platform_name(), a))
        .collect();

    while let Some(event) = rx.recv().await {
        let platform_name = match event.platform {
            Platform::Telegram => "telegram",
            Platform::Discord => "discord",
            Platform::Slack => "slack",
            Platform::Lark => "lark",
            Platform::Whatsapp => "whatsapp",
        };

        let adapter = if let Some(a) = adapter_map.get(platform_name) { a } else {
            crate::log::warn!("[adapter-handler] no adapter for {platform_name}");
            continue;
        };

        let text = event.text.trim().to_string();

        // 处理命令
        if let Some(cmd) = text.strip_prefix('/') {
            let parts: Vec<&str> = cmd.splitn(2, ' ').collect();
            let command = parts[0];
            let args = parts.get(1).copied().unwrap_or("");

            match command {
                "ask" => {
                    handle_ask(adapter, &event, args, &roles, &ctx).await;
                },
                "new" => {
                    adapter.reply(&event, "已清空上下文，开始新对话").await.ok();
                },
                "tools" => {
                    let tools = ctx.all_tools();
                    if tools.is_empty() {
                        adapter.reply(&event, "暂无可用工具").await.ok();
                    } else {
                        let names: Vec<_> = tools.iter().map(|t| format!("- {}", t.name)).collect();
                        let msg = format!("可用工具:\n{}", names.join("\n"));
                        adapter.reply(&event, &msg).await.ok();
                    }
                },
                "skills" => {
                    let names: Vec<_> = roles.iter().map(|r| r.name.as_str()).collect();
                    if names.is_empty() {
                        adapter.reply(&event, "暂无可用角色").await.ok();
                    } else {
                        let msg = format!(
                            "可用角色:\n{}",
                            names
                                .iter()
                                .map(|n| format!("- {n}"))
                                .collect::<Vec<_>>()
                                .join("\n")
                        );
                        adapter.reply(&event, &msg).await.ok();
                    }
                },
                "model" => {
                    // TODO 实现这个slash command
                    let msg = format!("当前模型: {:?}", "");
                    adapter.reply(&event, &msg).await.ok();
                },
                "mcp" => {
                    let tools = ctx.mcp_tools();
                    if tools.is_empty() {
                        adapter.reply(&event, "暂无 MCP 服务").await.ok();
                    } else {
                        let names: Vec<_> = tools.iter().map(|t| format!("- {}", t.name)).collect();
                        let msg = format!("MCP 服务:\n{}", names.join("\n"));
                        adapter.reply(&event, &msg).await.ok();
                    }
                },
                "start" | "help" => {
                    let msg = "可用命令:\n/ask role_name 问题 — 向指定角色提问\n\
                         /new — 开始新对话\n\
                         /tools — 查看可用工具\n/skills — 查看可用角色\n\
                         /model — 查看当前模型\n/mcp — 查看 MCP 服务"
                        .to_string();
                    adapter.reply(&event, &msg).await.ok();
                },
                _ => {
                    adapter
                        .reply(
                            &event,
                            &format!("未知命令: /{command}，发送 /help 查看可用命令"),
                        )
                        .await
                        .ok();
                },
            }
        }
    }

    Ok(())
}

async fn handle_ask(
    adapter: &Arc<dyn MessageAdapter>,
    event: &MessageEvent,
    args: &str,
    roles: &[RoleEntry],
    ctx: &AppContext,
) {
    let (role_name, question) = if let Some((r, q)) = args.split_once(' ') { (r, q) } else {
        adapter
            .reply(event, "用法: /ask role_name 你的问题")
            .await
            .ok();
        return;
    };

    let role = if let Some(r) = roles.iter().find(|r| r.name == role_name) { r } else {
        let names: Vec<_> = roles.iter().map(|r| r.name.as_str()).collect();
        adapter
            .reply(
                event,
                &format!("未找到角色 '{role_name}'，可用角色: {}", names.join(", ")),
            )
            .await
            .ok();
        return;
    };

    let _ = adapter
        .reply(event, &format!("🤔 {} 正在思考…", role.display_name))
        .await;

    let system = crate::skills::SkillRegistry::build_role_prompt(role);
    let agent_facade = match ctx
        .runtime
        .create_session(Some(&system), None, Some(false), None, None)
        .await
    {
        Ok(session) => session,
        Err(e) => {
            let _ = adapter
                .reply(event, &format!("❌ 创建 Agent Session 失败: {e}"))
                .await;
            return;
        },
    };

    match agent_facade.clone().process_chat(question).await {
        Ok(response) => {
            let _ = adapter
                .reply(event, &format!("**{}**: {}", role.display_name, response))
                .await;
        },
        Err(e) => {
            let _ = adapter.reply(event, &format!("❌ 处理失败: {e}")).await;
        },
    }
}
