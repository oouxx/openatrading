use std::path::Path;
use std::str::FromStr;

use anyhow::Result;
use serde::Serialize;

use crate::cli::config::AppConfig;
use openatrading_web::gateway::Gateway;

#[derive(Serialize)]
struct DoctorResult {
    config: CheckItem,
    sqlite: CheckItem,
    cron: CheckItem,
    telegram: CheckItem,
    roles: CheckItem,
}

#[derive(Serialize)]
struct CheckItem {
    status: String, // "ok" | "error" | "warn"
    message: String,
}

fn check_icon(status: &str) -> &str {
    match status {
        "ok" => "✅",
        "error" => "❌",
        "warn" => "⚠",
        _ => "?",
    }
}

async fn check_telegram(app_config: &AppConfig) -> CheckItem {
    match &app_config.social {
        Some(sc) if sc.telegram.as_ref().map(|t| t.enabled).unwrap_or(false) => {
            let tg = sc.telegram.as_ref().unwrap();
            let token = tg.bot_token.as_ref().filter(|t| !t.is_empty());
            let chat = tg.chat_id.as_ref().filter(|c| !c.is_empty());

            let (token, chat) = match (token, chat) {
                (Some(t), Some(c)) => (t, c),
                _ => {
                    return CheckItem {
                        status: "error".into(),
                        message: "bot_token 或 chat_id 未配置".into(),
                    }
                }
            };

            let client = reqwest::Client::new();
            let send_url = format!("https://api.telegram.org/bot{token}/sendMessage");
            match client
                .post(&send_url)
                .json(&serde_json::json!({
                    "chat_id": chat,
                    "text": "✅ OpenATrading Telegram 连接测试",
                }))
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => {
                    // Delete the test message
                    if let Ok(body) = resp.json::<serde_json::Value>().await {
                        if let Some(msg_id) = body["result"]["message_id"].as_i64() {
                            let del_url =
                                format!("https://api.telegram.org/bot{token}/deleteMessage");
                            let _ = client
                                .post(&del_url)
                                .json(&serde_json::json!({
                                    "chat_id": chat,
                                    "message_id": msg_id,
                                }))
                                .send()
                                .await;
                        }
                    }
                    CheckItem {
                        status: "ok".into(),
                        message: "Telegram 连接成功".into(),
                    }
                }
                Ok(resp) => {
                    let body = resp.text().await.unwrap_or_default();
                    CheckItem {
                        status: "error".into(),
                        message: format!("API 返回错误: {body}"),
                    }
                }
                Err(e) => CheckItem {
                    status: "error".into(),
                    message: format!("连接失败: {e}"),
                },
            }
        }
        _ => CheckItem {
            status: "warn".into(),
            message: "Telegram 未启用".into(),
        },
    }
}

pub async fn cmd_doctor(project_root: &Path, json: bool) -> Result<()> {
    // ── Config ────────────────────────────────────
    let (app_config, config_item) = match AppConfig::load(project_root) {
        Ok(cfg) => (
            Some(cfg),
            CheckItem {
                status: "ok".into(),
                message: "config.toml".into(),
            },
        ),
        Err(e) => (
            None,
            CheckItem {
                status: "error".into(),
                message: format!("{e:#}"),
            },
        ),
    };

    // ── SQLite ─────────────────────────────────────
    let db_path = project_root.join(".atrading").join(
        app_config
            .as_ref()
            .map(|c| c.service.db_path.as_str())
            .unwrap_or("data.db"),
    );
    let sqlite_item = match Gateway::init(&db_path) {
        Ok(_) => CheckItem {
            status: "ok".into(),
            message: "SQLite".into(),
        },
        Err(e) => CheckItem {
            status: "error".into(),
            message: format!("{e:#}"),
        },
    };

    // ── Cron ───────────────────────────────────────
    let cron_item = match &app_config {
        Some(cfg) => match &cfg.cron {
            Some(c) if c.enabled => {
                // Validate cron schedule by trying to parse
                if cron::Schedule::from_str(&c.collection_schedule).is_ok()
                    && cron::Schedule::from_str(&c.decision_schedule).is_ok()
                {
                    CheckItem {
                        status: "ok".into(),
                        message: format!(
                            "采集: {}  决策: {}",
                            c.collection_schedule, c.decision_schedule
                        ),
                    }
                } else {
                    CheckItem {
                        status: "error".into(),
                        message: "cron 表达式无效".into(),
                    }
                }
            }
            _ => CheckItem {
                status: "warn".into(),
                message: "cron 未启用".into(),
            },
        },
        None => CheckItem {
            status: "error".into(),
            message: "无法加载配置".into(),
        },
    };

    // ── Telegram ───────────────────────────────────
    let telegram_item = if let Some(ref cfg) = app_config {
        check_telegram(cfg).await
    } else {
        CheckItem {
            status: "error".into(),
            message: "无法加载配置".into(),
        }
    };

    // ── Roles ──────────────────────────────────────
    let roles_item = match &app_config {
        Some(cfg) => CheckItem {
            status: "ok".into(),
            message: format!("{} 个角色", cfg.roles.len()),
        },
        None => CheckItem {
            status: "error".into(),
            message: "无法加载配置".into(),
        },
    };

    // ── Output ─────────────────────────────────────
    if json {
        let result = DoctorResult {
            config: config_item,
            sqlite: sqlite_item,
            cron: cron_item,
            telegram: telegram_item,
            roles: roles_item,
        };
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        println!("OpenATrading 诊断报告");
        println!("{}", "─".repeat(40));
        println!(
            "{} {}",
            check_icon(&config_item.status),
            config_item.message
        );
        println!(
            "{} {}",
            check_icon(&sqlite_item.status),
            sqlite_item.message
        );
        println!("{} {}", check_icon(&cron_item.status), cron_item.message);
        println!(
            "{} {}",
            check_icon(&telegram_item.status),
            telegram_item.message
        );
        println!("{} {}", check_icon(&roles_item.status), roles_item.message);

        // Summary
        let all_ok = config_item.status == "ok"
            && sqlite_item.status == "ok"
            && cron_item.status != "error"
            && telegram_item.status != "error";
        println!("{}", "─".repeat(40));
        if all_ok {
            println!("✅ 一切正常");
        } else {
            println!("❌ 存在问题，请检查上方错误项");
        }
    }

    Ok(())
}
