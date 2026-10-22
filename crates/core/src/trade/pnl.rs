//! 区间收益分析：把成交流水回放成「每天/每月/每年赚了多少」。
//!
//! 与 [`super::service::TradeService::pnl_history`] 的区别：
//!
//! * `pnl_history` 的历史点按**成本价**给持仓估值，只适合画一条粗糙的累计曲线；
//! * 本模块用**当日收盘价**回放，因此每一天都反映真实的浮动盈亏，才能支撑
//!   「日收益日历 / 月度收益 / 年度收益」这类按天切分的视图。
//!
//! 缺行情的日子按成本价兜底并把 `incomplete` 置真——不编造价格。

use std::collections::{BTreeSet, HashMap};

use serde::Serialize;

use super::history::DailyCloseMap;
use super::order::{round_money, Side, Trade};

/// 上证指数：既做基准，也用来推导 A 股交易日历（它上市以来每个交易日都有数据）。
pub const SHANGHAI_INDEX: &str = "000001.SS";
/// 深证成指。
pub const SHENZHEN_INDEX: &str = "399001.SZ";
/// 创业板指。
pub const CHINEXT_INDEX: &str = "399006.SZ";

/// 对比大盘用的三个指数。
pub const BENCHMARKS: [(&str, &str); 3] = [
    (SHANGHAI_INDEX, "上证"),
    (SHENZHEN_INDEX, "深证"),
    (CHINEXT_INDEX, "创业板"),
];

/// 聚合粒度。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Granularity {
    /// 每个交易日一个点。
    Day,
    /// 每个自然月一个点（取当月最后一个交易日）。
    Month,
    /// 每个自然年一个点（取当年最后一个交易日）。
    Year,
}

impl Granularity {
    /// 解析查询参数，非法值返回 `None`（由调用方映射成 400）。
    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "day" | "日" => Some(Self::Day),
            "month" | "月" => Some(Self::Month),
            "year" | "年" => Some(Self::Year),
            _ => None,
        }
    }

    /// 某一天属于哪个聚合桶（`2026-10-21` → `2026-10-21` / `2026-10` / `2026`）。
    fn bucket(self, date: &str) -> String {
        match self {
            Self::Day => date.to_string(),
            Self::Month => date.get(..7).unwrap_or(date).to_string(),
            Self::Year => date.get(..4).unwrap_or(date).to_string(),
        }
    }
}

/// 一个收益点。`pnl` / `pct` 是**本期**盈亏，`cum_*` 是从区间起点累计到本期的盈亏。
#[derive(Debug, Clone, Serialize)]
pub struct PnlPointView {
    /// 日粒度是 `YYYY-MM-DD`，月粒度 `YYYY-MM`，年粒度 `YYYY`。
    pub date: String,
    pub equity: f64,
    pub cash: f64,
    pub market_value: f64,
    pub pnl: f64,
    pub pct: f64,
    pub cum_pnl: f64,
    pub cum_pct: f64,
}

/// 指数基准在同一区间的涨跌幅。
#[derive(Debug, Clone, Serialize)]
pub struct BenchmarkView {
    pub code: String,
    pub name: String,
    /// 区间涨跌幅（百分数）。拿不到数据时为 `None`。
    pub pct: Option<f64>,
}

/// 区间汇总。
#[derive(Debug, Clone, Serialize)]
pub struct PnlSummary {
    pub total_pnl: f64,
    pub total_pct: f64,
    /// 区间内的点数：日粒度是交易日数，月/年粒度是月/年数。
    pub trading_days: u32,
    /// 上涨/下跌的**交易日**数量。
    pub win_days: u32,
    pub lose_days: u32,
    /// 区间最大回撤（百分数，正数表示回撤幅度）。
    pub max_drawdown_pct: f64,
}

/// 区间收益分析结果。
#[derive(Debug, Clone, Serialize)]
pub struct PnlCalendar {
    pub from: String,
    pub to: String,
    pub points: Vec<PnlPointView>,
    pub summary: PnlSummary,
    pub benchmarks: Vec<BenchmarkView>,
    /// 有持仓在某天拿不到收盘价、退化为成本价估值。
    pub incomplete: bool,
}

