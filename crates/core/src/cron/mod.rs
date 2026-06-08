use anyhow::Result;
use chrono::NaiveDateTime;
use chrono_tz::Asia::Shanghai;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio_cron_scheduler::{Job, JobScheduler};

use crate::api::Client as ApiClient;
use crate::llm::tools::{create_historical_context_tool, create_positions_tool};
use crate::llm::{DynTool, LlmClient};
use crate::memory::MemoryStore;
use crate::roles::pnl::INITIAL_CAPITAL;
use crate::roles::prompt::TradeInstruction;
use crate::social::NotificationSender;
use crate::trade::{Position, TradeManager};
use crate::RoleEntry;

const MAX_SINGLE_POSITION_RATIO: f64 = 0.3;
const MAX_TOTAL_POSITION_RATIO: f64 = 0.9;
const MIN_SHARES: i64 = 100;

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

fn default_true() -> bool {
    true
}
fn default_collection_schedule() -> String {
    "0 */30 9-15 * * 1-5".into()
}
fn default_decision_schedule() -> String {
    "0 0 0 * * 1-5".into()
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
    pub symbol: String,
    pub action: String,
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
            },
            Err(e) => {
                eprintln!("[cron] {}: invalid schedule '{}': {e}", $name, $schedule);
            },
        }
    };
}

/// Grouped context for scheduler dependencies, avoiding an 8-parameter constructor.
pub struct SchedulerContext {
    pub config: CronConfig,
    pub memory: Arc<dyn MemoryStore>,
    pub trade: Arc<dyn TradeManager>,
    pub social: Arc<dyn NotificationSender>,
    pub llm: LlmClient,
    pub api: ApiClient,
    pub roles: Vec<RoleEntry>,
    pub mcp_servers: Vec<crate::llm::mcp_tools::McpServerConfig>,
}

pub async fn start_scheduler(ctx: SchedulerContext) -> Result<()> {
    if !ctx.config.enabled {
        return Ok(());
    }

    let scheduler = JobScheduler::new().await?;

    // ── Collection ──
    {
        let mem = ctx.memory.clone();
        let soc = ctx.social.clone();
        let api = ctx.api.clone();
        add_job!(
            scheduler,
            ctx.config.collection_schedule,
            "collection",
            move |_uuid, _lock| {
                let mem = mem.clone();
                let soc = soc.clone();
                let api = api.clone();
                Box::pin(async move {
                    if let Err(e) = run_collection(&*mem, &api, &*soc).await {
                        eprintln!("[cron] collection error: {e}");
                    }
                })
            }
        );
    }

    // ── Decision ──
    {
        let mem = ctx.memory.clone();
        let trd = ctx.trade.clone();
        let soc = ctx.social.clone();
        let llm = ctx.llm.clone();
        let api = ctx.api.clone();
        let roles = ctx.roles.clone();
        let tool_cfg = crate::llm::ToolConfig {
            mcp_servers: ctx.mcp_servers.clone(),
        };
        let all_tools: Vec<Arc<DynTool>> = crate::llm::build_tools(&tool_cfg)
            .await
            .all_tools()
            .to_vec();

        add_job!(
            scheduler,
            ctx.config.decision_schedule,
            "decision",
            move |_uuid, _lock| {
                let mem = mem.clone();
                let trd = trd.clone();
                let soc = soc.clone();
                let llm = llm.clone();
                let api = api.clone();
                let roles = roles.clone();
                let decision_tools = all_tools.clone();
                Box::pin(async move {
                    if let Err(e) = run_decision(
                        mem.clone(),
                        trd.clone(),
                        &*soc,
                        &llm,
                        &api,
                        &roles,
                        &decision_tools,
                    )
                    .await
                    {
                        eprintln!("[cron] decision error: {e}");
                    }
                })
            }
        );
    }

    // ── News push (trading hours) ──
    {
        let soc = ctx.social.clone();
        let api = ctx.api.clone();
        add_job!(
            scheduler,
            ctx.config.news_schedule,
            "news_push",
            move |_uuid, _lock| {
                let soc = soc.clone();
                let api = api.clone();
                Box::pin(async move {
                    if let Err(e) = run_news_push(&api, &*soc).await {
                        eprintln!("[cron] news_push error: {e}");
                    }
                })
            }
        );
    }

    // ── News push (off-hours, 24/7) ──
    {
        let soc = ctx.social.clone();
        let api = ctx.api.clone();
        let schedule = String::from("0 */30 0-8,16-23 * * *");
        add_job!(
            scheduler,
            schedule,
            "news_push_offhours",
            move |_uuid, _lock| {
                let soc = soc.clone();
                let api = api.clone();
                Box::pin(async move {
                    if let Err(e) = run_news_push(&api, &*soc).await {
                        eprintln!("[cron] news_push error: {e}");
                    }
                })
            }
        );
    }

    scheduler.start().await?;
    eprintln!("[cron] scheduler started (tz=Asia/Shanghai)");

    // Wait forever (Ctrl+C handled by caller)
    tokio::signal::ctrl_c().await?;
    eprintln!("\n[cron] shutting down...");

    Ok(())
}

