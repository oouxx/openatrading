use anyhow::Result;
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::api::xuangubao::MarketOverview;
use crate::api::Client as ApiClient;
use crate::cron::{run_decision, CronContext};
use crate::llm::session::AgentSession;
use crate::llm::LlmClient;
use crate::memory::MemoryStore;
use crate::trade::TradeManager;
use crate::RoleEntry;

const MAX_HISTORY_MESSAGES: usize = 40;

struct ChatSession {
    role_name: String,
    history: Vec<serde_json::Value>,
}

pub struct TelegramBotConfig {
    pub bot_token: String,
    pub chat_id: String,
    pub mcp_servers: Vec<crate::llm::mcp_tools::McpServerConfig>,
    pub llm_provider: Option<String>,
    pub llm_model: Option<String>,
}

pub struct TelegramBot {
    bot_token: String,
    chat_id: String,
    mcp_servers: Vec<crate::llm::mcp_tools::McpServerConfig>,
    llm: LlmClient,
    api: ApiClient,
    roles: Vec<RoleEntry>,
    session: Arc<Mutex<Option<ChatSession>>>,
    client: reqwest::Client,
    memory: Arc<dyn MemoryStore>,
    trade: Arc<dyn TradeManager>,
}

#[derive(Deserialize)]
struct Update {
    update_id: i64,
    message: Option<TgMessage>,
}

#[derive(Deserialize)]
struct TgMessage {
    chat: TgChat,
    text: Option<String>,
}

#[derive(Deserialize)]
struct TgChat {
    id: i64,
}

#[derive(Deserialize)]
struct GetUpdatesResponse {
    ok: bool,
    error_code: Option<i64>,
    description: Option<String>,
    result: Option<Vec<Update>>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct SendMessageResponse {
    ok: bool,
    result: Option<TgMessageObj>,
}

#[derive(Deserialize)]
struct TgMessageObj {
    message_id: i64,
}

impl TelegramBot {
    pub fn new(
        config: TelegramBotConfig,
        roles: Vec<RoleEntry>,
        memory: Arc<dyn MemoryStore>,
        trade: Arc<dyn TradeManager>,
    ) -> Self {
        Self {
            bot_token: config.bot_token,
            chat_id: config.chat_id,
            mcp_servers: config.mcp_servers,
            llm: LlmClient::from_config(
                config.llm_provider.as_deref(),
                config.llm_model.as_deref(),
            ),
            api: ApiClient::new(),
            roles,
            session: Arc::new(Mutex::new(None)),
            client: reqwest::Client::new(),
            memory,
            trade,
        }
    }

    pub async fn register_commands(&self) -> Result<()> {
        let url = format!(
            "https://api.telegram.org/bot{}/setMyCommands",
            self.bot_token
        );
        let resp = self
            .client
            .post(&url)
            .json(&serde_json::json!({
                "commands": [
                    {
                        "command": "ask",
                        "description": "向指定角色提问（支持多轮对话），用法: /ask role_name question"
                    },
                    {
                        "command": "new",
                        "description": "开始新对话，清空当前会话历史"
                    },
                    {
                        "command": "decision",
                        "description": "手动触发每日决策，可选指定角色: /decision [role_name ...]"
                    }
                ]
            }))
            .send()
            .await?;

        if resp.status().is_success() {
            tracing::info!("[telegram-bot] registered /ask command");
        } else {
            let body = resp.text().await.unwrap_or_default();
            tracing::warn!("[telegram-bot] failed to register commands: {body}");
        }
        Ok(())
    }

    pub async fn start_polling(&self) -> Result<()> {
        self.register_commands().await?;

        let mut offset: i64 = 0;

        tracing::info!("[telegram-bot] polling started");

        loop {
            let url = format!(
                "https://api.telegram.org/bot{}/getUpdates?offset={}&timeout=30",
                self.bot_token, offset
            );

            let resp = match self.client.get(&url).send().await {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!("[telegram-bot] getUpdates error: {e}");
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    continue;
                },
            };

            let text = match resp.text().await {
                Ok(t) => t,
                Err(e) => {
                    tracing::error!("[telegram-bot] read response body error: {e}");
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    continue;
                },
            };

            let body: GetUpdatesResponse = match serde_json::from_str(&text) {
                Ok(b) => b,
                Err(e) => {
                    tracing::error!(
                        "[telegram-bot] parse error: {e}, body: {}",
                        &text[..text.len().min(500)]
                    );
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    continue;
                },
            };

            if !body.ok {
                let desc = body.description.as_deref().unwrap_or("unknown");
                let code = body
                    .error_code
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "-".into());
                tracing::error!("[telegram-bot] getUpdates returned error {code}: {desc}");
                if body.error_code == Some(409) {
                    tracing::warn!(
                        "[telegram-bot] multiple bot instances detected, backing off for 30s"
                    );
                    tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                } else {
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                }
                continue;
            }