/// 计算所需的一切输入。打包成结构体而不是散参数，避免调用点出现 5 个同类型参数。
pub struct CalendarInput<'a> {
    pub initial_capital: f64,
    pub from: &'a str,
    pub to: &'a str,
    pub granularity: Granularity,
    /// 按成交时间升序。
    pub trades: &'a [Trade],
    /// 雅虎代码 → 日线。
    pub closes: &'a DailyCloseMap,
    /// 裸代码（`600000`）→ 雅虎代码（`600000.SS`）。
    pub yahoo_of: &'a HashMap<String, String>,
}

/// 回放成交流水，产出区间收益点。
///
/// 交易日历取自上证指数的日线日期：有日线的日子就是交易日，因此周末与节假日
/// 自然不会出现在结果里（前端据此显示「休」）。
#[must_use]
pub fn build_calendar(input: CalendarInput<'_>) -> PnlCalendar {
    let dates = trading_dates(input.closes);
    let mut incomplete = false;

    // 一天一个点：从第一笔成交日开始回放（区间起点之前的权益是「本期盈亏」的分母）
    let mut replay: Vec<(String, f64, f64, f64)> = Vec::new(); // (date, equity, cash, market_value)
    let mut cash = input.initial_capital;
    let mut holdings: HashMap<String, (i64, f64)> = HashMap::new();
    let first_trade_day = input
        .trades
        .first()
        .map(|t| day_of(&t.traded_at))
        .unwrap_or_default();

    for date in &dates {
        if *date < first_trade_day {
            continue;
        }
        // 先应用当天的成交，再按当天收盘价估值——当日买入当天就算浮盈浮亏
        for trade in input.trades.iter().filter(|t| day_of(&t.traded_at) == *date) {
            let amount = trade.price * trade.shares as f64;
            match trade.side {
                Side::Buy => {
                    cash -= amount;
                    let entry = holdings.entry(trade.symbol.0.clone()).or_insert((0, 0.0));
                    let total = entry.0 + trade.shares;
                    if total > 0 {
                        entry.1 = (entry.1 * entry.0 as f64 + amount) / total as f64;
                    }
                    entry.0 = total;
                }
                Side::Sell => {
                    cash += amount;
                    if let Some(entry) = holdings.get_mut(trade.symbol.0.as_str()) {
                        entry.0 -= trade.shares;
                    }
                    holdings.retain(|_, (shares, _)| *shares > 0);
                }
            }
        }

        if date.as_str() > input.to {
            break;
        }

        // 当日市值：优先用当日收盘价，停牌则退到最近一个交易日，再退到成本价
        let mut market_value = 0.0;
        for (symbol, (shares, cost)) in &holdings {
            let yahoo = input.yahoo_of.get(symbol).map_or("", String::as_str);
            let price = last_close_at(input.closes, yahoo, date);
            match price {
                Some(p) => market_value += p * *shares as f64,
                None => {
                    incomplete = true;
                    market_value += cost * *shares as f64;
                }
            }
        }
        replay.push((date.clone(), cash + market_value, cash, market_value));
    }

    // 日序列 → 按粒度聚合
    let mut daily: Vec<PnlPointView> = Vec::with_capacity(replay.len());
    let mut prev_equity = input.initial_capital;
    for (date, equity, day_cash, day_mv) in &replay {
        let pnl = equity - prev_equity;
        let pct = pct_of(pnl, prev_equity);
        daily.push(PnlPointView {
            date: date.clone(),
            equity: round_money(*equity),
            cash: round_money(*day_cash),
            market_value: round_money(*day_mv),
            pnl: round_money(pnl),
            pct,
            cum_pnl: 0.0,
            cum_pct: 0.0,
        });
        prev_equity = *equity;
    }

    let points = match input.granularity {
        Granularity::Day => daily,
        Granularity::Month | Granularity::Year => aggregate(&daily, input.granularity),
    };

    // 只保留区间内的点；累计值以区间起点为基准重算
    let points = clip_range(
        points,
        input.granularity.bucket(input.from).as_str(),
        input.granularity.bucket(input.to).as_str(),
        input.initial_capital,
    );

    let summary = summarize(&points, input.initial_capital);
    let benchmarks = benchmark_views(input.closes, input.from, input.to);

    PnlCalendar {
        from: input.from.to_string(),
        to: input.to.to_string(),
        points,
        summary,
        benchmarks,
        incomplete,
    }
}

