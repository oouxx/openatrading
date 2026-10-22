//! 交易用例层。
//!
//! 负责三件事，全部围绕「不编造数据」：
//! 1. 下单前的输入校验与归一化（非法输入在写库前就被拒）；
//! 2. 持仓估值——行情拿不到时留空并标记 `valuation_complete = false`，
//!    而不是拿成本价冒充现价；
//! 3. 账户快照与盈亏曲线。

use std::collections::HashMap;
use std::sync::Arc;

use super::order::{
    round_money, Account, ClosedTrade, OrderRequest, PnlPoint, Position, PositionView, Side, Trade,
};
use super::history::{ApiDailyCloseSource, DailyCloseSource};
use super::pnl::{build_calendar, CalendarInput, Granularity, PnlCalendar};
use super::quote::QuoteSource;
use super::store::{OrderOutcome, PlaceOrderCtx, TradeStore};
use super::{OrderError, Result};
use crate::types::StockCode;

/// 区间收益分析请求。
///
/// `initial_capital` 由调用方从角色配置解析后传入，与 `place_order` 一致。
pub struct PnlCalendarRequest {
    pub role: String,
    pub initial_capital: f64,
    /// 含当天的起始日期 `YYYY-MM-DD`。
    pub from: String,
    /// 含当天的结束日期 `YYYY-MM-DD`。
    pub to: String,
    pub granularity: Granularity,
}

/// 模拟盘交易服务。
pub struct TradeService {
    store: Arc<TradeStore>,
    quote_source: Arc<dyn QuoteSource>,
    daily_closes: Arc<dyn DailyCloseSource>,
}

impl TradeService {
    /// 用默认的历史日线源（选股宝日线接口）构造。
    #[must_use]
    pub fn new(store: Arc<TradeStore>, quote_source: Arc<dyn QuoteSource>) -> Self {
        Self::with_daily_closes(
            store,
            quote_source,
            Arc::new(ApiDailyCloseSource::new()),
        )
    }

    /// 注入自定义历史日线源——测试用假数据替换网络依赖。
    #[must_use]
    pub fn with_daily_closes(
        store: Arc<TradeStore>,
        quote_source: Arc<dyn QuoteSource>,
        daily_closes: Arc<dyn DailyCloseSource>,
    ) -> Self {
        Self {
            store,
            quote_source,
            daily_closes,
        }
    }

    /// 区间收益分析（曲线 / 日历视图共用）。
    ///
    /// 与 [`Self::pnl_history`] 的区别见 [`super::pnl`] 模块文档：这里用**当日收盘价**
    /// 估值，因此能按天切分「日收益」，代价是需要额外拉一批历史日线。
    ///
    /// # Errors
    /// 读库或历史行情失败返回错误——不静默返回空曲线，否则用户会以为「没赚钱」。
    pub async fn pnl_calendar(
        &self,
        req: &PnlCalendarRequest,
    ) -> Result<PnlCalendar> {
        let trades = self.store.all_trades(&req.role)?;

        // 需要覆盖「区间起点」和「首笔成交日」里更早的那个：
        //   - 日历视图会翻到首笔成交之前的月份，那些月要能取到交易日历；
        //   - 曲线只需要有持仓的区间。
        // 但「全部」传的是 2000-01-01 这类哨兵值，直接照算会向接口要几十年数据，
        // 所以对 tick 数设上限（约 6 年交易日）；更早的日子退化为成本价估值。
        let data_start = trades
            .first()
            .map_or_else(|| req.from.clone(), |t| {
                let first = t.traded_at.chars().take(10).collect::<String>();
                if first < req.from { first } else { req.from.clone() }
            });
        let needed_days = trading_days_between(&data_start, &req.to).min(MAX_DAILY_TICKS);

        let mut codes: Vec<String> = Vec::new();
        let mut yahoo_of: HashMap<String, String> = HashMap::new();
        for trade in &trades {
            let bare = trade.symbol.0.clone();
            if yahoo_of.contains_key(&bare) {
                continue;
            }
            if let Ok(yahoo) = crate::api::utils::to_yahoo_symbol(&bare) {
                yahoo_of.insert(bare, yahoo.clone());
                codes.push(yahoo);
            }
        }
        // 指数既是基准，也用来推导交易日历
        codes.extend(
            super::pnl::BENCHMARKS
                .iter()
                .map(|(code, _)| (*code).to_string()),
        );

        let closes = self
            .daily_closes
            .daily_closes(&codes, needed_days)
            .await
            .map_err(|e| OrderError::Store(format!("历史日线获取失败: {e}")))?;

        Ok(build_calendar(CalendarInput {
            initial_capital: req.initial_capital,
            from: &req.from,
            to: &req.to,
            granularity: req.granularity,
            trades: &trades,
            closes: &closes,
            yahoo_of: &yahoo_of,
        }))
    }

