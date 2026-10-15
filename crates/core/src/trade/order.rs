//! 交易领域类型。
//!
//! 价格与股数用 newtype 包住：这两个值在调用点都是裸数字，混用不会有编译错误，
//! 而它们的量纲完全不同（元 vs 股）。newtype 同时承担第一道校验——构造即拒绝
//! 非法值，非法数据进不到下游。

use serde::{Deserialize, Serialize};

use super::OrderError;
use crate::types::StockCode;

/// 买卖方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Buy,
    Sell,
}

impl Side {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Buy => "buy",
            Self::Sell => "sell",
        }
    }

    /// 解析数据库或 API 传入的方向字符串（大小写不敏感）。
    ///
    /// # Errors
    /// 无法识别的取值返回 [`OrderError::InvalidSide`]。
    pub fn parse(s: &str) -> Result<Self, OrderError> {
        match s.trim().to_ascii_lowercase().as_str() {
            "buy" => Ok(Self::Buy),
            "sell" => Ok(Self::Sell),
            other => Err(OrderError::InvalidSide(other.to_string())),
        }
    }
}

/// 成交价（元）。构造时拒绝非正数与非有限值。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "f64", into = "f64")]
pub struct Price(f64);

impl Price {
    /// # Errors
    /// 非正数、NaN 或无穷大返回 [`OrderError::InvalidPrice`]。
    pub fn new(value: f64) -> Result<Self, OrderError> {
        if !value.is_finite() || value <= 0.0 {
            return Err(OrderError::InvalidPrice(value));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }
}

impl TryFrom<f64> for Price {
    type Error = OrderError;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<Price> for f64 {
    fn from(p: Price) -> Self {
        p.0
    }
}

/// 股数。构造时拒绝非正数。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "i64", into = "i64")]
pub struct Shares(i64);

impl Shares {
    /// # Errors
    /// 非正数返回 [`OrderError::InvalidShares`]。
    pub fn new(value: i64) -> Result<Self, OrderError> {
        if value <= 0 {
            return Err(OrderError::InvalidShares(format!(
                "股数必须是正整数，收到 {value}"
            )));
        }
        Ok(Self(value))
    }

    /// 校验为 A 股整手（100 股的整数倍）。
    ///
    /// 只用于**买入**——防止录入错误；卖出允许零股，因为持有零股
    /// （配股、送股产生）后清仓卖出是真实存在的场景。
    ///
    /// # Errors
    /// 非 100 的整数倍返回 [`OrderError::InvalidShares`]。
    pub fn require_board_lot(self) -> Result<Self, OrderError> {
        if self.0 % 100 != 0 {
            return Err(OrderError::InvalidShares(format!(
                "买入股数必须是 100 的整数倍，收到 {}",
                self.0
            )));
        }
        Ok(self)
    }

    #[must_use]
    pub const fn get(self) -> i64 {
        self.0
    }
}

impl TryFrom<i64> for Shares {
    type Error = OrderError;

