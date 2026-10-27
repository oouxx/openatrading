use serde::{Deserialize, Serialize};

/// 盘口里的一档（价 + 挂单量）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderBookLevel {
    /// 委托价（元）。该档无挂单时为 0。
    pub price: f64,
    /// 挂单量，单位**手**（1 手 = 100 股）。
    pub volume: f64,
}

/// 个股盘口快照：买卖各五档。
///
/// `bids[0]` 是买一、`asks[0]` 是卖一，都按「离现价由近到远」排列；展示时
/// 卖档通常要倒过来从卖五排到卖一。
///
/// 非交易时段或停牌时各档价为 0 —— 调用方必须把「档位为 0」当作「没有挂单」，
/// 不能当成真实价格。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderBook {
    pub symbol: String,
    pub name: String,
    /// 最新价。
    pub last: f64,
    /// 昨收。
    pub prev_close: f64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    /// 涨跌幅 %。
    pub change_pct: f64,
    /// 涨停价。新浪不提供，固定为 0。
    pub limit_up: f64,
    /// 跌停价。新浪不提供，固定为 0。
    pub limit_down: f64,
    /// 买一 … 买五。
    pub bids: Vec<OrderBookLevel>,
    /// 卖一 … 卖五。
    pub asks: Vec<OrderBookLevel>,
}
