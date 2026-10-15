use anyhow::Result;
use pi_coding_agent::core::model_registry::ModelRegistry;
use pi_coding_agent::core::settings_manager::SettingsManager;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::context::AppContext;
use crate::social::current_model_display;
use crate::RoleEntry;

const MAX_HISTORY_MESSAGES: usize = 40;

struct ChatSession {
    role_name: String,
    history: Vec<serde_json::Value>,
}

pub struct TelegramBotConfig {
    pub bot_token: String,
    pub chat_id: String,
}

pub struct TelegramBot {
    bot_token: String,
    chat_id: String,
    ctx: AppContext,
    roles: Vec<RoleEntry>,
    session: Arc<Mutex<Option<ChatSession>>>,
    client: reqwest::Client,
}

#[derive(Deserialize)]
struct Update {
    update_id: i64,
    message: Option<TgMessage>,
    callback_query: Option<TgCallbackQuery>,
}

#[derive(Deserialize)]
struct TgCallbackQuery {
    id: String,
    #[serde(default)]
    data: Option<String>,
    #[serde(default)]
    message: Option<TgCallbackMessage>,
}

#[derive(Deserialize)]
struct TgCallbackMessage {
    message_id: i64,
    chat: TgChat,
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
struct SendMessageResponse {
    result: Option<TgMessageObj>,
}

#[derive(Deserialize)]
struct TgMessageObj {
    message_id: i64,
}

// ── /model 交互菜单（Inline Keyboard） ──────────

const MODEL_BUTTONS_PER_ROW: usize = 2;
const MAX_MODELS_SHOWN: usize = 60;

/// 转义 legacy Markdown 的下划线（未配对的 `_` 会导致 Telegram 400）。
fn md_sanitize(s: &str) -> String {
    s.replace('_', "\\_")
}

#[derive(Clone)]
struct InlineButton {
    text: String,
    callback_data: String,
}

fn to_rows(buttons: Vec<InlineButton>, per_row: usize) -> Vec<Vec<InlineButton>> {
    buttons
        .chunks(per_row)
        .map(<[InlineButton]>::to_vec)
        .collect()
}

fn keyboard_json(rows: &[Vec<InlineButton>]) -> serde_json::Value {
    let grid: Vec<Vec<serde_json::Value>> = rows
        .iter()
        .map(|row| {
            row.iter()
                .map(|b| {
                    serde_json::json!({
                        "text": b.text.clone(),
                        "callback_data": b.callback_data.clone(),
                    })
                })
                .collect()
        })
        .collect();
    serde_json::json!({ "inline_keyboard": grid })
}

/// 确定性模型目录：按 provider 分组排序，model 按 id 排序去重。
/// callback_data 用下标回查，渲染与回调使用同一构建逻辑，保证下标一致。
fn model_catalog() -> Vec<(String, Vec<(String, String)>)> {
    let registry = ModelRegistry::new(ModelRegistry::builtin_models_list());
    let mut grouped: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    for m in registry.get_models() {
        let entry = grouped.entry(m.provider.clone()).or_default();
        if entry.iter().all(|(_, id)| id != &m.id) {
            entry.push((m.name.clone(), m.id.clone()));
        }
    }
    for list in grouped.values_mut() {
        list.sort_by(|a, b| a.1.cmp(&b.1));
    }
    grouped.into_iter().collect()
}

fn provider_buttons() -> Vec<InlineButton> {
    model_catalog()
        .iter()
        .enumerate()
        .map(|(i, (provider, _))| InlineButton {
            text: provider.clone(),
            callback_data: format!("mp:{i}"),
        })
        .collect()
}

/// 构建某供应商的模型选择菜单：返回（消息文本，键盘行）。
fn model_menu_rows(
    p_idx: usize,
    provider: &str,
    models: &[(String, String)],
) -> (String, Vec<Vec<InlineButton>>) {
    // 当前模型显示为 `provider/model_id`，用于在当前项上加 ✅
    let current_id = current_model_display()
        .split_once('/')
        .map(|(_, id)| id.to_string());

    let shown = models.len().min(MAX_MODELS_SHOWN);
    let buttons: Vec<InlineButton> = models[..shown]
        .iter()
        .enumerate()
        .map(|(i, (name, id))| {
            let mark = if current_id.as_deref() == Some(id.as_str()) {
                "✅ "
            } else {
                ""
            };
            InlineButton {
                text: format!("{mark}{name}"),
                callback_data: format!("ms:{p_idx}:{i}"),
            }
        })
        .collect();

    let mut rows = to_rows(buttons, MODEL_BUTTONS_PER_ROW);
    rows.push(vec![InlineButton {
        text: "« 返回供应商".to_string(),
        callback_data: "mr".to_string(),
    }]);

    let truncated = if models.len() > MAX_MODELS_SHOWN {
        format!("\n\n_(仅显示前 {MAX_MODELS_SHOWN} 个)_")
    } else {
        String::new()
    };
    let text = format!(
        "供应商: *{}*\n当前模型: *{}*\n\n选择模型:{truncated}",
        md_sanitize(provider),
        md_sanitize(&current_model_display())
    );
    (text, rows)
}

impl TelegramBot {
    pub fn new(config: TelegramBotConfig, roles: Vec<RoleEntry>, ctx: AppContext) -> Self {
        Self {
            bot_token: config.bot_token,
            chat_id: config.chat_id,
            ctx,
            roles,
            session: Arc::new(Mutex::new(None)),
            client: reqwest::Client::new(),
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
                        "command": "model",
                        "description": "查看并切换模型（弹出选择菜单）"
                    },
                    {
                        "command": "new",
                        "description": "开始新对话，清空当前会话历史"
                    }
                ]
            }))
            .send()
            .await?;

        if resp.status().is_success() {
            crate::log::info!("[telegram-bot] registered commands: /model /new");
        } else {
            let body = resp.text().await.unwrap_or_default();
            crate::log::warn!("[telegram-bot] failed to register commands: {body}");
        }
        Ok(())
    }

    pub async fn start_polling(&self) -> Result<()> {
        self.register_commands().await?;

        let mut offset: i64 = 0;

        crate::log::info!("[telegram-bot] polling started");

        loop {
            let url = format!(
                "https://api.telegram.org/bot{}/getUpdates?offset={}&timeout=30",
                self.bot_token, offset
            );

            let resp = match self.client.get(&url).send().await {
                Ok(r) => r,
                Err(e) => {
                    crate::log::error!("[telegram-bot] getUpdates error: {e}");
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    continue;
                }
            };

            let text = match resp.text().await {
                Ok(t) => t,
                Err(e) => {
                    crate::log::error!("[telegram-bot] read response body error: {e}");
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    continue;
                }
            };

            let body: GetUpdatesResponse = match serde_json::from_str(&text) {
                Ok(b) => b,
                Err(e) => {
                    crate::log::error!(
                        "[telegram-bot] parse error: {e}, body: {}",
                        &text[..text.len().min(500)]
                    );
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    continue;
                }
            };

            if !body.ok {
                let desc = body.description.as_deref().unwrap_or("unknown");
                let code = body
                    .error_code
                    .map_or_else(|| "-".into(), |c| c.to_string());
                crate::log::error!("[telegram-bot] getUpdates returned error {code}: {desc}");
                if body.error_code == Some(409) {
                    crate::log::warn!(
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

                    // 按钮点击：必须 answerCallbackQuery，否则客户端转圈
                    if let Some(cb) = update.callback_query {
                        let in_chat = cb
                            .message
                            .as_ref()
                            .map(|m| m.chat.id.to_string() == self.chat_id)
                            .unwrap_or(false);
                        if !in_chat {
                            let _ = self.answer_callback(&cb.id, None).await;
                            continue;
                        }
                        if let Err(e) = self.handle_callback(&cb).await {
                            crate::log::error!("[telegram-bot] handle_callback error: {e}");
                        }
                        continue;
                    }

                    if let Some(msg) = update.message {
                        if let Some(text) = msg.text {
                            if msg.chat.id.to_string() != self.chat_id {
                                continue;
                            }

                            if let Err(e) = self.handle_message(&text).await {
                                crate::log::error!("[telegram-bot] handle_message error: {e}");
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

                let role = if let Some(r) = self.roles.iter().find(|r| r.name == role_name) {
                    r
                } else {
                    let mut session = self.session.lock().await;
                    *session = None;
                    drop(session);
                    self.send_message(&format!(
                        "❌ 当前会话技能 *{role_name}* 已不可用，请使用 /skill:<名称> [问题] 开始新对话"
                    ))
                    .await?;
                    return Ok(());
                };

                let msg_id = self
                    .send_message(&format!("🤔 {} 正在思考...", role.display_name))
                    .await?;

                if let Err(e) = self.stream_ask(role, trimmed, msg_id).await {
                    self.edit_message(msg_id, &format!("❌ 处理失败: {e}"))
                        .await?;
                }
            } else {
                drop(session);
                self.send_message("尚未开始对话，请使用 /skill:<名称> [问题] 选择技能开始")
                    .await?;
            }
            return Ok(());
        }

        // 命令解析：`/skill:<名称> [问题]` 归一化为 `/skill <名称> [问题]`
        let (cmd, args) = if let Some(space) = trimmed.find(' ') {
            (&trimmed[..space], &trimmed[space + 1..])
        } else {
            (trimmed, "")
        };
        let (cmd, args): (&str, String) = if let Some(rest) = cmd.strip_prefix("/skill:") {
            let rest = rest.trim();
            if args.is_empty() {
                ("/skill", rest.to_string())
            } else {
                ("/skill", format!("{rest} {args}"))
            }
        } else {
            (cmd, args.to_string())
        };

        match cmd {
            "/new" => {
                let mut session = self.session.lock().await;
                if let Some(old) = session.take() {
                    let old_role = old.role_name;
                    drop(session);
                    self.send_message(&format!("🗑 已清空与 *{old_role}* 的对话历史，开始新会话"))
                        .await?;
                } else {
                    drop(session);
                    self.send_message("当前没有进行中的对话，使用 /skill:<名称> [问题] 开始")
                        .await?;
                }
            }
            "/model" => {
                let text = format!(
                    "当前模型: *{}*\n\n选��模型供应商:",
                    md_sanitize(&current_model_display())
                );
                let rows = to_rows(provider_buttons(), 2);
                self.send_message_with_keyboard(&text, &rows).await?;
            }
            "/skill" => {
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
                        .map(|r| format!("• /skill:{} — {}", r.name, r.display_name))
                        .collect();
                    self.send_message(&format!(
                        "用法: /skill:<名称> [问题]\n\n可用技能:\n{}{}",
                        role_list.join("\n"),
                        session_info
                    ))
                    .await?;
                    return Ok(());
                }

                let (skill_name, question) = match args.split_once(' ') {
                    Some((name, q)) => (name.trim(), q.trim()),
                    None => (args.trim(), ""),
                };

                let Some(role) = self.roles.iter().find(|r| r.name == skill_name) else {
                    let available: Vec<String> =
                        self.roles.iter().map(|r| r.name.clone()).collect();
                    self.send_message(&format!(
                        "❌ 未知技能: {skill_name}\n可用技能: {}",
                        available.join(", ")
                    ))
                    .await?;
                    return Ok(());
                };

                {
                    let mut session = self.session.lock().await;
                    if session.as_ref().map(|s| s.role_name.as_str()) != Some(skill_name) {
                        *session = Some(ChatSession {
                            role_name: skill_name.to_string(),
                            history: Vec::new(),
                        });
                    }
                }

                if question.is_empty() {
                    self.send_message(&format!(
                        "✅ 已选择技能 *{}*，直接发送消息即可开始对话",
                        role.display_name
                    ))
                    .await?;
                    return Ok(());
                }

                let msg_id = self
                    .send_message(&format!("🤔 {} 正在思考...", role.display_name))
                    .await?;

                if let Err(e) = self.stream_ask(role, question, msg_id).await {
                    self.edit_message(msg_id, &format!("❌ 处理失败: {e}"))
                        .await?;
                }
            }
            _ => {
                self.send_message(
                    "未知命令。可用命令:\n/skill:<名称> [问题] — 选择技能提问\n/new — 清空会话历史\n/model — 查看/切换模型（弹出菜单）",
                )
                .await?;
            }
        }

        Ok(())
    }

    async fn stream_ask(&self, role: &RoleEntry, question: &str, msg_id: i64) -> Result<()> {
        let mut user_msg = String::new();
        {
            let session = self.session.lock().await;
            if let Some(s) = session.as_ref() {
                for msg in &s.history {
                    let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("");
                    let c = msg.get("content").and_then(|v| v.as_str()).unwrap_or("");
                    user_msg.push_str(&format!("\n{role}: {c}"));
                }
            }
        }
        user_msg.push_str(&format!("\n\nuser: {question}"));

        let cwd = std::env::current_dir()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let accumulated = self
            .ctx
            .pool
            .prompt_once(&cwd, &user_msg)
            .await
            .unwrap_or_else(|e| format!("❌ LLM 调用失败: {e}"));

        let prefix = format!("*{} 回复:*\n\n", role.display_name);
        let display = format!("{prefix}{accumulated}");
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

    // ── /model 交互菜单 ───────────────────────────

    async fn send_message_with_keyboard(
        &self,
        text: &str,
        rows: &[Vec<InlineButton>],
    ) -> Result<i64> {
        let url = format!("https://api.telegram.org/bot{}/sendMessage", self.bot_token);
        let resp = self
            .client
            .post(&url)
            .json(&serde_json::json!({
                "chat_id": self.chat_id,
                "text": text,
                "parse_mode": "Markdown",
                "reply_markup": keyboard_json(rows),
            }))
            .send()
            .await?;

        let body: SendMessageResponse = resp
            .json()
            .await
            .unwrap_or(SendMessageResponse { result: None });

        Ok(body.result.map_or(-1, |r| r.message_id))
    }

    async fn edit_message_with_keyboard(
        &self,
        message_id: i64,
        text: &str,
        rows: &[Vec<InlineButton>],
    ) -> Result<()> {
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
                "reply_markup": keyboard_json(rows),
            }))
            .send()
            .await?;
        Ok(())
    }

    /// 应答按钮点击（未应答时客户端持续转圈，text 以 toast 显示）。
    async fn answer_callback(&self, callback_query_id: &str, text: Option<&str>) -> Result<()> {
        let url = format!(
            "https://api.telegram.org/bot{}/answerCallbackQuery",
            self.bot_token
        );
        let mut payload = serde_json::json!({ "callback_query_id": callback_query_id });
        if let Some(t) = text {
            payload["text"] = serde_json::Value::String(t.to_string());
        }
        self.client.post(&url).json(&payload).send().await?;
        Ok(())
    }

    /// 处理按钮点击：`mr` 返回供应商列表 / `mp:<pi>` 选供应商 / `ms:<pi>:<mi>` 设为当前模型。
    async fn handle_callback(&self, cb: &TgCallbackQuery) -> Result<()> {
        let Some(msg) = cb.message.as_ref() else {
            let _ = self.answer_callback(&cb.id, None).await;
            return Ok(());
        };
        let msg_id = msg.message_id;
        let data = cb.data.as_deref().unwrap_or_default();
        let parts: Vec<&str> = data.split(':').collect();

        match parts.as_slice() {
            // 返回供应商列表
            ["mr"] => {
                let text = format!(
                    "当前模型: *{}*\n\n选择模型供应商:",
                    md_sanitize(&current_model_display())
                );
                let rows = to_rows(provider_buttons(), MODEL_BUTTONS_PER_ROW);
                let _ = self.edit_message_with_keyboard(msg_id, &text, &rows).await;
                let _ = self.answer_callback(&cb.id, None).await;
            }
            // 选中供应商 → 列出其模型
            ["mp", idx] => {
                let catalog = model_catalog();
                let entry = idx
                    .parse::<usize>()
                    .ok()
                    .and_then(|i| catalog.get(i).map(|e| (i, e)));
                match entry {
                    Some((i, (provider, models))) => {
                        let (text, rows) = model_menu_rows(i, provider, models);
                        let _ = self.edit_message_with_keyboard(msg_id, &text, &rows).await;
                        let _ = self.answer_callback(&cb.id, None).await;
                    }
                    None => {
                        let _ = self
                            .answer_callback(&cb.id, Some("菜单已更新，请重新发送 /model"))
                            .await;
                    }
                }
            }
            // 选中模型 → 写入默认配置并确认
            ["ms", p, m] => {
                let catalog = model_catalog();
                let selection = p.parse::<usize>().ok().and_then(|pi| {
                    catalog.get(pi).and_then(|(_, models)| {
                        m.parse::<usize>()
                            .ok()
                            .and_then(|mi| models.get(mi).map(|(_, id)| (pi, id.clone())))
                    })
                });

                if let Some((pi, model_id)) = selection {
                    let provider = catalog[pi].0.clone();
                    let cwd = std::env::current_dir()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    SettingsManager::create(&cwd, None)
                        .set_default_model_and_provider(&provider, &model_id);
                    crate::log::info!("[telegram-bot] model switched to {provider}/{model_id}");
                    let _ = self
                        .edit_message(
                            msg_id,
                            &format!(
                                "✅ 当前模型已切换为: *{}*",
                                md_sanitize(&format!("{provider}/{model_id}"))
                            ),
                        )
                        .await;
                    let _ = self.answer_callback(&cb.id, Some("✅ 已切换模型")).await;
                } else {
                    let _ = self
                        .answer_callback(&cb.id, Some("菜单已过期，请重新发送 /model"))
                        .await;
                }
            }
            _ => {
                let _ = self.answer_callback(&cb.id, None).await;
            }
        }

        Ok(())
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

        let body: SendMessageResponse = resp
            .json()
            .await
            .unwrap_or(SendMessageResponse { result: None });

        let msg_id = body.result.map_or(-1, |r| r.message_id);
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_model_menu_markdown_sanitize() {
        assert_eq!(md_sanitize("deepseek_v3"), "deepseek\\_v3");
        assert_eq!(md_sanitize("glm-5.3-flash:cloud"), "glm-5.3-flash:cloud");
        assert_eq!(md_sanitize("未设置"), "未设置");
    }

    #[test]
    fn test_model_catalog_sorted_grouped_deduped() {
        // callback 用下标回查，依赖目录的确定性：provider 严格递增、
        // 每个 provider 内 model id 严格递增且无重复。
        let catalog = model_catalog();
        assert!(!catalog.is_empty(), "catalog should not be empty");

        for i in 0..catalog.len() {
            if i > 0 {
                assert!(
                    catalog[i - 1].0 < catalog[i].0,
                    "providers must be unique & sorted"
                );
            }
            let (_, models) = &catalog[i];
            assert!(!models.is_empty(), "provider {i} has no models");
            for j in 0..models.len() {
                if j > 0 {
                    assert!(
                        models[j - 1].1 < models[j].1,
                        "model ids must be unique & sorted within provider"
                    );
                }
            }
            // 同一渲染构建两次（渲染 + 回调）结果必须完全一致
            let catalog2 = model_catalog();
            assert_eq!(&catalog, &catalog2, "catalog must be deterministic");
        }
    }

    #[test]
    fn test_model_menu_rows_back_button_and_cap() {
        let provider = "test".to_string();
        let models: Vec<(String, String)> = (0..70)
            .map(|i| (format!("model-{i}"), format!("id-{i:03}")))
            .collect();
        let (text, rows) = model_menu_rows(0, &provider, &models);

        // 60 个模型 + 1 行返回按钮
        let model_rows = rows.len() - 1;
        assert_eq!(rows.last().unwrap().len(), 1);
        assert_eq!(rows.last().unwrap()[0].callback_data, "mr");
        let total_model_buttons: usize = rows[..model_rows].iter().map(|r| r.len()).sum();
        assert_eq!(total_model_buttons, MAX_MODELS_SHOWN);
        assert!(text.contains("仅显示前"));
    }

    #[test]
    fn test_model_menu_rows_small_list_no_backmark() {
        let models = vec![("GLM".to_string(), "glm-5.3-flash:cloud".to_string())];
        let (text, rows) = model_menu_rows(0, "ollama", &models);
        assert!(!text.contains("仅显示前"));
        assert_eq!(rows.len(), 2); // 1 行模型 + 1 行返回
        assert_eq!(rows[0][0].callback_data, "ms:0:0");
    }
}
