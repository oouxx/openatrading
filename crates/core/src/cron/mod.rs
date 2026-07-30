use anyhow::Result;
use chrono::NaiveDateTime;
use chrono_tz::Asia::Shanghai;
use pi_extension_api::{ToolCallOutput, ToolDefinition, ToolExecuteFn};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_cron_scheduler::{Job, JobScheduler};

use crate::api::Client as ApiClient;
use crate::context::AppContext;
use crate::roles::prompt::DecisionOutput;
use crate::social::NotificationSender;
use crate::trade::sqlite::TradeManager;
use crate::trade::{execute_buy, execute_sell};
use crate::RoleEntry;

#[derive(Debug, Clone, Deserialize)]
pub struct CronConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_collection_schedule")]
    pub collection_schedule: String,
    #[serde(default = "default_decision_schedule")]
    pub decision_schedule: String,
    #[serde(default = "default_news_schedule")]
    pub news_schedule: String,
}

const fn default_true() -> bool {
    true
}
fn default_collection_schedule() -> String {
    "0 0 18 * * 1-5".into()
}
fn default_decision_schedule() -> String {
    "0 */30 9-15 * * 1-5".into()
}
fn default_news_schedule() -> String {
    "0 */5 9-15 * * 1-5".into()
}

#[derive(Debug, Clone)]
pub struct CollectSummary {
    pub timestamp: NaiveDateTime,
    pub sources: Vec<String>,
    pub snapshot_count: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct TradeSuggestion {
    pub symbol: crate::types::StockCode,
    pub action: crate::trade::TradeAction,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct Decision {
    pub role_name: String,
    pub summary: String,
    pub trade_suggestions: Vec<TradeSuggestion>,
}

macro_rules! add_job {
    ($scheduler:expr, $schedule:expr, $name:expr, $task:expr) => {
        match Job::new_async_tz($schedule.as_str(), Shanghai, $task) {
            Ok(j) => {
                $scheduler.add(j).await?;
            }
            Err(e) => {
                crate::log::error!("[cron] {}: invalid schedule '{}': {e}", $name, $schedule);
            }
        }
    };
}

pub async fn start_scheduler(
    ctx: Arc<AppContext>,
    config: CronConfig,
    roles: Vec<RoleEntry>,
) -> Result<()> {
    if !config.enabled {
        return Ok(());
    }

    let scheduler = JobScheduler::new().await?;

    // ── Collection ──
    {
        let ctx = ctx.clone();
        add_job!(
            scheduler,
            config.collection_schedule,
            "collection",
            move |_uuid, _lock| {
                let ctx = ctx.clone();
                Box::pin(async move {
                    if let Err(e) = run_collection(&ctx).await {
                        crate::log::error!("[cron] collection error: {e}");
                    }
                })
            }
        );
    }

    // ── Decision ──
    {
        let ctx = ctx.clone();
        let roles = roles.clone();
        add_job!(
            scheduler,
            config.decision_schedule,
            "decision",
            move |_uuid, _lock| {
                let ctx = ctx.clone();
                let roles = roles.clone();
                Box::pin(async move {
                    if let Err(e) = run_decision(&ctx, &roles).await {
                        crate::log::error!("[cron] decision error: {e}");
                    }
                })
            }
        );
    }

    // ── News push (trading hours) ──
    {
        let ctx = ctx.clone();
        add_job!(
            scheduler,
            config.news_schedule,
            "news_push",
            move |_uuid, _lock| {
                let ctx = ctx.clone();
                Box::pin(async move {
                    if let Err(e) = run_news_push(&ctx.api, &*ctx.social).await {
                        crate::log::error!("[cron] news_push error: {e}");
                    }
                })
            }
        );
    }

    // ── News push (off-hours, 24/7) ──
    {
        let ctx = ctx.clone();
        let schedule = String::from("0 */30 0-8,16-23 * * *");
        add_job!(
            scheduler,
            schedule,
            "news_push_offhours",
            move |_uuid, _lock| {
                let ctx = ctx.clone();
                Box::pin(async move {
                    if let Err(e) = run_news_push(&ctx.api, &*ctx.social).await {
                        crate::log::error!("[cron] news_push error: {e}");
                    }
                })
            }
        );
    }

    scheduler.start().await?;
    crate::log::info!("[cron] scheduler started (tz=Asia/Shanghai)");

    tokio::signal::ctrl_c().await?;
    crate::log::info!("[cron] shutting down...");

    Ok(())
}

async fn run_collection(ctx: &AppContext) -> Result<()> {
    let mut sources = Vec::new();
    let mut snapshot_count = 0u32;

    match ctx.api.xuangubao.fetch_market_overview().await {
        Ok(overview) => {
            let json = serde_json::json!({
                "rise_count": overview.rise_count,
                "fall_count": overview.fall_count,
                "limit_up_count": overview.limit_up_count,
                "limit_down_count": overview.limit_down_count,
                "bomb_rate": overview.bomb_rate,
                "market_temperature": overview.market_temperature,
            });
            ctx.memory
                .save_snapshot("xuangubao_overview", &json.to_string())?;
            sources.push("xuangubao_overview".into());
            snapshot_count += 1;
        }
        Err(e) => {
            crate::log::warn!("[cron] collection: fetch overview failed: {e}");
        }
    }

    match ctx.api.xuangubao.fetch_limit_up().await {
        Ok(stocks) => {
            let json = serde_json::to_string(&stocks).unwrap_or_else(|_| "[]".into());
            ctx.memory.save_snapshot("xuangubao_limit_up", &json)?;
            sources.push("xuangubao_limit_up".into());
            snapshot_count += 1;
        }
        Err(e) => {
            crate::log::warn!("[cron] collection: fetch limit_up failed: {e}");
        }
    }

    match ctx.api.xuangubao.fetch_super_stock().await {
        Ok(stocks) => {
            let json = serde_json::to_string(&stocks).unwrap_or_else(|_| "[]".into());
            ctx.memory.save_snapshot("xuangubao_super_stock", &json)?;
            sources.push("xuangubao_super_stock".into());
            snapshot_count += 1;
        }
        Err(e) => {
            crate::log::warn!("[cron] collection: fetch super_stock failed: {e}");
        }
    }

    {
        let date = crate::utils::time::beijing_now()
            .format("%Y-%m-%d")
            .to_string();
        match ctx.api.jiuyan.fetch_field_str(&date, "1").await {
            Ok(items) => {
                let json = serde_json::to_string(&items).unwrap_or_else(|_| "{}".into());
                ctx.memory.save_snapshot("jiuyan_field_items", &json)?;
                sources.push("jiuyan_field_items".into());
                snapshot_count += 1;
            }
            Err(e) => {
                crate::log::warn!("[cron] collection: fetch jiuyan field failed: {e}");
            }
        }
    }

    let summary = CollectSummary {
        timestamp: crate::utils::time::beijing_now().naive_local(),
        sources,
        snapshot_count,
    };
    let text = format!(
        "📊 采集完成\n时间: {}\n数据源: {}\n快照: {} 条",
        summary.timestamp.format("%H:%M:%S"),
        summary.sources.join(", "),
        summary.snapshot_count,
    );
    let _ = ctx.social.notify_collect(&text).await;

    Ok(())
}

pub async fn run_decision(ctx: &AppContext, roles: &[RoleEntry]) -> Result<()> {
    crate::log::info!("[cron] decision: starting");

    let market_context = match ctx.api.xuangubao.fetch_market_overview().await {
        Ok(o) => crate::prompts::build_market_context(
            &o,
            &crate::utils::time::beijing_now()
                .format("%Y-%m-%d %H:%M")
                .to_string(),
        ),
        Err(e) => {
            crate::log::warn!("[cron] fetch market overview failed: {e}");
            crate::prompts::MARKET_CONTEXT_ERROR.to_string()
        }
    };

    update_all_position_prices(&ctx.trade, &ctx.api).await;

    for role in roles {
        if let Err(e) = run_decision_for_role(ctx, role, &market_context).await {
            crate::log::error!("[cron] decision failed for role '{}': {e}", role.name);
        }
    }

    crate::log::info!("[cron] decision: completed");
    Ok(())
}

async fn update_all_position_prices(trade: &TradeManager, api: &ApiClient) {
    let positions = match trade.list_all_positions() {
        Ok(p) => p,
        Err(e) => {
            crate::log::error!("[cron] list_all_positions failed: {e}");
            return;
        }
    };

    if positions.is_empty() {
        return;
    }

    let mut updated = 0u32;
    let mut failed = 0u32;

    for pos in &positions {
        match api.xuangubao.fetch_realtime(pos.symbol.as_str()).await {
            Ok(quote) => {
                if let Err(e) = trade.update_position_price(pos.id, quote.last_px) {
                    crate::log::warn!(
                        "[cron] update price failed for {} (id={}): {e}",
                        pos.symbol,
                        pos.id
                    );
                    failed += 1;
                } else {
                    updated += 1;
                }
            }
            Err(e) => {
                crate::log::warn!("[cron] fetch realtime failed for {}: {e}", pos.symbol);
                failed += 1;
            }
        }
    }

    crate::log::info!("[cron] position prices updated: {updated} ok, {failed} failed");
}

async fn run_decision_for_role(
    ctx: &AppContext,
    role: &RoleEntry,
    market_context: &str,
) -> Result<()> {
    crate::log::info!("[cron] decision for '{}' starting", role.name,);

    let mut user_message =
        crate::prompts::build_decision_user_message(market_context, &role.display_name);
    if crate::is_aggressive_mode() {
        user_message.push_str(crate::prompts::AGGRESSIVE_MODE_USER_PROMPT);
    }

    // ── Build make_decision tool ──
    // The LLM calls this tool with structured DecisionOutput params.
    // The execute function captures the structured data into shared state
    // so the caller can retrieve it after prompt() completes.
    let decision_state: Arc<Mutex<Option<DecisionOutput>>> = Arc::new(Mutex::new(None));
    {
        let state = decision_state.clone();
        let execute: ToolExecuteFn = Arc::new(move |_id, args, _signal| {
            let state = state.clone();
            Box::pin(async move {
                let decision: DecisionOutput =
                    serde_json::from_value(args).map_err(|e| format!("解析决策参数失败: {e}"))?;
                let mut guard = state.lock().await;
                *guard = Some(decision);
                Ok(ToolCallOutput {
                    content: vec![serde_json::json!({
                        "type": "text",
                        "text": "决策已记录，正在执行交易指令..."
                    })],
                    details: None,
                    is_error: false,
                    terminate: None,
                })
            })
        });

        let _make_decision_tool = ToolDefinition {
            name: "make_decision".to_string(),
            label: None,
            description: "提交交易决策。包含市场分析、交易指令列表（买入/卖出/持有）。                每次决策必须调用此工具提交最终决策结果。                参数：analysis（市场分析文本）、trades（交易指令数组）。                每个交易指令包含：action（Buy/Sell/Hold）、symbol（股票代码）、                name（股票名称）、shares（股数）、reason（理由）。".to_string(),
            prompt_snippet: None,
            prompt_guidelines: None,
            parameters: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "analysis": {
                        "type": "string",
                        "description": "市场分析与决策理由"
                    },
                    "trades": {
                        "type": "array",
                        "description": "交易指令列表",
                        "items": {
                            "type": "object",
                            "properties": {
                                "action": {
                                    "type": "string",
                                    "enum": ["Buy", "Sell", "Hold"],
                                    "description": "交易动作"
                                },
                                "symbol": {
                                    "type": "string",
                                    "description": "股票代码"
                                },
                                "name": {
                                    "type": "string",
                                    "description": "股票名称"
                                },
                                "shares": {
                                    "type": "integer",
                                    "description": "股数"
                                },
                                "reason": {
                                    "type": "string",
                                    "description": "交易理由"
                                }
                            },
                            "required": ["action", "symbol", "name", "shares", "reason"]
                        }
                    }
                },
                "required": ["analysis", "trades"]
            })),
            render_shell: None,
            execution_mode: None,
            execute: Some(execute),
        };

        let cwd = std::env::current_dir()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let _text = match ctx.pool.prompt_once(&cwd, user_message.as_str()).await {
            Ok(t) => t,
            Err(e) => {
                let err_msg = format!("❌ LLM 调用失败: {e}");
                let _ = ctx
                    .social
                    .notify_decision(&format!(
                        "【{} 决策】
{}",
                        role.display_name, err_msg
                    ))
                    .await;
                return Err(anyhow::anyhow!(err_msg));
            }
        };
    }

    // Retrieve structured decision from shared state
    let decision = decision_state
        .lock()
        .await
        .take()
        .ok_or_else(|| anyhow::anyhow!("LLM 未调用 make_decision tool，无法获取结构化决策"))?;

    let analysis = decision.analysis;
    let trade_instructions = decision.trades;

    let mut executed_trades: Vec<String> = Vec::new();

    for instr in &trade_instructions {
        match instr.action {
            crate::trade::TradeAction::Buy => {
                if let Err(e) = execute_buy(&ctx.trade, &ctx.api, &role.name, instr).await {
                    crate::log::error!(
                        "[cron] execute buy failed for {} {}: {e}",
                        instr.symbol,
                        instr.name
                    );
                    executed_trades
                        .push(format!("❌ 买入 {} {} 失败: {e}", instr.symbol, instr.name));
                } else {
                    executed_trades.push(format!(
                        "✅ 买入 {} {} {}股 — {}",
                        instr.symbol, instr.name, instr.shares, instr.reason
                    ));
                    let trade_text = format!(
                        "💹 持仓变动\n操作: 买入\n价格: {}\n股数: {}\n理由: {}",
                        instr.symbol, instr.shares, instr.reason,
                    );
                    let _ = ctx.social.notify_trade(&trade_text).await;
                }
            }
            crate::trade::TradeAction::Sell => {
                if let Err(e) = execute_sell(&ctx.trade, &ctx.api, &role.name, instr).await {
                    crate::log::error!(
                        "[cron] execute sell failed for {} {}: {e}",
                        instr.symbol,
                        instr.name
                    );
                    executed_trades
                        .push(format!("❌ 卖出 {} {} 失败: {e}", instr.symbol, instr.name));
                } else {
                    executed_trades.push(format!(
                        "✅ 卖出 {} {} — {}",
                        instr.symbol, instr.name, instr.reason
                    ));
                    let trade_text = format!("💹 持仓变动\n操作: 卖出\n理由: {}", instr.reason,);
                    let _ = ctx.social.notify_trade(&trade_text).await;
                }
            }
            crate::trade::TradeAction::Hold => {
                executed_trades.push(format!(
                    "⏸ 持有 {} {} — {}",
                    instr.symbol, instr.name, instr.reason
                ));
            }
            crate::trade::TradeAction::Open | crate::trade::TradeAction::Close => {
                crate::log::warn!(
                    "[cron] unexpected action '{}' for {} {}",
                    instr.action,
                    instr.symbol,
                    instr.name
                );
            }
        }
    }

    let trade_section = if executed_trades.is_empty() {
        "无交易操作".to_string()
    } else {
        executed_trades.join("\n")
    };

    let summary = format!(
        "【{} 决策】\n\n📊 分析：{}\n\n💹 操作：\n{}",
        role.display_name, analysis, trade_section
    );

    let suggestions: Vec<TradeSuggestion> = trade_instructions
        .iter()
        .map(|t| TradeSuggestion {
            symbol: t.symbol.clone(),
            action: t.action,
            reason: t.reason.clone(),
        })
        .collect();

    let decision = Decision {
        role_name: role.display_name.clone(),
        summary,
        trade_suggestions: suggestions,
    };

    let _ = ctx.social.notify_decision(&decision.summary).await;

    Ok(())
}