/// 交易日历：优先上证指数，取不到则退化为所有标的日期并集。
fn trading_dates(closes: &DailyCloseMap) -> Vec<String> {
    let mut set: BTreeSet<&String> = BTreeSet::new();
    if let Some(index) = closes.get(SHANGHAI_INDEX) {
        for date in index.keys() {
            set.insert(date);
        }
    }
    if set.is_empty() {
        for series in closes.values() {
            for date in series.keys() {
                set.insert(date);
            }
        }
    }
    set.into_iter().cloned().collect()
}

/// 某标的在 `date` 及之前最近一个交易日的收盘价。
fn last_close_at(closes: &DailyCloseMap, yahoo: &str, date: &str) -> Option<f64> {
    let series = closes.get(yahoo)?;
    series
        .range(..=date.to_string())
        .next_back()
        .map(|(_, price)| *price)
}

/// 按月/年聚合：同一桶只保留最后一个交易日，盈亏相对上一桶末值。
fn aggregate(daily: &[PnlPointView], granularity: Granularity) -> Vec<PnlPointView> {
    let mut out: Vec<PnlPointView> = Vec::new();
    let mut prev_equity: Option<f64> = None;
    let mut idx = 0;
    while idx < daily.len() {
        let bucket = granularity.bucket(&daily[idx].date);
        let mut last = idx;
        while last + 1 < daily.len() && granularity.bucket(&daily[last + 1].date) == bucket {
            last += 1;
        }
        let point = &daily[last];
        // 首桶没有上期可比，沿用该点自身的本期盈亏
        let pnl = prev_equity.map_or(point.pnl, |prev| point.equity - prev);
        let pct = prev_equity.map_or(point.pct, |prev| pct_of(pnl, prev));
        out.push(PnlPointView {
            date: bucket,
            equity: point.equity,
            cash: point.cash,
            market_value: point.market_value,
            pnl: round_money(pnl),
            pct,
            cum_pnl: 0.0,
            cum_pct: 0.0,
        });
        prev_equity = Some(point.equity);
        idx = last + 1;
    }
    out
}

/// 切区间并把 `cum_*` 按区间起点重算。
///
/// `from` / `to` 必须是**已经按粒度对齐**的桶键（日粒度即日期本身），否则
/// 月/年桶（`2026-09`）与日边界（`2026-09-01`）比较会整段被滤掉。
fn clip_range(
    points: Vec<PnlPointView>,
    from: &str,
    to: &str,
    initial_capital: f64,
) -> Vec<PnlPointView> {
    let in_range: Vec<PnlPointView> = points
        .into_iter()
        .filter(|p| p.date.as_str() >= from && p.date.as_str() <= to)
        .collect();
    let Some(first) = in_range.first() else {
        return Vec::new();
    };
    // 区间起点前的权益 = 首点权益 - 首点本期盈亏
    let base = first.equity - first.pnl;
    let base = if base > 0.0 { base } else { initial_capital };

    in_range
        .into_iter()
        .map(|mut p| {
            p.cum_pnl = round_money(p.equity - base);
            p.cum_pct = pct_of(p.cum_pnl, base);
            p
        })
        .collect()
}