async fn run_collection(
    memory: &dyn MemoryStore,
    api: &ApiClient,
    social: &dyn NotificationSender,
) -> Result<()> {
    let mut sources = Vec::new();
    let mut snapshot_count = 0u32;

    match api.xuangubao.fetch_market_overview().await {
        Ok(overview) => {
            let json = serde_json::json!({
                "rise_count": overview.rise_count,
                "fall_count": overview.fall_count,
                "limit_up_count": overview.limit_up_count,
                "limit_down_count": overview.limit_down_count,
                "bomb_rate": overview.bomb_rate,
                "market_temperature": overview.market_temperature,
            });
            memory.save_snapshot("xuangubao_overview", &json.to_string())?;
            sources.push("xuangubao_overview".into());
            snapshot_count += 1;
        },
        Err(e) => {
            tracing::warn!("[cron] collection: fetch overview failed: {e}");
        },
    }

    match api.xuangubao.fetch_limit_up().await {
        Ok(stocks) => {
            let json = serde_json::to_string(&stocks).unwrap_or_else(|_| "[]".into());
            memory.save_snapshot("xuangubao_limit_up", &json)?;
            sources.push("xuangubao_limit_up".into());
            snapshot_count += 1;
        },
        Err(e) => {
            tracing::warn!("[cron] collection: fetch limit_up failed: {e}");
        },
    }

    let summary = CollectSummary {
        timestamp: crate::utils::time::beijing_now().naive_local(),
        sources,
        snapshot_count,
    };
    let _ = social.notify_collect(&summary).await;

    Ok(())
}

