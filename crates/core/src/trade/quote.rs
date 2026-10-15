//! 持仓估值与股票名称补全所需的行情来源。
//!
//! 抽成 trait 的唯一目的是让 [`crate::trade::TradeService`] 可测——旧交易模块
//! 一个测试都没有，根因就是行情依赖直接写死在执行路径里。测试注入假行情即可
//! 覆盖估值与名称补全，不需要联网。

use std::collections::HashMap;

use async_trait::async_trait;

use crate::api::Client as ApiClient;
use crate::types::StockCode;

/// 单个标的的行情快照。
#[derive(Debug, Clone)]
pub struct Quote {
    pub name: String,
    /// 最新价。接口返回 0 或缺失时视为无效行情，不会出现在结果里。
    pub last_price: f64,
}

/// 按股票代码批量取行情。
#[async_trait]
pub trait QuoteSource: Send + Sync {
    /// 返回以 [`StockCode`] 为键的行情。
    ///
    /// 缺键表示该标的本次拿不到行情；整体失败（网络/接口错误）返回 `Err`，
    /// 由调用方决定降级策略——持仓估值会因此留空而不是编造数值。
    ///
    /// # Errors
    /// 行情接口不可用时返回错误。
    async fn quotes(&self, codes: &[StockCode]) -> anyhow::Result<HashMap<StockCode, Quote>>;
}

/// 基于选股宝实时行情的实现。
pub struct ApiQuoteSource {
    api: ApiClient,
}

impl ApiQuoteSource {
    #[must_use]
    pub fn new(api: ApiClient) -> Self {
        Self { api }
    }
}

#[async_trait]
impl QuoteSource for ApiQuoteSource {
    async fn quotes(&self, codes: &[StockCode]) -> anyhow::Result<HashMap<StockCode, Quote>> {
        if codes.is_empty() {
            return Ok(HashMap::new());
        }
        let raw: Vec<String> = codes.iter().map(ToString::to_string).collect();
        let fetched = self.api.xuangubao.fetch_realtime_batch(&raw).await?;

        // 批量接口以 Yahoo 格式为键返回，`StockCode::new` 会把后缀剥掉还原成
        // 与持仓表一致的裸代码。价格为 0 的条目视为无效行情，直接丢弃。
        Ok(fetched
            .into_iter()
            .filter(|(_, q)| q.last_px > 0.0)
            .map(|(symbol, q)| {
                (
                    StockCode::new(symbol),
                    Quote {
                        name: q.name,
                        last_price: q.last_px,
                    },
                )
            })
            .collect())
    }
}
