use anyhow::Result;
use futures::StreamExt;
use serde::Deserialize;
use std::time::Duration;

use crate::api::Client as ApiClient;
use crate::api::xuangubao::MarketOverview;
use crate::llm::openrouter::LlmClient;
use crate::web::RoleEntry;

pub struct TelegramBotConfig {
    pub bot_token: String,
    pub chat_id: String,
}

pub struct TelegramBot {
    bot_token: String,
    chat_id: String,
    llm: LlmClient,
    api: ApiClient,
    roles: Vec<RoleEntry>,
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
    result: Vec<Update>,
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
    pub fn new(config: TelegramBotConfig, roles: Vec<RoleEntry>) -> Self {
        Self {
            bot_token: config.bot_token,
            chat_id: config.chat_id,
            llm: LlmClient::from_env(),
            api: ApiClient::new(),
            roles,
        }
    }

    pub async fn register_commands(&self) -> Result<()> {
        let url = format!("https://api.telegram.org/bot{}/setMyCommands", self.bot_token);
        let client = reqwest::Client::new();
        let resp = client
            .post(&url)
            .json(&serde_json::json!({
                "commands": [
                    {
                        "command": "ask",
                        "description": "向指定角色提问，用法: /ask role_name question"
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
        let client = reqwest::Client::new();

        tracing::info!("[telegram-bot] polling started");

        loop {
            let url = format!(
                "https://api.telegram.org/bot{}/getUpdates?offset={}&timeout=30",
                self.bot_token, offset
            );

            let resp = match client.get(&url).send().await {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!("[telegram-bot] getUpdates error: {e}");
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    continue;
                }
            };

            let body = match resp.json::<GetUpdatesResponse>().await {
                Ok(b) => b,
                Err(e) => {
                    tracing::error!("[telegram-bot] parse error: {e}");
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    continue;
                }
            };

            if !body.ok {
                tracing::error!("[telegram-bot] getUpdates returned ok=false");
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                continue;
            }

            for update in body.result {
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

    async fn handle_message(&self, text: &str) -> Result<()> {
        let trimmed = text.trim();

        if !trimmed.starts_with('/') {
            return Ok(());
        }

        let (cmd, args) = if let Some(space) = trimmed.find(' ') {
            (&trimmed[..space], &trimmed[space + 1..])
        } else {
            (trimmed, "")
        };

        match cmd {
            "/ask" => {
                if args.is_empty() {
                    let role_list: Vec<String> = self
                        .roles
                        .iter()
                        .map(|r| format!("• {} — {}", r.name, r.display_name))
                        .collect();
                    self.send_message(&format!("用法: /ask role_name question\n\n可用角色:\n{}", role_list.join("\n")))
                        .await?;
                    return Ok(());
                }

                let parts: Vec<&str> = args.splitn(2, ' ').collect();
                if parts.len() < 2 {
                    self.send_message("用法: /ask role_name question").await?;
                    return Ok(());
                }

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
                    }
                };

                let msg_id = self
                    .send_message(&format!("🤔 {} 正在思考...", role.display_name))
                    .await?;

                if let Err(e) = self.stream_ask(role, question, msg_id).await {
                    self.edit_message(msg_id, &format!("❌ 处理失败: {e}"))
                        .await?;
                }
            }
            "/start" | "/help" => {
                let role_list: Vec<String> = self
                    .roles
                    .iter()
                    .map(|r| format!("• `/ask {}` — {}", r.name, r.display_name))
                    .collect();
                self.send_message(&format!(
                    "🤖 *OpenATrading Bot*\n\n\
                     *命令列表:*\n\
                     /ask role\\_name question — 向指定角色提问\n\n\
                     *可用角色:*\n{}",
                    role_list.join("\n")
                ))
                .await?;
            }
            _ => {}
        }

        Ok(())
    }

    async fn stream_ask(
        &self,
        role: &RoleEntry,
        question: &str,
        msg_id: i64,
    ) -> Result<()> {
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
                chrono::Local::now().format("%Y-%m-%d %H:%M"),
                o.rise_count,
                o.fall_count,
                o.limit_up_count,
                o.limit_down_count,
                o.bomb_rate,
                o.market_temperature,
            ),
            None => "## 市场数据获取失败\n".to_string(),
        };

        let messages = serde_json::json!([
            {"role": "system", "content": role.strategy},
            {"role": "user", "content": format!("{}\n\n用户提问：{}", market_context, question)}
        ])
        .to_string();

        let mut stream = self.llm.chat_stream_with_messages(&messages).await?;

        let prefix = format!("*{} 回复:*\n\n", role.display_name);
        let mut accumulated = String::new();
        let mut last_edit = std::time::Instant::now();
        let edit_interval = Duration::from_millis(800);
        let mut pending_edit = false;

        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(text) if !text.is_empty() => {
                    accumulated.push_str(&text);
                    pending_edit = true;

                    if last_edit.elapsed() >= edit_interval {
                        let display = format!("{}{}", prefix, accumulated);
                        let _ = self.edit_message(msg_id, &display).await;
                        last_edit = std::time::Instant::now();
                        pending_edit = false;
                    }
                }
                Err(e) => {
                    accumulated.push_str(&format!("\n\n[ERROR] {}", e));
                    break;
                }
                _ => {}
            }
        }

        if pending_edit || !accumulated.is_empty() {
            let display = format!("{}{}", prefix, accumulated);
            self.edit_message(msg_id, &display).await?;
        }

        Ok(())
    }

    async fn fetch_market_overview(&self) -> Option<MarketOverview> {
        self.api.xuangubao.fetch_market_overview().await.ok()
    }

    async fn send_message(&self, text: &str) -> Result<i64> {
        let url = format!("https://api.telegram.org/bot{}/sendMessage", self.bot_token);
        let client = reqwest::Client::new();
        let resp = client
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

        let msg_id = body
            .result
            .map(|r| r.message_id)
            .unwrap_or(-1);
        Ok(msg_id)
    }

    async fn edit_message(&self, message_id: i64, text: &str) -> Result<()> {
        let url = format!("https://api.telegram.org/bot{}/editMessageText", self.bot_token);
        let client = reqwest::Client::new();
        client
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