static LAST_PUSHED_NEWS_TIME: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);
static NEWS_TIME_INITIALIZED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

pub async fn run_news_push(api: &ApiClient, social: &dyn NotificationSender) -> Result<()> {
    let items = api.cls.fetch_telegraph("", None, 50).await?;
    if items.is_empty() {
        return Ok(());
    }

    if !NEWS_TIME_INITIALIZED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        let now = chrono::Utc::now().timestamp();
        LAST_PUSHED_NEWS_TIME.store(now, std::sync::atomic::Ordering::Relaxed);
        crate::log::info!("[cron] news_push: initialized, skipping existing items");
        return Ok(());
    }

    let last_time = LAST_PUSHED_NEWS_TIME.load(std::sync::atomic::Ordering::Relaxed);

    let mut new_items: Vec<&crate::api::cls::TelegramInfo> = items
        .iter()
        .filter(|item| {
            let ts = item.ctime.unwrap_or(0);
            ts > last_time
        })
        .collect();

    if new_items.is_empty() {
        return Ok(());
    }

    new_items.sort_by_key(|item| item.ctime.unwrap_or(0));

    let max_time = new_items
        .iter()
        .map(|item| item.ctime.unwrap_or(0))
        .max()
        .unwrap_or(last_time);
    LAST_PUSHED_NEWS_TIME.store(max_time, std::sync::atomic::Ordering::Relaxed);

    let important: Vec<&crate::api::cls::TelegramInfo> = new_items
        .iter()
        .filter(|item| {
            let level = item.level.as_deref().unwrap_or("");
            level == "A" || level == "B"
        })
        .copied()
        .collect();

    if important.is_empty() {
        return Ok(());
    }

    let beijing_tz =
        chrono::FixedOffset::east_opt(8 * 3600).unwrap_or_else(|| panic!("invalid UTC+8 offset"));
    for item in &important {
        let title = item.title.as_deref().unwrap_or("");
        let brief = item.brief.as_deref().unwrap_or("");
        let content = item.content.as_deref().unwrap_or("");
        let text = if !content.is_empty() {
            content
        } else if !brief.is_empty() {
            brief
        } else {
            title
        };
        let level_tag = match item.level.as_deref().unwrap_or("") {
            "A" => "🔴",
            "B" => "⚡",
            _ => "📰",
        };
        let time_str = item
            .ctime
            .map(|t| {
                chrono::DateTime::from_timestamp(t, 0)
                    .map(|dt| dt.with_timezone(&beijing_tz).format("%H:%M").to_string())
                    .unwrap_or_default()
            })
            .unwrap_or_default();
        let msg = format!("{level_tag} {time_str} {text}");
        if let Err(e) = social.notify_news(&msg).await {
            crate::log::warn!("[cron] news_push: send failed: {e}");
        }
    }

    crate::log::info!(
        "[cron] news_push: pushed {} important items (A/B level)",
        important.len()
    );
    Ok(())
}
