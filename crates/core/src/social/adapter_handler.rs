use crate::context::AppContext;
use crate::llm::session::AgentSession;
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

        let adapter = match adapter_map.get(platform_name) {
            Some(a) => a,
            None => {
                tracing::warn!("[adapter-handler] no adapter for {platform_name}");
                continue;
            },
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
                "decision" => {
                    adapter.reply(&event, "触发决策中，请稍候…").await.ok();
                    // TODO: run decision
                },
                "start" | "help" => {
                    let msg = "可用命令:\n/ask role_name 问题 — 向指定角色提问\n\
                         /new — 开始新对话\n/decision — 触发每日决策".to_string();
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
    let (role_name, question) = match args.split_once(' ') {
        Some((r, q)) => (r, q),
        None => {
            adapter
                .reply(event, "用法: /ask role_name 你的问题")
                .await
                .ok();
            return;
        },
    };

    let role = match roles.iter().find(|r| r.name == role_name) {
        Some(r) => r,
        None => {
            let names: Vec<_> = roles.iter().map(|r| r.name.as_str()).collect();
            adapter
                .reply(
                    event,
                    &format!("未找到角色 '{role_name}'，可用角色: {}", names.join(", ")),
                )
                .await
                .ok();
            return;
        },
    };

    let _ = adapter
        .reply(event, &format!("🤔 {} 正在思考…", role.display_name))
        .await;

    let system = crate::skills::SkillRegistry::build_role_prompt(role);
    match AgentSession::new(&ctx.llm, &system)
        .process_chat(question)
        .await
    {
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