    /// 下一笔手动单。
    ///
    /// `initial_capital` 由调用方从角色配置解析后传入——角色配置的合并逻辑在
    /// web/cli 层（`config.toml` 的 `[[roles]]` 与磁盘角色文件都要看），
    /// core 不重复实现一份。
    ///
    /// # Errors
    /// 输入非法返回对应的 [`OrderError`] 变体；校验失败不写库。
    pub async fn place_order(
        &self,
        role: &str,
        initial_capital: f64,
        req: &OrderRequest,
    ) -> Result<OrderOutcome> {
        let symbol = normalize_symbol(&req.symbol)?;
        // 买入必须是整手，卖出允许零股（见 `Shares::require_board_lot`）。
        let shares = match req.side {
            Side::Buy => req.shares.require_board_lot()?,
            Side::Sell => req.shares,
        };
        // 名称空着时用行情补全；前端通常已经在搜索框选中股票并带上了真实名称。
        let requested_name = req.stock_name.trim();
        let stock_name = if requested_name.is_empty() {
            self.lookup_name(&symbol).await
        } else {
            requested_name.to_string()
        };
        let traded_at = crate::utils::time::beijing_now()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();

        self.store.place_order(&PlaceOrderCtx {
            role_name: role,
            symbol,
            stock_name: &stock_name,
            side: req.side,
            shares: shares.get(),
            price: req.price.get(),
            reason: req.reason.trim(),
            initial_capital,
            traded_at: &traded_at,
        })
    }

    /// 当前持仓，带实时估值。
    ///
    /// 第二个返回值是「估值是否完整」：有任何持仓拿不到行情即为 `false`，
    /// 相应的 `last_price` / `market_value` / `unrealized_*` 为 `None`。
    ///
    /// # Errors
    /// 只有读库失败才返回错误；行情失败会降级而不是让整个接口失败。
    pub async fn positions(&self, role: &str) -> Result<(Vec<PositionView>, bool)> {
        let positions = self.store.list_positions(role)?;
        if positions.is_empty() {
            return Ok((Vec::new(), true));
        }
        let codes: Vec<StockCode> = positions.iter().map(|p| p.symbol.clone()).collect();
        let quotes = self.fetch_quotes(&codes).await;

        let mut complete = true;
        let views = positions
            .into_iter()
            .map(|position| {
                let last_price = quotes.get(&position.symbol).map(|q| q.last_price);
                if last_price.is_none() {
                    complete = false;
                }
                PositionView::with_last_price(position, last_price)
            })
            .collect();
        Ok((views, complete))
    }

    /// 账户快照（现金 / 市值 / 权益 / 盈亏）。
    ///
    /// # Errors
    /// 读库失败返回错误。
    pub async fn account(&self, role: &str, initial_capital: f64) -> Result<Account> {
        let (cash, realized_pnl) = self.store.cash_and_realized(role, initial_capital)?;
        let (views, valuation_complete) = self.positions(role).await?;

        let market_value = round_money(views.iter().filter_map(|v| v.market_value).sum());
        let unrealized_pnl = round_money(views.iter().filter_map(|v| v.unrealized_pnl).sum());
        let equity = round_money(cash + market_value);
        let total_pnl = round_money(equity - initial_capital);
        let total_pnl_pct = if initial_capital > 0.0 {
            (total_pnl / initial_capital * 1000.0).round() / 10.0
        } else {
            0.0
        };

        Ok(Account {
            role_name: role.to_string(),
            initial_capital,
            cash,
            market_value,
            equity,
            unrealized_pnl,
            realized_pnl,
            total_pnl,
            total_pnl_pct,
            valuation_complete,
        })
    }

    /// 成交流水（倒序）。
    ///
    /// # Errors
    /// 读库失败返回错误。
    pub fn trades(&self, role: &str, limit: u32) -> Result<Vec<Trade>> {
        self.store.list_trades(role, limit)
    }

    /// 已平仓记录（倒序）。
    ///
    /// # Errors
    /// 读库失败返回错误。
    pub fn closed_trades(&self, role: &str, limit: u32) -> Result<Vec<ClosedTrade>> {
        self.store.list_closed_trades(role, limit)
    }

