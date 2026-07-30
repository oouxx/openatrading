use async_trait::async_trait;
use std::time::Duration;

use super::{Check, CheckContext, CheckResult, CheckStatus, FixResult, FixRisk, FixStatus, Result};

/// Network timeout for the Telegram API probe.
///
/// Telegram's API is normally fast; 10s is generous enough to absorb a slow
/// international link while still failing fast on a hung/blocked endpoint.
const NETWORK_TIMEOUT: Duration = Duration::from_secs(10);

pub struct TelegramCheck;

#[async_trait]
impl Check for TelegramCheck {
    fn id(&self) -> &'static str {
        "telegram"
    }
    fn name(&self) -> &'static str {
        "Telegram"
    }
    fn has_fix(&self) -> bool {
        true
    }
    fn fix_risk(&self) -> FixRisk {
        FixRisk::Safe
    }
    fn fix_label(&self) -> Option<String> {
        Some("提示配置".into())
    }

    async fn check(&self, ctx: &CheckContext) -> CheckResult {
        let tg = match &ctx.app_config {
            Some(cfg) => cfg.social.as_ref().and_then(|s| s.telegram.as_ref()),
            None => {
                return CheckResult {
                    status: CheckStatus::Error("无法加载配置".into()),
                    summary: "配置加载失败".into(),
                    details: vec![],
                    duration_ms: 0,
                }
            }
        };

        let tg = match tg {
            Some(t) if t.enabled => t,
            _ => {
                return CheckResult {
                    status: CheckStatus::Warn("未启用".into()),
                    summary: "Telegram 通知未启用".into(),
                    details: vec![],
                    duration_ms: 0,
                }
            }
        };

        let Some(token) = tg.bot_token.as_ref().filter(|t| !t.is_empty()) else {
            return CheckResult {
                status: CheckStatus::Error("未配置".into()),
                summary: "bot_token 未设置".into(),
                details: vec![],
                duration_ms: 0,
            };
        };

        let Some(chat_id) = tg.chat_id.as_ref().filter(|c| !c.is_empty()) else {
            return CheckResult {
                status: CheckStatus::Error("未配置".into()),
                summary: "chat_id 未设置".into(),
                details: vec![],
                duration_ms: 0,
            };
        };

        let client = match reqwest::Client::builder().timeout(NETWORK_TIMEOUT).build() {
            Ok(c) => c,
            Err(e) => {
                return CheckResult {
                    status: CheckStatus::Error("客户端构建失败".into()),
                    summary: format!("构建 HTTP 客户端失败: {e}"),
                    details: vec![],
                    duration_ms: 0,
                }
            }
        };
        let send_url = format!("https://api.telegram.org/bot{token}/sendMessage");
        match client
            .post(&send_url)
            .json(&serde_json::json!({
                "chat_id": chat_id,
                "text": "✓ OpenATrading 连接测试",
            }))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                // Clean up: delete the test message
                if let Ok(body) = resp.json::<serde_json::Value>().await {
                    if let Some(msg_id) = body["result"]["message_id"].as_i64() {
                        let del_url = format!("https://api.telegram.org/bot{token}/deleteMessage");
                        let _ = client
                            .post(&del_url)
                            .json(&serde_json::json!({
                                "chat_id": chat_id,
                                "message_id": msg_id,
                            }))
                            .send()
                            .await;
                    }
                }
                CheckResult {
                    status: CheckStatus::Ok,
                    summary: "连接成功".into(),
                    details: vec![],
                    duration_ms: 0,
                }
            }
            Ok(resp) => {
                let body = resp.text().await.unwrap_or_default();
                CheckResult {
                    status: CheckStatus::Error("API 错误".into()),
                    summary: "Telegram API 返回错误".into(),
                    details: vec![body],
                    duration_ms: 0,
                }
            }
            Err(e) => {
                let is_timeout = e.is_timeout();
                CheckResult {
                    status: CheckStatus::Error("连接失败".into()),
                    summary: if is_timeout {
                        format!("连接超时 (>{NETWORK_TIMEOUT:?})")
                    } else {
                        "无法连接 Telegram API".into()
                    },
                    details: vec![format!("{e}")],
                    duration_ms: 0,
                }
            }
        }
    }

    async fn fix(&self, _ctx: &CheckContext) -> Result<FixResult> {
        Ok(FixResult {
            status: FixStatus::Skipped,
            message: "请在 config.toml [social.telegram] 中配置 bot_token 和 chat_id".into(),
        })
    }
}
