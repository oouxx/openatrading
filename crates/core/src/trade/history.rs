//! 历史日线收盘价。
//!
//! 区间盈亏分析必须用「当时的价格」给持仓估值——只靠实时行情的话，历史每一天
//! 都只能按成本价计价，曲线会变成只在成交日跳变的阶梯，等于没有浮动盈亏。
//!
//! 和 [`super::quote::QuoteSource`] 分开是因为两者语义不同：实时报价只关心最新价，
//! 日线要的是「某一天收在多少」，且必须能取到指数（`000001.SS` 上证指数与
//! `000001.SZ` 平安银行是两只不同的标的，不能共用 [`crate::types::StockCode`]
//! 的裸代码键）。

use std::collections::{BTreeMap, HashMap};

use async_trait::async_trait;

use crate::api::xuangubao::{KlinePeriod, XuanguBaoClient};

/// 一个标的的日线序列：`YYYY-MM-DD` → 收盘价，按日期升序。
pub type DailySeries = BTreeMap<String, f64>;

/// 代码 → 日线序列。键是**带交易所后缀的雅虎代码**（`600000.SS` / `000001.SS`）。
pub type DailyCloseMap = HashMap<String, DailySeries>;

/// 按交易取历史日线。
#[async_trait]
pub trait DailyCloseSource: Send + Sync {
    /// `trading_days` 是需要的**交易日**数量上限（从最近一个交易日往前数）。
    ///
    /// 缺键表示该标的没有拿到日线；整体失败返回 `Err`，由调用方决定降级
    /// （区间分析会退化成按成本价估值并标记 `incomplete`，而不是编造价格）。
    ///
    /// # Errors
    /// 行情接口不可用时返回错误。
    async fn daily_closes(
        &self,
        codes: &[String],
        trading_days: u32,
    ) -> anyhow::Result<DailyCloseMap>;
}

/// 基于选股宝日线接口的实现（批量：一次请求取多个标的）。
pub struct ApiDailyCloseSource {
    client: XuanguBaoClient,
}

impl ApiDailyCloseSource {
    /// 用默认接口地址构造。
    #[must_use]
    pub fn new() -> Self {
        Self {
            client: XuanguBaoClient::new(),
        }
    }
}

impl Default for ApiDailyCloseSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl DailyCloseSource for ApiDailyCloseSource {
    async fn daily_closes(
        &self,
        codes: &[String],
        trading_days: u32,
    ) -> anyhow::Result<DailyCloseMap> {
        if codes.is_empty() {
            return Ok(HashMap::new());
        }
        // 接口按 tick 数返回，多要一些以覆盖「区间起点之前的最后一个交易日」。
        let request_count = trading_days.saturating_add(40).max(60);
        let data = self
            .client
            .fetch_kline_batch(codes, KlinePeriod::Day, request_count)
            .await?;

        let mut out: DailyCloseMap = HashMap::with_capacity(data.len());
        for (code, kline) in data {
            let mut series = DailySeries::new();
            for candle in kline.candles {
                // 收盘价缺失或非正数的点直接丢弃，避免污染估值
                if candle.close > 0.0 {
                    series.insert(beijing_date(candle.time), candle.close);
                }
            }
            if !series.is_empty() {
                out.insert(code, series);
            }
        }
        Ok(out)
    }
}

/// 把接口的秒级时间戳转成北京时间的 `YYYY-MM-DD`。
///
/// 用 `chrono-tz` 而不是手写固定偏移，省掉「偏移量构造失败怎么办」这种
/// 永远不会发生的分支（也就不用 `expect`）。
fn beijing_date(timestamp_secs: i64) -> String {
    chrono::DateTime::from_timestamp(timestamp_secs, 0)
        .map(|utc| {
            utc.with_timezone(&chrono_tz::Asia::Shanghai)
                .format("%Y-%m-%d")
                .to_string()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::beijing_date;

    /// 2026-10-20 00:00 UTC → 北京时间仍是 10-20（+8 小时未跨日）。
    #[test]
    fn converts_utc_to_beijing_date() {
        assert_eq!(beijing_date(1_792_454_400), "2026-10-20");
    }

    /// 2026-10-20 16:00 UTC → 北京时间 10-21 00:00，必须进到第二天。
    #[test]
    fn rolls_over_after_16_utc() {
        assert_eq!(beijing_date(1_792_454_400 + 16 * 3600), "2026-10-21");
    }
}