fn summarize(points: &[PnlPointView], initial_capital: f64) -> PnlSummary {
    if points.is_empty() {
        return PnlSummary {
            total_pnl: 0.0,
            total_pct: 0.0,
            trading_days: 0,
            win_days: 0,
            lose_days: 0,
            max_drawdown_pct: 0.0,
        };
    }
    let last = points.last().map_or(initial_capital, |p| p.equity);
    let base = points.first().map_or(initial_capital, |p| p.equity - p.pnl);
    let total_pnl = round_money(last - base);

    let win_days = points.iter().filter(|p| p.pnl > 0.0).count();
    let lose_days = points.iter().filter(|p| p.pnl < 0.0).count();

    // 最大回撤按权益峰值回撤口径，仅用区间内的点
    let mut peak = points.first().map_or(base, |p| p.equity);
    let mut max_dd = 0.0_f64;
    for p in points {
        if p.equity > peak {
            peak = p.equity;
        } else if peak > 0.0 {
            max_dd = max_dd.max((peak - p.equity) / peak * 100.0);
        }
    }

    PnlSummary {
        total_pnl,
        total_pct: pct_of(total_pnl, base),
        trading_days: u32::try_from(points.len()).unwrap_or(u32::MAX),
        win_days: u32::try_from(win_days).unwrap_or(u32::MAX),
        lose_days: u32::try_from(lose_days).unwrap_or(u32::MAX),
        max_drawdown_pct: (max_dd * 100.0).round() / 100.0,
    }
}

fn benchmark_views(closes: &DailyCloseMap, from: &str, to: &str) -> Vec<BenchmarkView> {
    BENCHMARKS
        .iter()
        .map(|(code, name)| BenchmarkView {
            code: (*code).to_string(),
            name: (*name).to_string(),
            pct: range_pct(closes, code, from, to),
        })
        .collect()
}

/// 指数在区间内的涨跌幅：起点取 `from` 之前最后一个收盘价（含当天）。
fn range_pct(closes: &DailyCloseMap, code: &str, from: &str, to: &str) -> Option<f64> {
    let series = closes.get(code)?;
    let start = series.range(..=from.to_string()).next_back().map(|(_, v)| *v)?;
    let end = series.range(..=to.to_string()).next_back().map(|(_, v)| *v)?;
    if start <= 0.0 {
        return None;
    }
    Some(((end - start) / start * 10000.0).round() / 100.0)
}

fn pct_of(delta: f64, base: f64) -> f64 {
    if base.abs() < f64::EPSILON {
        return 0.0;
    }
    ((delta / base) * 10000.0).round() / 100.0
}

