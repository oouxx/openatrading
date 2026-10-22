//! 模拟盘手动交易。
//!
//! 用户自己填写成交价与股数来记账，**不做撮合**（无滑点、无模拟成交价），
//! **不计手续费**。每个角色一个独立账户，初始资金由角色配置提供。
//!
//! 现金不落库，由成交流水推导：
//!
//! ```text
//! cash = initial_capital - Σ(买入 price × shares) + Σ(卖出 price × shares)
//! ```
//!
//! 因此不存在「现金列与流水不一致」的可能。

pub mod history;
pub mod order;
pub mod pnl;
pub mod quote;
pub mod service;
pub mod store;

pub use order::{
    round_money, Account, ClosedTrade, OrderRequest, PnlPoint, Position, PositionView, Price,
    Shares, Side, Trade,
};
pub use history::{ApiDailyCloseSource, DailyCloseSource};
pub use pnl::{BenchmarkView, Granularity, PnlCalendar, PnlPointView, PnlSummary};
pub use quote::{ApiQuoteSource, Quote, QuoteSource};
pub use service::{PnlCalendarRequest, TradeService};
pub use store::{OrderOutcome, PlaceOrderCtx, TradeStore};

/// 交易账户的错误类型。
///
/// 刻意把「调用方输入有误」与「存储失败」分开：前者应映射为 HTTP 400 并把
/// 中文原因展示给用户，后者才是 500。`is_client_error` 是这条分界线的唯一判据。
#[derive(Debug, Clone)]
pub enum OrderError {
    /// 买卖方向无法识别。
    InvalidSide(String),
    /// 价格非正数或非有限值。
    InvalidPrice(f64),
    /// 股数非正数，或买入股数不是 100 的整数倍。
    InvalidShares(String),
    /// 股票代码无法归一化。
    InvalidSymbol(String),
    /// 卖出的股数超过当前持仓。
    InsufficientShares { held: i64, requested: i64 },
    /// 买入所需金额超过可用现金。
    InsufficientCash { available: f64, required: f64 },
    /// 角色不存在（仅用于读取路径；下单会自动建账）。
    UnknownRole(String),
    /// 存储层失败。
    Store(String),
}

impl OrderError {
    /// 是否属于「调用方输入有误」（映射 HTTP 400 而非 500）。
    pub const fn is_client_error(&self) -> bool {
        !matches!(self, Self::Store(_))
    }
}

impl std::fmt::Display for OrderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidSide(v) => write!(f, "买卖方向无效: {v}（应为 buy 或 sell）"),
            Self::InvalidPrice(v) => write!(f, "价格必须是正数，收到 {v}"),
            Self::InvalidShares(msg) => f.write_str(msg),
            Self::InvalidSymbol(v) => write!(f, "股票代码无效: {v}"),
            Self::InsufficientShares { held, requested } => {
                write!(
                    f,
                    "卖出股数超过持仓：持有 {held} 股，试图卖出 {requested} 股"
                )
            }
            Self::InsufficientCash {
                available,
                required,
            } => write!(
                f,
                "可用现金不足：可用 {available:.2} 元，本次买入需要 {required:.2} 元"
            ),
            Self::UnknownRole(v) => write!(f, "角色不存在: {v}"),
            Self::Store(msg) => write!(f, "交易数据读写失败: {msg}"),
        }
    }
}

impl std::error::Error for OrderError {}

impl From<rusqlite::Error> for OrderError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Store(e.to_string())
    }
}

/// 交易模块的 Result 别名。
pub type Result<T> = std::result::Result<T, OrderError>;