            if let Some(updates) = body.result {
                for update in updates {
                    offset = update.update_id + 1;

                    if let Some(msg) = update.message {
                        if let Some(text) = msg.text {
                            if msg.chat.id.to_string() != self.chat_id {
                                continue;
                            }

                            if let Err(e) = self.handle_message(&text).await {
                                tracing::error!("[telegram-bot] handle_message error: {e}");
                            }
                        }
                    }
                }
            }
        }
    }

    async fn handle_message(&self, text: &str) -> Result<()> {
        let trimmed = text.trim();

        if !trimmed.starts_with('/') {
            let session = self.session.lock().await;
            if let Some(s) = session.as_ref() {
                let role_name = s.role_name.clone();
                drop(session);

                let role = match self.roles.iter().find(|r| r.name == role_name) {
                    Some(r) => r,
                    None => {
                        let mut session = self.session.lock().await;
                        *session = None;
                        drop(session);
                        self.send_message(&format!(
                            "❌ 当前会话角色 *{}* 已不可用，请使用 /ask role\\_name question 开始新对话",
                            role_name
                        ))
                        .await?;
                        return Ok(());
                    },
                };

                let msg_id = self
                    .send_message(&format!("🤔 {} 正在思考...", role.display_name))
                    .await?;

                if let Err(e) = self.stream_ask(role, trimmed, msg_id).await {
                    self.edit_message(msg_id, &format!("❌ 处理失败: {e}"))
                        .await?;
                }
            }
            return Ok(());
        }

        let (cmd, args) = if let Some(space) = trimmed.find(' ') {
            (&trimmed[..space], &trimmed[space + 1..])
        } else {
            (trimmed, "")
        };

        match cmd {
            "/new" => {
                let mut session = self.session.lock().await;
                if session.is_some() {
                    let old_role = session.as_ref().unwrap().role_name.clone();
                    *session = None;
                    drop(session);
                    self.send_message(&format!("🗑 已清空与 *{}* 的对话历史，开始新会话", old_role))
                        .await?;
                } else {
                    drop(session);
                    self.send_message("当前没有进行中的对话，使用 /ask role_name question 开始")
                        .await?;
                }
            },
            "/ask" => {
                if args.is_empty() {
                    let session = self.session.lock().await;
                    let session_info = if let Some(s) = session.as_ref() {
                        format!(
                            "\n\n📝 当前会话: *{}* ({} 轮对话)\n直接发送消息即可继续对话",
                            s.role_name,
                            s.history.len() / 2
                        )
                    } else {
                        "\n\n💡 尚无进行中的对话".to_string()
                    };
                    drop(session);

                    let role_list: Vec<String> = self
                        .roles
                        .iter()
                        .map(|r| format!("• {} — {}", r.name, r.display_name))
                        .collect();
                    self.send_message(&format!(
                        "用法: /ask role\\_name question\n\n可用角色:\n{}{}",
                        role_list.join("\n"),
                        session_info
                    ))
                    .await?;
                    return Ok(());
                }

                let parts: Vec<&str> = args.splitn(2, ' ').collect();

                if parts.len() == 2 {
                    let role_name = parts[0];
                    let question = parts[1];

                    let role = match self.roles.iter().find(|r| r.name == role_name) {
                        Some(r) => r,
                        None => {
                            let available: Vec<String> =
                                self.roles.iter().map(|r| r.name.clone()).collect();
                            self.send_message(&format!(
                                "❌ 未知角色: {role_name}\n可用角色: {}",
                                available.join(", ")
                            ))
                            .await?;
                            return Ok(());
                        },
                    };

                    let mut session = self.session.lock().await;
                    if session.is_none() || session.as_ref().unwrap().role_name != role_name {
                        *session = Some(ChatSession {
                            role_name: role_name.to_string(),
                            history: Vec::new(),
                        });
                    }
                    drop(session);

                    let msg_id = self
                        .send_message(&format!("🤔 {} 正在思考...", role.display_name))
                        .await?;

                    if let Err(e) = self.stream_ask(role, question, msg_id).await {
                        self.edit_message(msg_id, &format!("❌ 处理失败: {e}"))
                            .await?;
                    }
                } else {
                    let question = parts[0];
                    let session = self.session.lock().await;

                    if let Some(s) = session.as_ref() {
                        let role_name = s.role_name.clone();
                        drop(session);

                        let role = match self.roles.iter().find(|r| r.name == role_name) {
                            Some(r) => r,
                            None => {
                                self.send_message(&format!(
                                    "❌ 当前会话角色 *{}* 已不可用，请使用 /new 开始新对话",
                                    role_name
                                ))
                                .await?;
                                return Ok(());
                            },
                        };

                        let msg_id = self
                            .send_message(&format!("🤔 {} 正在思考...", role.display_name))
                            .await?;

                        if let Err(e) = self.stream_ask(role, question, msg_id).await {
                            self.edit_message(msg_id, &format!("❌ 处理失败: {e}"))
                                .await?;
                        }
                    } else {
                        drop(session);
                        self.send_message(
                            "❌ 尚未开始对话，请使用 /ask role\\_name question 指定角色开始",
                        )
                        .await?;
                    }
                }
            },
            "/decision" => {
                let filtered_roles: Vec<RoleEntry> = if args.is_empty() {
                    self.roles.clone()
                } else {
                    let names: Vec<&str> = args.split_whitespace().collect();
                    let matched: Vec<RoleEntry> = self
                        .roles
                        .iter()
                        .filter(|r| names.contains(&r.name.as_str()))
                        .cloned()
                        .collect();
                    if matched.is_empty() {
                        let available: Vec<String> =
                            self.roles.iter().map(|r| r.name.clone()).collect();
                        self.send_message(&format!(
                            "❌ 未找到角色: {}\n可用角色: {}",
                            args,
                            available.join(", ")
                        ))
                        .await?;
                        return Ok(());
                    }
                    matched
                };

                let role_names: Vec<&str> =
                    filtered_roles.iter().map(|r| r.name.as_str()).collect();
                let msg_id = self
                    .send_message(&format!(
                        "📋 正在执行决策: {}，请稍候...",
                        role_names.join(", ")
                    ))
                    .await?;

                struct DecisionNotifier {
                    client: reqwest::Client,
                    bot_token: String,
                    chat_id: String,
                }

                #[async_trait::async_trait]
                impl crate::social::NotificationSender for DecisionNotifier {
                    async fn send_text(&self, text: &str) -> anyhow::Result<()> {
                        let url =
                            format!("https://api.telegram.org/bot{}/sendMessage", self.bot_token);
                        let resp = self
                            .client
                            .post(&url)
                            .json(&serde_json::json!({
                                "chat_id": self.chat_id,
                                "text": text,
                            }))
                            .send()
                            .await?;
                        if !resp.status().is_success() {
                            let body = resp.text().await.unwrap_or_default();
                            anyhow::bail!("telegram sendMessage failed: {body}");
                        }
                        Ok(())
                    }
                }

                let notifier = DecisionNotifier {
                    client: self.client.clone(),
                    bot_token: self.bot_token.clone(),
                    chat_id: self.chat_id.clone(),
                };

                let tool_cfg = crate::llm::ToolConfig {
                    mcp_servers: self.mcp_servers.clone(),
                };
                let tools: Vec<Arc<crate::llm::DynTool>> = crate::llm::build_tools(&tool_cfg)
                    .await
                    .all_tools()
                    .to_vec();
                let ctx = CronContext {
                    memory: self.memory.clone(),
                    trade: self.trade.clone(),
                    social: Arc::new(notifier),
                    llm: self.llm.clone(),
                    api: self.api.clone(),
                };
                match run_decision(&ctx, &filtered_roles, &tools).await {
                    Ok(()) => {
                        self.edit_message(msg_id, "✅ 决策已完成，结果已逐条推送")
                            .await?;
                    },
                    Err(e) => {
                        self.edit_message(msg_id, &format!("❌ 决策执行失败: {e}"))
                            .await?;
                    },
                }
            },
            "/start" | "/help" => {
                let role_list: Vec<String> = self
                    .roles
                    .iter()
                    .map(|r| format!("• `/ask {}` — {}", r.name, r.display_name))
                    .collect();
                self.send_message(&format!(
                    "🤖 *OpenATrading Bot*\n\n\
                     *命令列表:*\n\
                     /ask role\\_name question — 向指定角色提问（开始新会话）\n\
                     /new — 清空对话历史，开始新会话\n\
                     /decision — 手动触发每日决策（可选: /decision role\\_name ...）\n\n\
                     *多轮对话:*\n\
                     首次使用 /ask 指定角色后，直接发送消息即可继续对话\n\n\
                     *可用角色:*\n{}",
                    role_list.join("\n")
                ))
                .await?;
            },
            _ => {},
        }

        Ok(())
    }

    async fn stream_ask(&self, role: &RoleEntry, question: &str, msg_id: i64) -> Result<()> {
        let overview = self.fetch_market_overview().await;

        let market_context = match overview {
            Some(o) => format!(
                "## 当前市场数据\n\
                 - 日期：{}\n\
                 - 上涨家数：{}\n\
                 - 下跌家数：{}\n\
                 - 涨停家数：{}\n\
                 - 跌停家数：{}\n\
                 - 炸板率：{:.1}%\n\
                 - 市场温度：{:.1}\n",
                crate::utils::time::beijing_now().format("%Y-%m-%d %H:%M"),
                o.rise_count,
                o.fall_count,
                o.limit_up_count,
                o.limit_down_count,
                o.bomb_rate * 100.0,
                o.market_temperature,
            ),
            None => "## 市场数据获取失败\n".to_string(),
        };

        let mut messages_vec: Vec<serde_json::Value> = vec![
            serde_json::json!({"role": "system", "content": role.strategy}),
            serde_json::json!({"role": "user", "content": market_context}),
            serde_json::json!({"role": "assistant", "content": "已收到市场数据，请提问。"}),
        ];

        {
            let session = self.session.lock().await;
            if let Some(s) = session.as_ref() {
                for msg in &s.history {
                    messages_vec.push(msg.clone());
                }
            }
        }

        messages_vec.push(serde_json::json!({
            "role": "user",
            "content": question,
        }));

        let tool_cfg = crate::llm::ToolConfig {
            mcp_servers: self.mcp_servers.clone(),
        };
        let tools: Vec<Arc<crate::llm::DynTool>> = crate::llm::build_tools(&tool_cfg)
            .await
            .all_tools()
            .to_vec();

        // Combine context + history + question into a single user message
        let mut user_msg = market_context;
        {
            let session = self.session.lock().await;
            if let Some(s) = session.as_ref() {
                for msg in &s.history {
                    let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("");
                    let c = msg.get("content").and_then(|v| v.as_str()).unwrap_or("");
                    user_msg.push_str(&format!("\n{}: {}", role, c));
                }
            }
        }
        user_msg.push_str(&format!("\n\nuser: {}", question));

        let system = crate::skills::SkillRegistry::build_role_prompt(role);
        let accumulated = AgentSession::new(&self.llm, &system)
            .with_tools(&tools)
            .process_chat(&user_msg)
            .await
            .unwrap_or_else(|e| format!("❌ LLM 调用失败: {e}"));

        let prefix = format!("*{} 回复:*\n\n", role.display_name);
        let display = format!("{}{}", prefix, accumulated);
        self.edit_message(msg_id, &display).await?;

        {
            let mut session = self.session.lock().await;
            if let Some(s) = session.as_mut() {
                s.history.push(serde_json::json!({
                    "role": "user",
                    "content": question,
                }));
                s.history.push(serde_json::json!({
                    "role": "assistant",
                    "content": accumulated,
                }));

                if s.history.len() > MAX_HISTORY_MESSAGES {
                    let excess = s.history.len() - MAX_HISTORY_MESSAGES;
                    s.history.drain(0..excess);
                }
            }
        }

        Ok(())
    }

    async fn fetch_market_overview(&self) -> Option<MarketOverview> {
        self.api.xuangubao.fetch_market_overview().await.ok()
    }

    async fn send_message(&self, text: &str) -> Result<i64> {
        let url = format!("https://api.telegram.org/bot{}/sendMessage", self.bot_token);
        let resp = self
            .client
            .post(&url)
            .json(&serde_json::json!({
                "chat_id": self.chat_id,
                "text": text,
                "parse_mode": "Markdown",
            }))
            .send()
            .await?;

        let body: SendMessageResponse = resp.json().await.unwrap_or(SendMessageResponse {
            ok: false,
            result: None,
        });

        let msg_id = body.result.map(|r| r.message_id).unwrap_or(-1);
        Ok(msg_id)
    }

    async fn edit_message(&self, message_id: i64, text: &str) -> Result<()> {
        let url = format!(
            "https://api.telegram.org/bot{}/editMessageText",
            self.bot_token
        );
        self.client
            .post(&url)
            .json(&serde_json::json!({
                "chat_id": self.chat_id,
                "message_id": message_id,
                "text": text,
                "parse_mode": "Markdown",
            }))
            .send()
            .await?;
        Ok(())
    }
}