    fn try_from(value: i64) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<Shares> for i64 {
    fn from(s: Shares) -> Self {
        s.0
    }
}

/// 金额按「分」四舍五入。
///
/// 交易金额都是 `price × shares` 的累加，用 f64 累加会攒出尾差；所有对外
/// 展示与比较前都过一遍这里，避免出现 `1234.5600000000002` 这类值。
#[must_use]
pub fn round_money(value: f64) -> f64 {
    let rounded = (value * 100.0).round() / 100.0;
    // Rust 的 `Sum<f64>` 用 `-0.0` 作加法单位元，所以「无持仓」时的空求和得到
    // `-0.0`，会在 JSON 里原样出现。加 0.0 归一成 0.0（对其它值无影响）。
    rounded + 0.0
}

/// 下单请求。
///
/// 字段多于 3 个且同类型字段多，按项目规范合并为 struct，函数只收一个引用。
#[derive(Debug, Clone, Deserialize)]
pub struct OrderRequest {
    pub side: Side,
    /// 股票代码。接受裸代码（`300272`）、Yahoo 格式（`600519.SS`）等常见写法，
    /// 服务内统一归一化为 [`StockCode`]。
    pub symbol: String,
    /// 股票名称。留空时由服务端尝试用行情接口补全。
    #[serde(default)]
    pub stock_name: String,
    pub shares: Shares,
    pub price: Price,
    #[serde(default)]
    pub reason: String,
}

/// 一条成交流水。
///
/// 买入行的 `cost_price` 等于成交价、`realized_pnl` 为 `None`；
/// 卖出行的 `cost_price` 是卖出当时的持仓成本、`realized_pnl` 为已实现盈亏。
/// 这样每行自洽，历史视图不需要回放全部流水。
#[derive(Debug, Clone, Serialize)]
pub struct Trade {
    pub id: i64,
    pub role_name: String,
    pub symbol: StockCode,
    pub stock_name: String,
    pub side: Side,
    pub price: f64,
    pub shares: i64,
    pub cost_price: f64,
    pub realized_pnl: Option<f64>,
    pub reason: String,
    pub traded_at: String,
}

/// 当前持仓。
///
/// 同一 `(role_name, symbol)` 合并为一行，`cost_price` 是加权平均成本。
#[derive(Debug, Clone, Serialize)]
pub struct Position {
    pub role_name: String,
    pub symbol: StockCode,
    pub stock_name: String,
    pub shares: i64,
    pub cost_price: f64,
    pub opened_at: String,
}

/// 带实时估值的持仓视图。
///
/// 行情不可用时估值字段为 `None`——不编造数值，让 UI 显示 `—`。
#[derive(Debug, Clone, Serialize)]
pub struct PositionView {
    pub role_name: String,
    pub symbol: StockCode,
    pub stock_name: String,
    pub shares: i64,
    pub cost_price: f64,
    pub opened_at: String,
    pub last_price: Option<f64>,
    pub market_value: Option<f64>,
    pub unrealized_pnl: Option<f64>,
    pub unrealized_pct: Option<f64>,
}

impl PositionView {
    /// 用最新价补齐估值字段；`last_price` 为 `None` 时全部留空。
    #[must_use]
    pub fn with_last_price(position: Position, last_price: Option<f64>) -> Self {
        let cost = position.cost_price * position.shares as f64;
        let (market_value, unrealized_pnl, unrealized_pct) = match last_price {
            Some(px) => {
                let mv = px * position.shares as f64;
                let pnl = mv - cost;
                let pct = if cost > 0.0 { pnl / cost * 100.0 } else { 0.0 };
                (
                    Some(round_money(mv)),
                    Some(round_money(pnl)),
                    Some((pct * 100.0).round() / 100.0),
                )
            }
            None => (None, None, None),
        };
        Self {
            role_name: position.role_name,
            symbol: position.symbol,
            stock_name: position.stock_name,
            shares: position.shares,
            cost_price: position.cost_price,
            opened_at: position.opened_at,
            last_price,
            market_value,
            unrealized_pnl,
            unrealized_pct,
        }
    }
}

/// 一笔已平仓记录（由卖出流水直接投影）。
#[derive(Debug, Clone, Serialize)]
pub struct ClosedTrade {
    pub id: i64,
    pub role_name: String,
    pub symbol: StockCode,
    pub stock_name: String,
    pub shares: i64,
    pub cost_price: f64,
    pub exit_price: f64,
    pub realized_pnl: f64,
    pub realized_pct: f64,
    pub closed_at: String,
    pub reason: String,
}

/// 账户快照。
#[derive(Debug, Clone, Serialize)]
pub struct Account {
    pub role_name: String,
    pub initial_capital: f64,
    pub cash: f64,
    pub market_value: f64,
    pub equity: f64,
    pub unrealized_pnl: f64,
    pub realized_pnl: f64,
    pub total_pnl: f64,
    pub total_pnl_pct: f64,
    /// 为 `false` 表示有持仓拿不到行情，`market_value` / `equity` / `total_pnl`
    /// 是不完整值——调用方必须据此提示用户，不能当作准确数据展示。
    pub valuation_complete: bool,
}

/// 盈亏曲线上的一个点。
#[derive(Debug, Clone, Serialize)]
pub struct PnlPoint {
    pub date: String,
    pub equity: f64,
    pub cash: f64,
    pub market_value: f64,
    /// 累计收益率（百分数，如 `1.5` 表示 +1.5%）。
    pub return_pct: f64,
}