fn day_of(timestamp: &str) -> String {
    timestamp.chars().take(10).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::StockCode;
    use std::collections::BTreeMap;

    fn trade(day: &str, side: Side, price: f64, shares: i64) -> Trade {
        Trade {
            id: 0,
            role_name: "t".into(),
            symbol: StockCode::new("600000"),
            stock_name: "浦发银行".into(),
            side,
            price,
            shares,
            cost_price: price,
            realized_pnl: None,
            reason: String::new(),
            traded_at: format!("{day}T09:30:00+08:00"),
        }
    }

    fn closes_with(points: &[(&str, f64)]) -> DailyCloseMap {
        let mut series = BTreeMap::new();
        for (date, price) in points {
            series.insert((*date).to_string(), *price);
        }
        let mut map = DailyCloseMap::new();
        map.insert("600000.SS".to_string(), series.clone());
        map.insert(SHANGHAI_INDEX.to_string(), series);
        map
    }

    fn yahoo() -> HashMap<String, String> {
        HashMap::from([("600000".to_string(), "600000.SS".to_string())])
    }

    /// 历史点用当日收盘价估值：买入后价格翻倍，区间收益应体现浮动盈亏，
    /// 而不是只在成交日跳变一次。
    #[test]
    fn values_holdings_at_daily_close() {
        let trades = vec![trade("2026-10-20", Side::Buy, 10.0, 1000)];
        let closes = closes_with(&[
            ("2026-10-20", 10.0),
            ("2026-10-21", 15.0),
            ("2026-10-22", 20.0),
        ]);
        let cal = build_calendar(CalendarInput {
            initial_capital: 100_000.0,
            from: "2026-10-20",
            to: "2026-10-22",
            granularity: Granularity::Day,
            trades: &trades,
            closes: &closes,
            yahoo_of: &yahoo(),
        });
        assert_eq!(cal.points.len(), 3);
        assert!(!cal.incomplete);
        // 10-21：现金 90000 + 持仓 1000×15 = 105000，较昨日 +5000
        assert_eq!(cal.points[1].equity, 105_000.0);
        assert_eq!(cal.points[1].pnl, 5_000.0);
        assert_eq!(cal.points[1].pct, 5.0);
        // 10-22：110000，较昨日 +5000
        assert_eq!(cal.points[2].pnl, 5_000.0);
        assert_eq!(cal.summary.total_pnl, 10_000.0);
    }

    /// 停牌日（当日无收盘价）应退到最近一个交易日，而不是当作 0 或成本价。
    #[test]
    fn falls_back_to_last_available_close() {
        let trades = vec![trade("2026-10-20", Side::Buy, 10.0, 1000)];
        let closes = closes_with(&[("2026-10-20", 10.0), ("2026-10-23", 12.0)]);
        let cal = build_calendar(CalendarInput {
            initial_capital: 100_000.0,
            from: "2026-10-20",
            to: "2026-10-23",
            granularity: Granularity::Day,
            trades: &trades,
            closes: &closes,
            yahoo_of: &yahoo(),
        });
        // 10-21、10-22 不在日历里（无日线），10-23 用当日价
        assert_eq!(cal.points.len(), 2);
        assert_eq!(cal.points[1].equity, 102_000.0);
        assert!(!cal.incomplete);
    }

    /// 完全没有该标的日线时不编造价格：退化为成本价并标记 incomplete。
    #[test]
    fn marks_incomplete_without_any_close() {
        let trades = vec![trade("2026-10-20", Side::Buy, 10.0, 1000)];
        let mut closes = closes_with(&[("2026-10-20", 10.0), ("2026-10-21", 10.0)]);
        closes.remove("600000.SS");
        let cal = build_calendar(CalendarInput {
            initial_capital: 100_000.0,
            from: "2026-10-20",
            to: "2026-10-21",
            granularity: Granularity::Day,
            trades: &trades,
            closes: &closes,
            yahoo_of: &yahoo(),
        });
        assert!(cal.incomplete);
        assert_eq!(cal.points[1].equity, 100_000.0);
    }

    /// 月聚合：取每月最后一个交易日，盈亏相对上月末。
    #[test]
    fn aggregates_by_month() {
        let trades = vec![trade("2026-09-30", Side::Buy, 10.0, 1000)];
        let closes = closes_with(&[
            ("2026-09-30", 10.0),
            ("2026-10-30", 12.0),
            ("2026-10-31", 13.0),
        ]);
        let cal = build_calendar(CalendarInput {
            initial_capital: 100_000.0,
            from: "2026-09-01",
            to: "2026-10-31",
            granularity: Granularity::Month,
            trades: &trades,
            closes: &closes,
            yahoo_of: &yahoo(),
        });
        assert_eq!(cal.points.len(), 2);
        assert_eq!(cal.points[0].date, "2026-09");
        assert_eq!(cal.points[1].date, "2026-10");
        assert_eq!(cal.points[1].equity, 103_000.0);
        assert_eq!(cal.points[1].pnl, 3_000.0);
    }

    /// 最大回撤：从峰值回落的最大幅度。
    #[test]
    fn computes_max_drawdown() {
        let trades = vec![trade("2026-10-20", Side::Buy, 10.0, 1000)];
        let closes = closes_with(&[
            ("2026-10-20", 10.0),
            ("2026-10-21", 20.0),
            ("2026-10-22", 10.0),
        ]);
        let cal = build_calendar(CalendarInput {
            initial_capital: 100_000.0,
            from: "2026-10-20",
            to: "2026-10-22",
            granularity: Granularity::Day,
            trades: &trades,
            closes: &closes,
            yahoo_of: &yahoo(),
        });
        // 峰值 110000（10-21）→ 100000（10-22），回撤 9.09%
        assert_eq!(cal.summary.max_drawdown_pct, 9.09);
    }
}