pub async fn run_decision(
    memory: Arc<dyn MemoryStore>,
    trade: Arc<dyn TradeManager>,
    social: &dyn NotificationSender,
    llm: &LlmClient,
    api: &ApiClient,
    roles: &[RoleEntry],
    tools: &[Arc<DynTool>],
) -> Result<()> {
    tracing::info!("[cron] decision: starting");

    let market_context = match api.xuangubao.fetch_market_overview().await {
        Ok(o) => format!(
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
        Err(e) => {
            tracing::warn!("[cron] fetch market overview failed: {e}");
            "## 市场数据获取失败\n".to_string()
        },
    };

    update_all_position_prices(&*trade, api).await;

    for role in roles {
        // Inject per-role tools: positions + memory (LLM calls them as needed)
        let mut role_tools = tools.to_vec();
        role_tools.push(create_positions_tool(trade.clone(), role.name.clone()));
        role_tools.push(create_historical_context_tool(memory.clone()));

        if let Err(e) = run_decision_for_role(
            &*trade,
            social,
            llm,
            api,
            role,
            &market_context,
            &role_tools,
        )
        .await
        {
            tracing::error!("[cron] decision failed for role '{}': {e}", role.name);
        }
    }

    tracing::info!("[cron] decision: completed");
    Ok(())
}

async fn update_all_position_prices(trade: &dyn TradeManager, api: &ApiClient) {
    let positions = match trade.list_all_positions() {
        Ok(p) => p,
        Err(e) => {
            tracing::error!("[cron] list_all_positions failed: {e}");
            return;
        },
    };

    if positions.is_empty() {
        return;
    }

    let mut updated = 0u32;
    let mut failed = 0u32;

    for pos in &positions {
        match api.xuangubao.fetch_realtime(&pos.symbol).await {
            Ok(quote) => {
                if let Err(e) = trade.update_position_price(pos.id, quote.last_px) {
                    tracing::warn!(
                        "[cron] update price failed for {} (id={}): {e}",
                        pos.symbol,
                        pos.id
                    );
                    failed += 1;
                } else {
                    updated += 1;
                }
            },
            Err(e) => {
                tracing::warn!("[cron] fetch realtime failed for {}: {e}", pos.symbol);
                failed += 1;
            },
        }
    }

    tracing::info!("[cron] position prices updated: {updated} ok, {failed} failed");
}

async fn run_decision_for_role(
    trade: &dyn TradeManager,
    social: &dyn NotificationSender,
    llm: &LlmClient,
    api: &ApiClient,
    role: &RoleEntry,
    market_context: &str,
    tools: &[Arc<DynTool>],
) -> Result<()> {
    tracing::info!(
        "[cron] decision for '{}': {} tools injected (positions, memory, market)",
        role.name,
        tools.len()
    );

    // LLM calls get_current_positions / get_historical_context tools as needed
    let market_user_message = format!(
        "{}\n\n请以【{}】身份，根据策略体系分析当前市场状态和持仓情况。你可以调用 get_current_positions 查询持仓，调用 get_historical_context 查询历史数据。给出今日复盘和明日操作建议。",
        market_context, role.display_name
    );

    let system = crate::skills::SkillRegistry::build_role_prompt(role);
    let decision = match crate::llm::session::AgentSession::new(llm, &system)
        .with_tools(tools)
        .process_decision(&market_user_message)
        .await
    {
        Ok(d) => d,
        Err(e) => {
            let err_msg = format!("❌ LLM 调用失败: {e}");
            let decision = Decision {
                role_name: role.display_name.clone(),
                summary: format!("【{} 决策】\n{}", role.display_name, err_msg),
                trade_suggestions: vec![],
            };
            let _ = social.notify_decision(&decision).await;
            return Err(e);
        },
    };

    let analysis = decision.analysis;
    let trade_instructions = decision.trades;

    let mut executed_trades: Vec<String> = Vec::new();

    for instr in &trade_instructions {
        match instr.action.as_str() {
            "buy" => {
                if let Err(e) = execute_buy(trade, api, &role.name, instr).await {
                    tracing::error!(
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
                    let log = crate::trade::TradeLog {
                        id: 0,
                        position_id: 0,
                        action: "Open".into(),
                        price: 0.0,
                        shares: instr.shares,
                        reason: instr.reason.clone(),
                        created_at: crate::utils::time::beijing_now()
                            .format("%Y-%m-%d %H:%M:%S")
                            .to_string(),
                    };
                    let _ = social.notify_trade(&log).await;
                }
            },
            "sell" => {
                if let Err(e) = execute_sell(trade, api, &role.name, instr).await {
                    tracing::error!(
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
                    let log = crate::trade::TradeLog {
                        id: 0,
                        position_id: 0,
                        action: "Close".into(),
                        price: 0.0,
                        shares: 0,
                        reason: instr.reason.clone(),
                        created_at: crate::utils::time::beijing_now()
                            .format("%Y-%m-%d %H:%M:%S")
                            .to_string(),
                    };
                    let _ = social.notify_trade(&log).await;
                }
            },
            "hold" => {
                executed_trades.push(format!(
                    "⏸ 持有 {} {} — {}",
                    instr.symbol, instr.name, instr.reason
                ));
            },
            other => {
                tracing::warn!(
                    "[cron] unknown action '{}' for {} {}",
                    other,
                    instr.symbol,
                    instr.name
                );
            },
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
            action: t.action.clone(),
            reason: t.reason.clone(),
        })
        .collect();

    let decision = Decision {
        role_name: role.display_name.clone(),
        summary,
        trade_suggestions: suggestions,
    };

    if let Err(e) = social.notify_decision(&decision).await {
        tracing::error!(
            "[cron] notify decision failed for role '{}': {e}",
            role.name
        );
    }

    Ok(())
}

async fn execute_buy(
    trade: &dyn TradeManager,
    api: &ApiClient,
    role_name: &str,
    instr: &TradeInstruction,
) -> Result<()> {
    if instr.shares < MIN_SHARES {
        anyhow::bail!(
            "shares {} is less than minimum {}",
            instr.shares,
            MIN_SHARES
        );
    }

    if instr.shares % MIN_SHARES != 0 {
        anyhow::bail!(
            "shares {} is not a multiple of {}",
            instr.shares,
            MIN_SHARES
        );
    }

    let quote = api
        .xuangubao
        .fetch_realtime(&instr.symbol)
        .await
        .map_err(|e| anyhow::anyhow!("fetch realtime for {} failed: {e}", instr.symbol))?;

    let price = quote.last_px;
    if price <= 0.0 {
        anyhow::bail!("invalid price {} for {}", price, instr.symbol);
    }

    let cost = price * instr.shares as f64;

    let current_positions = trade.list_positions(role_name)?;
    let total_position_value: f64 = current_positions
        .iter()
        .map(|p| p.current_price.unwrap_or(p.cost_price) * p.shares as f64)
        .sum();
    let same_symbol_value: f64 = current_positions
        .iter()
        .filter(|p| p.symbol == instr.symbol)
        .map(|p| p.current_price.unwrap_or(p.cost_price) * p.shares as f64)
        .sum();

    if cost + same_symbol_value > INITIAL_CAPITAL * MAX_SINGLE_POSITION_RATIO {
        anyhow::bail!(
            "single stock position {:.0} + new {:.0} exceeds max {:.0} ({}% of capital)",
            same_symbol_value,
            cost,
            INITIAL_CAPITAL * MAX_SINGLE_POSITION_RATIO,
            (MAX_SINGLE_POSITION_RATIO * 100.0) as i32
        );
    }

    if total_position_value + cost > INITIAL_CAPITAL * MAX_TOTAL_POSITION_RATIO {
        anyhow::bail!(
            "total position value {:.0} + new {:.0} exceeds max {:.0} ({}% of capital)",
            total_position_value,
            cost,
            INITIAL_CAPITAL * MAX_TOTAL_POSITION_RATIO,
            (MAX_TOTAL_POSITION_RATIO * 100.0) as i32
        );
    }

    let today = crate::utils::time::beijing_now()
        .format("%Y-%m-%d")
        .to_string();

    let pos = Position {
        id: 0,
        role_name: role_name.to_string(),
        symbol: instr.symbol.clone(),
        stock_name: instr.name.clone(),
        entry_date: today,
        cost_price: price,
        shares: instr.shares,
        current_price: Some(price),
        stop_loss: None,
        target_price: None,
        entry_reason: instr.reason.clone(),
        sector: String::new(),
        position_pct: cost / INITIAL_CAPITAL,
        created_at: String::new(),
        updated_at: String::new(),
    };

    let position_id = trade.add_position(pos)?;
    tracing::info!(
        "[cron] BUY {} {} {}shares @ {:.2} — {} (position_id={})",
        instr.symbol,
        instr.name,
        instr.shares,
        price,
        instr.reason,
        position_id
    );

    Ok(())
}

async fn execute_sell(
    trade: &dyn TradeManager,
    api: &ApiClient,
    role_name: &str,
    instr: &TradeInstruction,
) -> Result<()> {
    let positions = trade.list_positions(role_name)?;

    let matching: Vec<&Position> = positions
        .iter()
        .filter(|p| p.symbol == instr.symbol)
        .collect();

    if matching.is_empty() {
        anyhow::bail!(
            "no open position found for {} in role {}",
            instr.symbol,
            role_name
        );
    }

    for pos in matching {
        let sell_price = match api.xuangubao.fetch_realtime(&pos.symbol).await {
            Ok(quote) if quote.last_px > 0.0 => quote.last_px,
            _ => pos.current_price.unwrap_or(pos.cost_price),
        };
        trade.close_position(pos.id, sell_price, &instr.reason)?;
        tracing::info!(
            "[cron] SELL {} {} (id={}) @ {:.2} — {}",
            pos.symbol,
            pos.stock_name,
            pos.id,
            sell_price,
            instr.reason
        );
    }

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

    // On first run, seed with current time to avoid flooding old news
    if !NEWS_TIME_INITIALIZED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        let now = chrono::Utc::now().timestamp();
        LAST_PUSHED_NEWS_TIME.store(now, std::sync::atomic::Ordering::Relaxed);
        tracing::info!("[cron] news_push: initialized, skipping existing items");
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

    // Push items with level A (most important) or B (important)
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

    let beijing_tz = chrono::FixedOffset::east_opt(8 * 3600).unwrap();
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
        let msg = format!("{} {} {}", level_tag, time_str, text);
        if let Err(e) = social.notify_news(&msg).await {
            tracing::warn!("[cron] news_push: send failed: {e}");
        }
    }

    tracing::info!(
        "[cron] news_push: pushed {} important items (A/B level)",
        important.len()
    );
    Ok(())
}

// ── Tests ──────────────────────────────────────────────

#[cfg(test)]
mod tests {}