    /// 盈亏曲线。
    ///
    /// 历史点没有当时的历史行情，持仓按**成本**计价（与旧实现同一限制，已在
    /// 文档中标注）；末点用实时行情，因此「今天」这一点是准确的。
    ///
    /// # Errors
    /// 读库失败返回错误。
    pub async fn pnl_history(
        &self,
        role: &str,
        initial_capital: f64,
        days: usize,
    ) -> Result<Vec<PnlPoint>> {
        let trades = self.store.all_trades(role)?;
        if trades.is_empty() {
            return Ok(Vec::new());
        }

        let mut cash = initial_capital;
        let mut holdings: HashMap<StockCode, (i64, f64)> = HashMap::new();
        let mut points: Vec<PnlPoint> = Vec::new();

        for trade in &trades {
            let amount = trade.price * trade.shares as f64;
            match trade.side {
                Side::Buy => {
                    cash -= amount;
                    let entry = holdings.entry(trade.symbol.clone()).or_insert((0, 0.0));
                    let total_shares = entry.0 + trade.shares;
                    entry.1 = (entry.1 * entry.0 as f64 + amount) / total_shares as f64;
                    entry.0 = total_shares;
                }
                Side::Sell => {
                    cash += amount;
                    if let Some(entry) = holdings.get_mut(&trade.symbol) {
                        entry.0 -= trade.shares;
                    }
                    holdings.retain(|_, (shares, _)| *shares > 0);
                }
            }
            let market_value_cost_basis: f64 = holdings
                .values()
                .map(|(shares, cost)| cost * *shares as f64)
                .sum();
            points.push(make_point(
                initial_capital,
                day_of(&trade.traded_at),
                cash,
                market_value_cost_basis,
            ));
        }

        // 末点：用实时行情估值，让「今天」这一点反映真实浮动盈亏。
        if !holdings.is_empty() {
            let (views, _) = self.positions(role).await?;
            let live_value: f64 = views.iter().filter_map(|v| v.market_value).sum();
            if live_value > 0.0 {
                points.push(make_point(
                    initial_capital,
                    crate::utils::time::beijing_now()
                        .format("%Y-%m-%d")
                        .to_string(),
                    cash,
                    live_value,
                ));
            }
        }

        // 按日去重保留当日最后一条（同一交易日多次成交只留一个点）。
        let mut daily: Vec<PnlPoint> = Vec::new();
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        for point in points.into_iter().rev() {
            if seen.insert(point.date.clone()) {
                daily.push(point);
            }
        }
        daily.reverse();

        if days > 0 && daily.len() > days {
            daily.drain(..daily.len() - days);
        }
        Ok(daily)
    }

    /// 清空某角色的持仓与流水（角色本身保留）。
    ///
    /// # Errors
    /// 写库失败返回错误。
    pub fn reset(&self, role: &str) -> Result<()> {
        self.store.reset_role(role)
    }

    /// 拉行情，失败只记警告并返回空表——由调用方把「估值不完整」透出去。
    async fn fetch_quotes(&self, codes: &[StockCode]) -> HashMap<StockCode, super::quote::Quote> {
        match self.quote_source.quotes(codes).await {
            Ok(quotes) => quotes,
            Err(e) => {
                crate::log::warn!("持仓估值拉取行情失败，估值将留空: {e}");
                HashMap::new()
            }
        }
    }

    /// 用户没填股票名时补一个；失败不阻断下单。
    async fn lookup_name(&self, symbol: &StockCode) -> String {
        match self.quote_source.quotes(std::slice::from_ref(symbol)).await {
            Ok(quotes) => quotes
                .get(symbol)
                .map_or_else(String::new, |q| q.name.clone()),
            Err(e) => {
                crate::log::debug!("下单时补全股票名失败: {e}");
                String::new()
            }
        }
    }
}

/// 归一化股票代码：先用行情侧的解析器校验，再转成存储用的裸代码。
fn normalize_symbol(raw: &str) -> Result<StockCode> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(OrderError::InvalidSymbol("股票代码不能为空".to_string()));
    }
    crate::api::utils::to_yahoo_symbol(trimmed)
        .map(StockCode::new)
        .map_err(|e| OrderError::InvalidSymbol(format!("{trimmed}（{e}）")))
}

/// 单次日线请求的 tick 上限（约 6 年交易日）。
///
/// 「全部」维度用 2000-01-01 当哨兵起点，不设上限会向接口要上万根 K 线。
const MAX_DAILY_TICKS: u32 = 1_500;

/// 两个日期之间的自然日数，按 ~5/7 折算成交易日估算。
///
/// 只是给日线接口一个够用的 `tick_count`，多要无害、少要会缺数据，所以向上取。
fn trading_days_between(from: &str, to: &str) -> u32 {
    use chrono::NaiveDate;

    let parse = |s: &str| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok();
    let (Some(start), Some(end)) = (parse(from), parse(to)) else {
        // 日期解析不了就退到一个足够大的默认值（约三年交易日）
        return 750;
    };
    let calendar_days = (end - start).num_days().max(0);
    let estimate = (calendar_days * 5).div_euclid(7) + 10;
    u32::try_from(estimate).unwrap_or(750).max(10)
}

fn day_of(timestamp: &str) -> String {
    timestamp.chars().take(10).collect()
}

fn make_point(initial_capital: f64, date: String, cash: f64, market_value: f64) -> PnlPoint {
    let equity = cash + market_value;
    let return_pct = if initial_capital > 0.0 {
        (equity - initial_capital) / initial_capital * 100.0
    } else {
        0.0
    };
    PnlPoint {
        date,
        equity: round_money(equity),
        cash: round_money(cash),
        market_value: round_money(market_value),
        return_pct: (return_pct * 100.0).round() / 100.0,
    }
}

/// 让 `Position` 的字段在估值缺位时也能被 API 层直接使用。
impl Position {
    /// 转成不带估值的视图（行情不可用时的兜底）。
    #[must_use]
    pub fn without_valuation(self) -> PositionView {
        PositionView::with_last_price(self, None)
    }
}
